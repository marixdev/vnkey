/*
 * VnKeyEventTap — CGEventTap keyboard hook + text output
 *
 * Kiến trúc giống OpenKey/Gõ Nhanh/PHTV: hook global keyboard qua
 * CGEventTap, xử lý tiếng Việt, gửi kết quả qua CGEvent.
 * Ưu điểm: không gạch chân, tab-completion/autocomplete hoạt động
 * bình thường, gõ tự nhiên như keystroke thật.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#import "VnKeyEventTap.h"
#import "vnkey-engine.h"
#import <Carbon/Carbon.h>
#import <ApplicationServices/ApplicationServices.h>
#import <unistd.h>

/* ==================== State ==================== */

static VnKeyEngine *sEngine = NULL;
static CFMachPortRef sEventTap = NULL;
static CFRunLoopSourceRef sRunLoopSource = NULL;
static BOOL sVietMode = YES;

/* Tag để nhận diện event do chính mình gửi, tránh loop */
static const int64_t kVnKeyEventTag = 0x564E4B59; /* "VNKY" */

/* ==================== Preferences ==================== */

static NSString *const kVnKeyInputMethod   = @"VnKeyInputMethod";
static NSString *const kVnKeyVietMode      = @"VnKeyVietMode";
static NSString *const kVnKeySpellCheck    = @"VnKeySpellCheck";
static NSString *const kVnKeyFreeMarking   = @"VnKeyFreeMarking";
static NSString *const kVnKeyModernStyle   = @"VnKeyModernStyle";
static NSString *const kVnKeyAutoRestore   = @"VnKeyAutoRestore";
static NSString *const kVnKeyEdeMode       = @"VnKeyEdeMode";
static NSString *const kVnKeyMacroEnabled  = @"VnKeyMacroEnabled";
static NSString *const kVnKeyMacros        = @"VnKeyMacros";

static void loadPreferencesIntoEngine(void) {
    if (!sEngine) return;

    NSUserDefaults *defaults = [NSUserDefaults standardUserDefaults];

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
    };
    [defaults registerDefaults:defaultValues];

    int im = (int)[defaults integerForKey:kVnKeyInputMethod];
    sVietMode = [defaults boolForKey:kVnKeyVietMode];
    BOOL spell = [defaults boolForKey:kVnKeySpellCheck];
    BOOL free  = [defaults boolForKey:kVnKeyFreeMarking];
    BOOL modern = [defaults boolForKey:kVnKeyModernStyle];
    BOOL autoRestore = [defaults boolForKey:kVnKeyAutoRestore];
    BOOL ede = [defaults boolForKey:kVnKeyEdeMode];
    BOOL macroEn = [defaults boolForKey:kVnKeyMacroEnabled];

    vnkey_engine_set_input_method(sEngine, im);
    vnkey_engine_set_viet_mode(sEngine, sVietMode ? 1 : 0);
    vnkey_engine_set_options(sEngine, free ? 1 : 0, modern ? 1 : 0,
                            spell ? 1 : 0, autoRestore ? 1 : 0, ede ? 1 : 0,
                            macroEn ? 1 : 0);

    NSString *macros = [defaults stringForKey:kVnKeyMacros];
    if (macros.length > 0) {
        vnkey_engine_load_macros(sEngine, [macros UTF8String]);
    }
}

/* ==================== Text Output ==================== */

/* Micro-delay (µs) giữa backspace và text output.
 * Cần thiết cho Electron-based apps (Zen, VS Code, etc.) nơi backspace
 * event chưa được xử lý xong trước khi text event đến → ký tự thừa.
 * 1000µs (1ms) đủ nhỏ không gây lag, đủ lớn cho event loop xử lý. */
static const useconds_t kBackspaceDelay = 1500;

