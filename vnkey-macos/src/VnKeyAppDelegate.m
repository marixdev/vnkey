/*
 * VnKeyAppDelegate — NSStatusItem + CGEventTap lifecycle
 *
 * Kiến trúc:
 * 1. applicationDidFinishLaunching: kiểm tra Accessibility → khởi tạo event tap
 * 2. NSStatusItem hiển thị "Vi"/"En" trên menu bar
 * 3. Menu: Toggle Việt/Anh, chọn kiểu gõ, Preferences, Quit
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#import "VnKeyAppDelegate.h"
#import "VnKeyEventTap.h"
#import "VnKeyPreferences.h"
#import <ApplicationServices/ApplicationServices.h>

/* Notification khác module gửi đến */
static NSString *const kVnKeyPreferencesChanged = @"VnKeyPreferencesChanged";

/* Input method names khớp với VnKeyPreferences popup */
static NSArray<NSString *> *inputMethodNames(void) {
    return @[@"Telex", @"Simple Telex", @"VNI", @"VIQR", @"Microsoft Vietnamese"];
}

@implementation VnKeyAppDelegate {
    NSMenu *_statusMenu;
    NSMenuItem *_toggleItem;
    NSArray<NSMenuItem *> *_imItems;
}

/* ==================== App Lifecycle ==================== */

- (void)applicationDidFinishLaunching:(NSNotification *)notification {
    /* Lắng nghe notification */
    NSNotificationCenter *nc = [NSNotificationCenter defaultCenter];
    [nc addObserver:self selector:@selector(preferencesChanged:)
               name:kVnKeyPreferencesChanged object:nil];

    /* Kiểm tra & yêu cầu quyền Accessibility */
    if (![self ensureAccessibility]) {
        /* Hiện alert hướng dẫn, nhưng vẫn tiếp tục (event tap sẽ retry) */
        [self showAccessibilityAlert];
    }

    /* Khởi tạo CGEventTap */
    if (!VnKeyEventTapStart()) {
        NSLog(@"VnKey: CGEventTap khởi tạo thất bại — chờ cấp quyền Accessibility");
        /* Retry sau 3 giây (user có thể đang cấp quyền) */
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 3 * NSEC_PER_SEC),
                       dispatch_get_main_queue(), ^{
            if (!VnKeyEventTapStart()) {
                NSLog(@"VnKey: Retry CGEventTap thất bại");
            } else {
                [self updateStatusIcon];
            }
        });
    }

    /* Tạo menu bar icon */
    [self setupStatusItem];

    NSLog(@"VnKey: App delegate khởi động (CGEventTap mode)");
}

- (void)applicationWillTerminate:(NSNotification *)notification {
    VnKeyEventTapStop();
    [[NSNotificationCenter defaultCenter] removeObserver:self];
}

/* ==================== Accessibility ==================== */

- (BOOL)ensureAccessibility {
    NSDictionary *opts = @{(__bridge NSString *)kAXTrustedCheckOptionPrompt: @YES};
    return AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)opts);
}

- (void)showAccessibilityAlert {
    NSAlert *alert = [[NSAlert alloc] init];
    alert.messageText = @"VnKey cần quyền Accessibility";
    alert.informativeText = @"Vào System Settings → Privacy & Security → "
        @"Accessibility → bật VnKey.\n\nSau khi bật, VnKey sẽ tự hoạt động.";
    alert.alertStyle = NSAlertStyleWarning;
    [alert addButtonWithTitle:@"Mở System Settings"];
    [alert addButtonWithTitle:@"Để sau"];

    NSModalResponse response = [alert runModal];
    if (response == NSAlertFirstButtonReturn) {
        [self openAccessibilitySettings:nil];
    }
}

- (void)openAccessibilitySettings:(id)sender {
    NSURL *url = [NSURL URLWithString:
        @"x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"];
    [[NSWorkspace sharedWorkspace] openURL:url];
}

/* ==================== Status Item ==================== */

