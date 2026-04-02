/*
 * VnKeyAppDelegate — NSApplicationDelegate cho VnKey (CGEventTap)
 *
 * Menu bar icon (NSStatusItem) hiển thị Vi/En, quản lý event tap,
 * kiểm tra quyền Accessibility, cửa sổ Preferences.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#import <Cocoa/Cocoa.h>

@interface VnKeyAppDelegate : NSObject <NSApplicationDelegate>

@property (nonatomic, strong) NSStatusItem *statusItem;
@property (nonatomic, strong) NSWindow *preferencesWindow;

@end
