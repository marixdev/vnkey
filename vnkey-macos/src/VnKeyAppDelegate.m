/*
 * VnKeyAppDelegate — Quản lý vòng đời app và cửa sổ Preferences
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#import "VnKeyAppDelegate.h"
#import "VnKeyPreferences.h"

@implementation VnKeyAppDelegate

- (void)applicationDidFinishLaunching:(NSNotification *)notification {
    /* Lắng nghe yêu cầu mở Preferences từ input controller menu */
    [[NSNotificationCenter defaultCenter]
        addObserver:self
           selector:@selector(showPreferences:)
               name:@"VnKeyShowPreferences"
             object:nil];

    NSLog(@"VnKey: App delegate khởi động");
}

- (void)applicationWillTerminate:(NSNotification *)notification {
    [[NSNotificationCenter defaultCenter] removeObserver:self];
}

- (void)showPreferences:(NSNotification *)note {
    if (self.preferencesWindow && self.preferencesWindow.isVisible) {
        [self.preferencesWindow makeKeyAndOrderFront:nil];
        return;
    }

    self.preferencesWindow = createPreferencesWindow();
    [self.preferencesWindow makeKeyAndOrderFront:nil];

    /* Đưa app lên foreground (vì là background-only app) */
    [NSApp activateIgnoringOtherApps:YES];
}

@end