/* Gửi n backspace qua CGEvent */
static void sendBackspaces(CGEventTapProxy proxy, size_t count) {
    CGEventSourceRef source = CGEventSourceCreate(kCGEventSourceStateCombinedSessionState);
    for (size_t i = 0; i < count; i++) {
        CGEventRef down = CGEventCreateKeyboardEvent(source, kVK_Delete, true);
        CGEventRef up   = CGEventCreateKeyboardEvent(source, kVK_Delete, false);
        CGEventSetIntegerValueField(down, kCGEventSourceUserData, kVnKeyEventTag);
        CGEventSetIntegerValueField(up,   kCGEventSourceUserData, kVnKeyEventTag);
        CGEventTapPostEvent(proxy, down);
        CGEventTapPostEvent(proxy, up);
        CFRelease(down);
        CFRelease(up);
    }
    if (source) CFRelease(source);
    /* Delay sau backspace để app kịp xử lý trước khi text output đến */
    if (count > 0) {
        usleep(kBackspaceDelay);
    }
}

/* Gửi chuỗi Unicode qua CGEventKeyboardSetUnicodeString.
 * Gom tối đa 20 UTF-16 code unit mỗi event (giống XKey/OpenKey). */
static void sendUnicodeString(CGEventTapProxy proxy, NSString *str) {
    if (str.length == 0) return;

    CGEventSourceRef source = CGEventSourceCreate(kCGEventSourceStateCombinedSessionState);
    NSUInteger total = str.length;  /* UTF-16 length */
    NSUInteger sent = 0;
    const NSUInteger kChunkSize = 20;

    while (sent < total) {
        NSUInteger remain = total - sent;
        NSUInteger chunkLen = (remain < kChunkSize) ? remain : kChunkSize;

        unichar buf[20];
        [str getCharacters:buf range:NSMakeRange(sent, chunkLen)];

        CGEventRef down = CGEventCreateKeyboardEvent(source, 0, true);
        CGEventRef up   = CGEventCreateKeyboardEvent(source, 0, false);
        CGEventKeyboardSetUnicodeString(down, (UniCharCount)chunkLen, buf);
        CGEventKeyboardSetUnicodeString(up,   (UniCharCount)chunkLen, buf);
        CGEventSetIntegerValueField(down, kCGEventSourceUserData, kVnKeyEventTag);
        CGEventSetIntegerValueField(up,   kCGEventSourceUserData, kVnKeyEventTag);

        CGEventTapPostEvent(proxy, down);
        CGEventTapPostEvent(proxy, up);
        CFRelease(down);
        CFRelease(up);

        sent += chunkLen;
    }
    if (source) CFRelease(source);
}

/* ==================== Event Callback ==================== */

