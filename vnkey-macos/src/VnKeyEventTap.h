/*
 * VnKeyEventTap — CGEventTap keyboard hook + text output
 *
 * Intercept keyDown → xử lý qua vnkey-engine → gửi backspace + text
 * qua CGEvent. Không dùng IMKit → không gạch chân, không ảnh hưởng
 * tab-completion / autocomplete.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#import <Cocoa/Cocoa.h>

/* Khởi tạo engine + tạo CGEventTap. Trả YES nếu thành công. */
BOOL VnKeyEventTapStart(void);

/* Huỷ CGEventTap và giải phóng engine. */
void VnKeyEventTapStop(void);

/* Bật/tắt chế độ tiếng Việt */
void VnKeyEventTapSetVietMode(BOOL enabled);
BOOL VnKeyEventTapGetVietMode(void);

/* Nạp lại preferences vào engine */
void VnKeyEventTapReloadPreferences(void);
