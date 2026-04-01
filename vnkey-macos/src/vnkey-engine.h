/* Header C cho FFI vnkey-engine (API theo instance) */
#ifndef VNKEY_ENGINE_H
#define VNKEY_ENGINE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Handle engine opaque */
typedef struct VnKeyEngine VnKeyEngine;

/* Tạo/hủy */
VnKeyEngine *vnkey_engine_new(void);
void vnkey_engine_free(VnKeyEngine *engine);

/* Xử lý phím. Trả 1 nếu xử lý, 0 nếu bỏ qua.
 * out_buf: nhận byte đầu ra UTF-8
 * out_len: dung lượng out_buf
 * actual_len: nhận số byte thực tế ghi
 * backspaces: nhận số backspace cần gửi trước đầu ra
 * backspaces_bytes: nhận số backspace tính theo byte cho bảng mã đa byte (có thể NULL) */
int vnkey_engine_process(VnKeyEngine *engine, uint32_t key_code,
                        uint8_t *out_buf, size_t out_len,
                        size_t *actual_len, size_t *backspaces,
                        size_t *backspaces_bytes);

/* Xử lý backspace. Ngữ nghĩa đầu ra giống vnkey_engine_process. */
int vnkey_engine_backspace(VnKeyEngine *engine,
                          uint8_t *out_buf, size_t out_len,
                          size_t *actual_len, size_t *backspaces,
                          size_t *backspaces_bytes);

/* Đặt lại trạng thái engine (vd: khi đổi focus) */
void vnkey_engine_reset(VnKeyEngine *engine);

/* Đặt lại mềm: lưu trạng thái để backspace có thể khôi phục */
void vnkey_engine_soft_reset(VnKeyEngine *engine);

/* Nạp ngữ cảnh (surrounding text) vào engine để khôi phục trạng thái. */
int vnkey_engine_feed_context(VnKeyEngine *engine, const char *text);

/* Đặt kiểu gõ: 0=Telex, 1=SimpleTelex, 2=VNI, 3=VIQR, 4=MsVi */
void vnkey_engine_set_input_method(VnKeyEngine *engine, int method);

/* Bật(1) / tắt(0) chế độ tiếng Việt */
void vnkey_engine_set_viet_mode(VnKeyEngine *engine, int enabled);

/* Đặt tùy chọn engine */
void vnkey_engine_set_options(VnKeyEngine *engine,
                             int free_marking, int modern_style,
                             int spell_check, int auto_restore);

/* Trả 1 nếu ở đầu từ */
int vnkey_engine_at_word_beginning(VnKeyEngine *engine);

/* Chuyển UTF-8 sang bảng mã đích. */
int vnkey_charset_from_utf8(const uint8_t *input, size_t input_len,
                           int charset_id,
                           uint8_t *out_buf, size_t out_len,
                           size_t *actual_len);

/* Chuyển byte từ bảng mã nguồn sang UTF-8. */
int vnkey_charset_to_utf8(const uint8_t *input, size_t input_len,
                         int charset_id,
                         uint8_t *out_buf, size_t out_len,
                         size_t *actual_len);

#ifdef __cplusplus
}
#endif

#endif /* VNKEY_ENGINE_H */