static CGEventRef eventCallback(CGEventTapProxy proxy,
                                CGEventType type,
                                CGEventRef event,
                                void *refcon)
{
    /* Nếu tap bị disable bởi hệ thống (timeout), bật lại */
    if (type == kCGEventTapDisabledByTimeout ||
        type == kCGEventTapDisabledByUserInput) {
        if (sEventTap) {
            CGEventTapEnable(sEventTap, true);
        }
        return event;
    }

    /* Chỉ xử lý keyDown */
    if (type != kCGEventKeyDown) {
        return event;
    }

    /* Bỏ qua event do chính mình gửi */
    if (CGEventGetIntegerValueField(event, kCGEventSourceUserData) == kVnKeyEventTag) {
        return event;
    }

    /* Không xử lý nếu tắt tiếng Việt */
    if (!sVietMode || !sEngine) {
        return event;
    }

    /* Lấy modifier flags */
    CGEventFlags flags = CGEventGetFlags(event);

    /* Bỏ qua phím có Command hoặc Control */
    if (flags & (kCGEventFlagMaskCommand | kCGEventFlagMaskControl)) {
        vnkey_engine_reset(sEngine);
        return event;
    }

    int64_t keyCode = CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode);

    /* Option+Delete = word-delete trên macOS → reset engine vì
     * macOS xóa cả từ nhưng engine chỉ biết xóa 1 ký tự.
     * Option+Arrow = word-jump → cũng cần reset.
     * CHỈ reset cho các combo cụ thể, KHÔNG reset cho mọi phím có Option
     * vì user có thể chưa nhả Option sau Opt+Backspace → VNI keys bị reset. */
    if (flags & kCGEventFlagMaskAlternate) {
        if (keyCode == kVK_Delete || keyCode == kVK_ForwardDelete ||
            keyCode == kVK_LeftArrow || keyCode == kVK_RightArrow ||
            keyCode == kVK_UpArrow || keyCode == kVK_DownArrow) {
            vnkey_engine_reset(sEngine);
        }
        return event;
    }

    /* Reset engine khi gặp phím đặc biệt */
    if (keyCode == kVK_Escape || keyCode == kVK_Return ||
        keyCode == kVK_Tab ||
        keyCode == kVK_LeftArrow || keyCode == kVK_RightArrow ||
        keyCode == kVK_UpArrow || keyCode == kVK_DownArrow ||
        keyCode == kVK_Home || keyCode == kVK_End ||
        keyCode == kVK_PageUp || keyCode == kVK_PageDown) {
        vnkey_engine_reset(sEngine);
        return event;
    }

    /* Mouse click → reset (detect qua flagsChanged hoặc ở đây bỏ qua) */

    /* Backspace */
    if (keyCode == kVK_Delete) {
        uint8_t outBuf[256];
        size_t actualLen = 0;
        size_t backspaces = 0;

        int processed = vnkey_engine_backspace(
            sEngine, outBuf, sizeof(outBuf), &actualLen, &backspaces, NULL);

        if (processed && backspaces > 1) {
            /* Engine yêu cầu xoá nhiều hơn 1 ký tự (undo dấu):
             * Gửi (backspaces - 1) thêm vì hệ thống đã xử lý 1 backspace,
             * sau đó gửi text thay thế nếu có. */

            /* Để native backspace đi trước (pass-through event gốc),
             * rồi gửi thêm backspace + text */
            sendBackspaces(proxy, backspaces - 1);
            if (actualLen > 0) {
                NSString *output = [[NSString alloc]
                    initWithBytes:outBuf length:actualLen
                         encoding:NSUTF8StringEncoding];
                if (output) {
                    sendUnicodeString(proxy, output);
                }
            }
            /* Pass-through native backspace */
            return event;
        }

        /* Backspace bình thường → để hệ thống xử lý */
        return event;
    }

    /* Space: xử lý macro expansion */
    if (keyCode == kVK_Space) {
        uint8_t outBuf[1024];
        size_t actualLen = 0;
        size_t backspaces = 0;
        int processed = vnkey_engine_process(
            sEngine, (uint32_t)' ',
            outBuf, sizeof(outBuf), &actualLen, &backspaces, NULL);

        if (processed && (backspaces > 0 || actualLen > 0)) {
            /* Macro expanded */
            sendBackspaces(proxy, backspaces);
            if (actualLen > 0) {
                NSString *output = [[NSString alloc]
                    initWithBytes:outBuf length:actualLen
                         encoding:NSUTF8StringEncoding];
                if (output) {
                    sendUnicodeString(proxy, output);
                }
            }
            /* Nuốt phím Space gốc vì đã xử lý */
            return NULL;
        }
        /* Không match macro → soft reset, để Space pass-through */
        vnkey_engine_soft_reset(sEngine);
        return event;
    }

    /* Lấy ký tự Unicode từ event */
    UniChar chars[4] = {0};
    UniCharCount charCount = 0;
    CGEventKeyboardGetUnicodeString(event, 4, &charCount, chars);

    if (charCount == 0) {
        return event;
    }

    unichar ch = chars[0];

    /* Chỉ xử lý ASCII printable (0x21–0x7E) */
    if (ch < 0x21 || ch > 0x7E) {
        vnkey_engine_reset(sEngine);
        return event;
    }

    /* Gửi tới vnkey engine */
    uint8_t outBuf[256];
    size_t actualLen = 0;
    size_t backspaces = 0;

    int processed = vnkey_engine_process(
        sEngine, (uint32_t)ch,
        outBuf, sizeof(outBuf), &actualLen, &backspaces, NULL);

    if (processed && (backspaces > 0 || actualLen > 0)) {
        /* Engine đã xử lý: gửi backspace + text thay thế.
         * Nuốt phím gốc trước (return NULL) để tránh race condition:
         * nếu pass-through event gốc + gửi backspace, một số app (Zen, etc.)
         * xử lý event gốc SAU backspace → ký tự thừa (vd: "dđ" thay vì "đ").
         * Thay vào đó: nuốt phím gốc, gửi backspace xóa ký tự cũ, rồi gửi output. */
        sendBackspaces(proxy, backspaces);
        if (actualLen > 0) {
            NSString *output = [[NSString alloc]
                initWithBytes:outBuf length:actualLen
                     encoding:NSUTF8StringEncoding];
            if (output) {
                sendUnicodeString(proxy, output);
            }
        }
        /* Nuốt phím gốc */
        return NULL;
    }

    /* Engine không xử lý → pass-through */
    return event;
}

