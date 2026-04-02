/*
 * VnKeyInputController — IMKInputController xử lý phím tiếng Việt
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#import <Cocoa/Cocoa.h>
#import <InputMethodKit/InputMethodKit.h>

@interface VnKeyInputController : IMKInputController {
    /* Preedit buffer (UTF-8 → NSString) */
    NSMutableString *_preedit;
    /* Con trỏ engine */
    void *_engine;
    /* Chế độ tiếng Việt */
    BOOL _vietMode;
    /* Sử dụng preedit (gạch chân) hay commit trực tiếp */
    BOOL _usePreedit;
}

- (void)commitPreedit:(id)sender;
- (void)updatePreedit:(id)sender;

@end
