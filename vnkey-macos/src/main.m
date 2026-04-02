/*
 * vnkey-macos — Bộ gõ tiếng Việt cho macOS
 * Sử dụng CGEventTap + vnkey-engine (Rust FFI)
 *
 * Kiến trúc: CGEventTap hook keyboard → vnkey-engine xử lý →
 * CGEvent backspace + Unicode text output. Không dùng IMKit
 * → không gạch chân, tab/autocomplete hoạt động bình thường.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#import <Cocoa/Cocoa.h>
#import "VnKeyAppDelegate.h"

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        [NSApplication sharedApplication];

        VnKeyAppDelegate *delegate = [[VnKeyAppDelegate alloc] init];
        [NSApp setDelegate:delegate];

        NSLog(@"VnKey: Khởi động (CGEventTap mode)");

        [NSApp run];
    }
    return 0;
}
