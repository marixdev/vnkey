/*
 * VnKeyInputController — Xử lý phím tiếng Việt qua Input Method Kit
 *
 * Mỗi instance đại diện cho một phiên nhập liệu (1 per client).
 * IMKit gọi handleEvent:client: cho mỗi phím, ta chuyển qua
 * vnkey-engine và trả kết quả qua insertText: / setMarkedText:.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#import "VnKeyInputController.h"
#import "vnkey-engine.h"
#import <Carbon/Carbon.h>
#import <CoreGraphics/CoreGraphics.h>
#include <unistd.h>

/* ==================== Preferences keys ==================== */
static NSString *const kVnKeyInputMethod   = @"VnKeyInputMethod";
static NSString *const kVnKeyVietMode      = @"VnKeyVietMode";
static NSString *const kVnKeySpellCheck    = @"VnKeySpellCheck";
static NSString *const kVnKeyFreeMarking   = @"VnKeyFreeMarking";
static NSString *const kVnKeyModernStyle   = @"VnKeyModernStyle";
static NSString *const kVnKeyAutoRestore   = @"VnKeyAutoRestore";
static NSString *const kVnKeyEdeMode       = @"VnKeyEdeMode";
static NSString *const kVnKeyMacroEnabled   = @"VnKeyMacroEnabled";
static NSString *const kVnKeyMacros         = @"VnKeyMacros";
static NSString *const kVnKeyUsePreedit     = @"VnKeyUsePreedit";

/* ==================== Helpers ==================== */

/* Đọc cài đặt từ UserDefaults */
static void loadPreferences(void *engine, BOOL *outVietMode, BOOL *outUsePreedit) {
    NSUserDefaults *defaults = [NSUserDefaults standardUserDefaults];

    /* Đăng ký mặc định */
    NSDictionary *defaultValues = @{
        kVnKeyInputMethod: @0,
        kVnKeyVietMode:    @YES,
        kVnKeySpellCheck:  @YES,
        kVnKeyFreeMarking: @YES,
        kVnKeyModernStyle: @YES,
        kVnKeyAutoRestore: @YES,
        kVnKeyEdeMode:     @NO,
        kVnKeyMacroEnabled:@NO,
        kVnKeyMacros:      @"",
        kVnKeyUsePreedit:  @NO,
    };
    [defaults registerDefaults:defaultValues];

    int im = (int)[defaults integerForKey:kVnKeyInputMethod];
    BOOL viet = [defaults boolForKey:kVnKeyVietMode];
    BOOL spell = [defaults boolForKey:kVnKeySpellCheck];
    BOOL free = [defaults boolForKey:kVnKeyFreeMarking];
    BOOL modern = [defaults boolForKey:kVnKeyModernStyle];
    BOOL autoRestore = [defaults boolForKey:kVnKeyAutoRestore];
    BOOL ede = [defaults boolForKey:kVnKeyEdeMode];
    BOOL macroEn = [defaults boolForKey:kVnKeyMacroEnabled];
    BOOL usePreedit = [defaults boolForKey:kVnKeyUsePreedit];

    vnkey_engine_set_input_method(engine, im);
    vnkey_engine_set_viet_mode(engine, viet ? 1 : 0);
    vnkey_engine_set_options(engine, free ? 1 : 0, modern ? 1 : 0,
                            spell ? 1 : 0, autoRestore ? 1 : 0, ede ? 1 : 0,
                            macroEn ? 1 : 0);

    /* Nạp macros */
    NSString *macros = [defaults stringForKey:kVnKeyMacros];
    if (macros && macros.length > 0) {
        vnkey_engine_load_macros(engine, [macros UTF8String]);
    }

    if (outVietMode) *outVietMode = viet;
    if (outUsePreedit) *outUsePreedit = usePreedit;
}

/* Xóa n ký tự UTF-16 cuối khỏi NSMutableString */
static void removeLastChars(NSMutableString *str, NSUInteger count) {
    for (NSUInteger i = 0; i < count && str.length > 0; i++) {
        /* Xử lý surrogate pair */
        NSRange last = [str rangeOfComposedCharacterSequenceAtIndex:str.length - 1];
        [str deleteCharactersInRange:last];
    }
}

/* ==================== VnKeyInputController ==================== */

@implementation VnKeyInputController