- (void)setupStatusItem {
    self.statusItem = [[NSStatusBar systemStatusBar]
        statusItemWithLength:NSVariableStatusItemLength];

    [self updateStatusIcon];

    /* Build menu */
    _statusMenu = [[NSMenu alloc] init];

    /* Toggle Việt/Anh */
    _toggleItem = [[NSMenuItem alloc]
        initWithTitle:@"Chuyển sang Tiếng Anh"
               action:@selector(toggleVietMode:)
        keyEquivalent:@""];
    _toggleItem.target = self;
    [_statusMenu addItem:_toggleItem];

    [_statusMenu addItem:[NSMenuItem separatorItem]];

    /* Kiểu gõ */
    NSArray *names = inputMethodNames();
    NSInteger currentIM = [[NSUserDefaults standardUserDefaults]
        integerForKey:@"VnKeyInputMethod"];
    NSMutableArray *imItems = [NSMutableArray array];
    for (NSUInteger i = 0; i < names.count; i++) {
        NSMenuItem *item = [[NSMenuItem alloc]
            initWithTitle:names[i]
                   action:@selector(selectInputMethod:)
            keyEquivalent:@""];
        item.target = self;
        item.tag = (NSInteger)i;
        if ((NSInteger)i == currentIM) {
            item.state = NSControlStateValueOn;
        }
        [_statusMenu addItem:item];
        [imItems addObject:item];
    }
    _imItems = [imItems copy];

    [_statusMenu addItem:[NSMenuItem separatorItem]];

    /* Tùy chỉnh */
    NSMenuItem *prefsItem = [[NSMenuItem alloc]
        initWithTitle:@"Tùy chỉnh..."
               action:@selector(showPreferences:)
        keyEquivalent:@","];
    prefsItem.target = self;
    [_statusMenu addItem:prefsItem];

    /* Cấp quyền Accessibility */
    NSMenuItem *accessItem = [[NSMenuItem alloc]
        initWithTitle:@"Cấp quyền Accessibility..."
               action:@selector(openAccessibilitySettings:)
        keyEquivalent:@""];
    accessItem.target = self;
    [_statusMenu addItem:accessItem];

    [_statusMenu addItem:[NSMenuItem separatorItem]];

    /* Thoát */
    NSMenuItem *quitItem = [[NSMenuItem alloc]
        initWithTitle:@"Thoát VnKey"
               action:@selector(quit:)
        keyEquivalent:@"q"];
    quitItem.target = self;
    [_statusMenu addItem:quitItem];

    self.statusItem.menu = _statusMenu;
}

- (void)updateStatusIcon {
    BOOL viet = VnKeyEventTapGetVietMode();

    /* Text-based icon: "Vi" hoặc "En" */
    self.statusItem.button.title = viet ? @"Vi" : @"En";
    self.statusItem.button.font = [NSFont monospacedSystemFontOfSize:14
                                                              weight:NSFontWeightMedium];

    /* Update toggle menu item */
    _toggleItem.title = viet
        ? @"Chuyển sang Tiếng Anh"
        : @"Chuyển sang Tiếng Việt";
}

/* ==================== Menu Actions ==================== */

- (void)toggleVietMode:(id)sender {
    BOOL newMode = !VnKeyEventTapGetVietMode();
    VnKeyEventTapSetVietMode(newMode);
    [self updateStatusIcon];
}

- (void)selectInputMethod:(id)sender {
    NSMenuItem *item = (NSMenuItem *)sender;
    NSInteger idx = item.tag;

    [[NSUserDefaults standardUserDefaults] setInteger:idx forKey:@"VnKeyInputMethod"];
    [[NSUserDefaults standardUserDefaults] synchronize];

    /* Update checkmarks */
    for (NSMenuItem *mi in _imItems) {
        mi.state = (mi.tag == idx)
            ? NSControlStateValueOn
            : NSControlStateValueOff;
    }

    /* Reload engine */
    VnKeyEventTapReloadPreferences();
}

- (void)showPreferences:(id)sender {
    if (self.preferencesWindow && self.preferencesWindow.isVisible) {
        [self.preferencesWindow makeKeyAndOrderFront:nil];
        [NSApp activateIgnoringOtherApps:YES];
        return;
    }

    self.preferencesWindow = createPreferencesWindow();
    [self.preferencesWindow makeKeyAndOrderFront:nil];
    [NSApp activateIgnoringOtherApps:YES];
}

- (void)quit:(id)sender {
    VnKeyEventTapStop();
    [NSApp terminate:nil];
}

/* ==================== Notifications ==================== */

- (void)preferencesChanged:(NSNotification *)note {
    VnKeyEventTapReloadPreferences();

    /* Cập nhật checkmark kiểu gõ */
    NSInteger currentIM = [[NSUserDefaults standardUserDefaults]
        integerForKey:@"VnKeyInputMethod"];
    for (NSMenuItem *mi in _imItems) {
        mi.state = (mi.tag == currentIM)
            ? NSControlStateValueOn
            : NSControlStateValueOff;
    }
}

@end