/* ==================== Public API ==================== */

BOOL VnKeyEventTapStart(void) {
    if (sEngine) return YES; /* Đã khởi tạo */

    sEngine = vnkey_engine_new();
    if (!sEngine) {
        NSLog(@"VnKey: Không thể tạo engine");
        return NO;
    }

    loadPreferencesIntoEngine();

    /* Tạo CGEventTap — thử HID level trước, fallback session level */
    CGEventMask mask = (1 << kCGEventKeyDown) | (1 << kCGEventKeyUp) |
                       (1 << kCGEventFlagsChanged);

    sEventTap = CGEventTapCreate(
        kCGHIDEventTap,              /* HID level — intercept trước session */
        kCGHeadInsertEventTap,       /* Chèn đầu chuỗi event */
        kCGEventTapOptionDefault,    /* Có thể modify/drop event */
        mask,
        eventCallback,
        NULL);

    if (!sEventTap) {
        /* Fallback sang session level */
        NSLog(@"VnKey: HID tap thất bại, thử session tap");
        sEventTap = CGEventTapCreate(
            kCGSessionEventTap,
            kCGHeadInsertEventTap,
            kCGEventTapOptionDefault,
            mask,
            eventCallback,
            NULL);
    }

    if (!sEventTap) {
        NSLog(@"VnKey: Không thể tạo CGEventTap — cần quyền Accessibility");
        vnkey_engine_free(sEngine);
        sEngine = NULL;
        return NO;
    }

    sRunLoopSource = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, sEventTap, 0);
    CFRunLoopAddSource(CFRunLoopGetMain(), sRunLoopSource, kCFRunLoopCommonModes);
    CGEventTapEnable(sEventTap, true);

    NSLog(@"VnKey: CGEventTap khởi động thành công");
    return YES;
}

void VnKeyEventTapStop(void) {
    if (sRunLoopSource) {
        CFRunLoopRemoveSource(CFRunLoopGetMain(), sRunLoopSource, kCFRunLoopCommonModes);
        CFRelease(sRunLoopSource);
        sRunLoopSource = NULL;
    }
    if (sEventTap) {
        CGEventTapEnable(sEventTap, false);
        CFRelease(sEventTap);
        sEventTap = NULL;
    }
    if (sEngine) {
        vnkey_engine_free(sEngine);
        sEngine = NULL;
    }
    NSLog(@"VnKey: CGEventTap dừng");
}

void VnKeyEventTapSetVietMode(BOOL enabled) {
    sVietMode = enabled;
    if (sEngine) {
        vnkey_engine_set_viet_mode(sEngine, enabled ? 1 : 0);
        vnkey_engine_reset(sEngine);
    }
    [[NSUserDefaults standardUserDefaults] setBool:enabled forKey:kVnKeyVietMode];
    [[NSUserDefaults standardUserDefaults] synchronize];
}

BOOL VnKeyEventTapGetVietMode(void) {
    return sVietMode;
}

void VnKeyEventTapReloadPreferences(void) {
    loadPreferencesIntoEngine();
}