- (instancetype)initWithServer:(IMKServer *)server
                      delegate:(id)delegate
                        client:(id)client {
    self = [super initWithServer:server delegate:delegate client:client];
    if (self) {
        _preedit = [[NSMutableString alloc] init];
        _engine = vnkey_engine_new();
        _vietMode = YES;
        _usePreedit = NO;
        loadPreferences(_engine, &_vietMode, &_usePreedit);

        /* Lắng nghe thay đổi cài đặt từ Preferences panel */
        [[NSNotificationCenter defaultCenter]
            addObserver:self
               selector:@selector(preferencesChanged:)
                   name:@"VnKeyPreferencesChanged"
                 object:nil];
    }
    return self;
}

- (void)dealloc {
    [[NSNotificationCenter defaultCenter] removeObserver:self];
    if (_engine) {
        vnkey_engine_free(_engine);
        _engine = NULL;
    }
}

/* ==================== IMKInputController overrides ==================== */

- (BOOL)handleEvent:(NSEvent *)event client:(id)sender {
    /* Chỉ xử lý key-down */
    if (event.type != NSEventTypeKeyDown) {
        return NO;
    }

    NSEventModifierFlags flags = event.modifierFlags;

    /* Bỏ qua phím có Command hoặc Control (phím tắt hệ thống) */
    if (flags & (NSEventModifierFlagCommand | NSEventModifierFlagControl)) {
        [self commitPreedit:sender];
        vnkey_engine_reset(_engine);
        return NO;
    }

    unsigned short keyCode = event.keyCode;
    NSString *chars = event.characters;

    /* Phím Escape */
    if (keyCode == kVK_Escape) {
        [self commitPreedit:sender];
        vnkey_engine_reset(_engine);
        return NO;
    }

    /* Phím Return / Tab */
    if (keyCode == kVK_Return || keyCode == kVK_Tab) {
        [self commitPreedit:sender];
        vnkey_engine_reset(_engine);
        return NO;
    }

    /* Phím Space: xử lý macro expansion hoặc soft reset */
    if (keyCode == kVK_Space) {
        if (_usePreedit && _preedit.length > 0) {
            [self commitPreedit:sender];
        }
        /* Gửi Space qua engine để kích hoạt macro expansion */
        uint8_t outBuf[1024];
        size_t actualLen = 0;
        size_t backspaces = 0;
        int processed = vnkey_engine_process(
            _engine, (uint32_t)' ',
            outBuf, sizeof(outBuf), &actualLen, &backspaces, NULL);
        if (processed && (backspaces > 0 || actualLen > 0)) {
            /* Macro expanded: xóa từ cũ + chèn expansion */
            if (_usePreedit) {
                /* Preedit đã commit ở trên, xoá trực tiếp */
                for (size_t i = 0; i < backspaces; i++) {
                    [sender insertText:@"" replacementRange:NSMakeRange(NSNotFound, 1)];
                }
            } else {
                /* Commit-inline: gửi backspace xoá từ cũ */
                for (size_t i = 0; i < backspaces; i++) {
                    /* Gửi Fn+Delete (forward delete không hoạt động), dùng deleteBackward */
                    CGEventRef bsDown = CGEventCreateKeyboardEvent(NULL, kVK_Delete, true);
                    CGEventRef bsUp = CGEventCreateKeyboardEvent(NULL, kVK_Delete, false);
                    CGEventPost(kCGHIDEventTap, bsDown);
                    CGEventPost(kCGHIDEventTap, bsUp);
                    CFRelease(bsDown);
                    CFRelease(bsUp);
                }
                /* Đợi backspace xử lý */
                usleep(backspaces * 5000);
            }
            if (actualLen > 0) {
                NSString *output = [[NSString alloc]
                    initWithBytes:outBuf
                           length:actualLen
                         encoding:NSUTF8StringEncoding];
                if (output) {
                    [sender insertText:output
                      replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
                }
            }
            return YES;
        }
        /* Không match macro → commit và để Space pass-through */
        vnkey_engine_soft_reset(_engine);
        return NO;
    }

    /* Backspace */
    if (keyCode == kVK_Delete) {
        return [self handleBackspace:sender];
    }

    /* Phím mũi tên, Home, End, PageUp, PageDown: commit và bỏ qua */
    if (keyCode == kVK_LeftArrow || keyCode == kVK_RightArrow ||
        keyCode == kVK_UpArrow || keyCode == kVK_DownArrow ||
        keyCode == kVK_Home || keyCode == kVK_End ||
        keyCode == kVK_PageUp || keyCode == kVK_PageDown) {
        [self commitPreedit:sender];
        vnkey_engine_reset(_engine);
        return NO;
    }

    /* Lấy ký tự ASCII */
    if (chars.length == 0) {
        return NO;
    }
    unichar ch = [chars characterAtIndex:0];
    if (ch < 0x21 || ch > 0x7E) {
        [self commitPreedit:sender];
        vnkey_engine_reset(_engine);
        return NO;
    }

    /* Gửi tới vnkey engine */
    uint8_t outBuf[256];
    size_t actualLen = 0;
    size_t backspaces = 0;

    int processed = vnkey_engine_process(
        _engine, (uint32_t)ch,
        outBuf, sizeof(outBuf), &actualLen, &backspaces, NULL);

    if (processed) {
        if (_usePreedit) {
            /* Chế độ preedit: cập nhật marked text */
            removeLastChars(_preedit, backspaces);
            if (actualLen > 0) {
                NSString *output = [[NSString alloc]
                    initWithBytes:outBuf
                           length:actualLen
                         encoding:NSUTF8StringEncoding];
                if (output) {
                    [_preedit appendString:output];
                }
            }
        } else {
            /* Chế độ commit-inline: ghi trực tiếp không gạch chân */
            /* Xóa ký tự cũ bằng backspace */
            for (size_t i = 0; i < backspaces; i++) {
                CGEventRef bsDown = CGEventCreateKeyboardEvent(NULL, kVK_Delete, true);
                CGEventRef bsUp = CGEventCreateKeyboardEvent(NULL, kVK_Delete, false);
                CGEventPost(kCGHIDEventTap, bsDown);
                CGEventPost(kCGHIDEventTap, bsUp);
                CFRelease(bsDown);
                CFRelease(bsUp);
            }
            if (backspaces > 0) {
                usleep(backspaces * 5000);
            }
            /* Chèn kết quả */
            if (actualLen > 0) {
                NSString *output = [[NSString alloc]
                    initWithBytes:outBuf
                           length:actualLen
                         encoding:NSUTF8StringEncoding];
                if (output) {
                    [sender insertText:output
                      replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
                }
            }
            return YES;
        }
    } else {
        if (_usePreedit) {
            [_preedit appendFormat:@"%c", (char)ch];
        } else {
            /* Commit-inline: để hệ thống xử lý phím thường */
            return NO;
        }
    }

    /* Preedit mode: ranh giới từ → commit ngay */
    if (vnkey_engine_at_word_beginning(_engine)) {
        [self commitPreedit:sender];
    } else {
        [self updatePreedit:sender];
    }

    return YES;
}

- (BOOL)handleBackspace:(id)sender {
    uint8_t outBuf[256];
    size_t actualLen = 0;
    size_t backspaces = 0;

    int processed = vnkey_engine_backspace(
        _engine, outBuf, sizeof(outBuf), &actualLen, &backspaces, NULL);

    if (processed && backspaces > 1) {
        /* Engine yêu cầu xóa nhiều hơn 1 ký tự (undo dấu) */
        removeLastChars(_preedit, backspaces);
        if (actualLen > 0) {
            NSString *output = [[NSString alloc]
                initWithBytes:outBuf
                       length:actualLen
                     encoding:NSUTF8StringEncoding];
            if (output) {
                [_preedit appendString:output];
            }
        }
        [self updatePreedit:sender];
        return YES;
    }

    if (_preedit.length > 0) {
        /* Xóa ký tự cuối preedit */
        removeLastChars(_preedit, 1);
        if (_preedit.length > 0) {
            [self updatePreedit:sender];
        } else {
            /* Preedit rỗng → xóa marked text */
            [sender setMarkedText:@""
                   selectionRange:NSMakeRange(0, 0)
                 replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
        }
        return YES;
    }

    /* Preedit rỗng, để hệ thống xử lý backspace */
    return NO;
}

/* ==================== Preedit management ==================== */

- (void)commitPreedit:(id)sender {
    if (_preedit.length > 0) {
        [sender insertText:_preedit
          replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
        [_preedit setString:@""];
    }
}

- (void)updatePreedit:(id)sender {
    if (_preedit.length == 0) return;

    /* Hiện preedit với gạch chân */
    NSDictionary *attrs = @{
        NSUnderlineStyleAttributeName: @(NSUnderlineStyleSingle),
        NSUnderlineColorAttributeName: [NSColor textColor],
    };
    NSAttributedString *marked = [[NSAttributedString alloc]
        initWithString:_preedit
            attributes:attrs];

    [sender setMarkedText:marked
           selectionRange:NSMakeRange(_preedit.length, 0)
         replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
}

/* ==================== Lifecycle ==================== */

- (void)activateServer:(id)sender {
    [super activateServer:sender];
    vnkey_engine_reset(_engine);
    [_preedit setString:@""];
    loadPreferences(_engine, &_vietMode, &_usePreedit);
}

- (void)deactivateServer:(id)sender {
    [self commitPreedit:sender];
    vnkey_engine_reset(_engine);
    [_preedit setString:@""];
    [super deactivateServer:sender];
}

/* ==================== Menu / Mode ==================== */

- (NSMenu *)menu {
    NSMenu *menu = [[NSMenu alloc] initWithTitle:@"VnKey"];

    /* Bật/tắt tiếng Việt */
    NSMenuItem *toggleItem = [[NSMenuItem alloc]
        initWithTitle:_vietMode ? @"✓ Tiếng Việt" : @"Tiếng Việt"
               action:@selector(toggleVietMode:)
        keyEquivalent:@""];
    toggleItem.target = self;
    [menu addItem:toggleItem];

    [menu addItem:[NSMenuItem separatorItem]];

    /* Kiểu gõ */
    NSArray *methods = @[@"Telex", @"Simple Telex", @"VNI", @"VIQR", @"Microsoft Vietnamese"];
    NSUserDefaults *defaults = [NSUserDefaults standardUserDefaults];
    int currentIM = (int)[defaults integerForKey:kVnKeyInputMethod];
    for (int i = 0; i < (int)methods.count; i++) {
        NSMenuItem *item = [[NSMenuItem alloc]
            initWithTitle:methods[i]
                   action:@selector(selectInputMethod:)
            keyEquivalent:@""];
        item.tag = i;
        item.target = self;
        if (i == currentIM) item.state = NSControlStateValueOn;
        [menu addItem:item];
    }

    [menu addItem:[NSMenuItem separatorItem]];

    /* Cài đặt */
    NSMenuItem *prefsItem = [[NSMenuItem alloc]
        initWithTitle:@"Tùy chỉnh..."
               action:@selector(showPreferences:)
        keyEquivalent:@""];
    prefsItem.target = self;
    [menu addItem:prefsItem];

    return menu;
}

- (void)toggleVietMode:(id)sender {
    _vietMode = !_vietMode;
    vnkey_engine_set_viet_mode(_engine, _vietMode ? 1 : 0);
    vnkey_engine_reset(_engine);
    [_preedit setString:@""];

    NSUserDefaults *defaults = [NSUserDefaults standardUserDefaults];
    [defaults setBool:_vietMode forKey:kVnKeyVietMode];
    [defaults synchronize];
}

- (void)selectInputMethod:(id)sender {
    NSMenuItem *item = (NSMenuItem *)sender;
    int im = (int)item.tag;

    vnkey_engine_set_input_method(_engine, im);
    vnkey_engine_reset(_engine);
    [_preedit setString:@""];

    NSUserDefaults *defaults = [NSUserDefaults standardUserDefaults];
    [defaults setInteger:im forKey:kVnKeyInputMethod];
    [defaults synchronize]; /* Đảm bảo ghi ngay lập tức */

    /* Thông báo các controller khác cập nhật kiểu gõ */
    [[NSNotificationCenter defaultCenter]
        postNotificationName:@"VnKeyPreferencesChanged" object:nil];
}

- (void)showPreferences:(id)sender {
    /* Gửi notification để AppDelegate mở Preferences window */
    [[NSNotificationCenter defaultCenter]
        postNotificationName:@"VnKeyShowPreferences" object:nil];
}

- (void)preferencesChanged:(NSNotification *)note {
    loadPreferences(_engine, &_vietMode, &_usePreedit);
}

@end
