/*
 * vnkey-macos — Bộ gõ tiếng Việt cho macOS
 * Sử dụng Input Method Kit (IMKit) + vnkey-engine (Rust FFI)
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#import <Cocoa/Cocoa.h>
#import <InputMethodKit/InputMethodKit.h>

IMKServer *gIMKServer = nil;

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        /* Khởi tạo NSApplication trước IMKServer */
        [NSApplication sharedApplication];

        /* Tạo IMKServer với connection name khớp Info.plist */
        NSString *connectionName = [[NSBundle mainBundle]
            objectForInfoDictionaryKey:@"InputMethodConnectionName"];
        gIMKServer = [[IMKServer alloc]
            initWithName:connectionName
        bundleIdentifier:[[NSBundle mainBundle] bundleIdentifier]];

        if (!gIMKServer) {
            NSLog(@"VnKey: Không thể tạo IMKServer");
            return 1;
        }

        NSLog(@"VnKey: Input method server khởi động (%@)", connectionName);

        /* Chạy vòng lặp sự kiện */
        [NSApp run];
    }
    return 0;
}
