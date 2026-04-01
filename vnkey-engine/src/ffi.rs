//! Tầng FFI tương thích C cho tích hợp đa nền tảng.
//! Xuất engine dưới dạng thư viện chia sẻ C (cdylib).
//!
//! Hai API:
//! - Singleton toàn cục (vnkey_setup/vnkey_process/...) cho dùng đơn giản
//! - Dạng instance (vnkey_engine_new/vnkey_engine_process/...) cho IME framework

use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr;
use std::sync::Mutex;

use crate::engine::Engine;
use crate::input::InputMethod;
use crate::charset::{self, Charset};

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);

fn with_engine<F, R>(f: F) -> R
where
    F: FnOnce(&mut Engine) -> R,
    R: Default,
{
    let mut guard = match ENGINE.lock() {
        Ok(g) => g,
        Err(_) => return R::default(),
    };
    if guard.is_none() {
        *guard = Some(Engine::new());
    }
    f(guard.as_mut().unwrap())
}

/// Khởi tạo engine. Gọi một lần khi khởi động.
#[no_mangle]
pub extern "C" fn vnkey_setup() {
    let mut guard = ENGINE.lock().unwrap();
    *guard = Some(Engine::new());
}

/// Tắt engine và giải phóng tài nguyên.
#[no_mangle]
pub extern "C" fn vnkey_cleanup() {
    let mut guard = ENGINE.lock().unwrap();
    *guard = None;
}

/// Xử lý một lần gõ phím. Trả số backspace cần thiết.
/// Byte UTF-8 đầu ra ghi vào `out_buf` (tối đa `out_len` byte).
/// Độ dài thực tế ghi vào `actual_len`.
/// Trả 1 nếu phím được xử lý, 0 nếu không.
#[no_mangle]
pub extern "C" fn vnkey_process(
    key_code: u32,
    out_buf: *mut u8,
    out_len: usize,
    actual_len: *mut usize,
    backspaces: *mut usize,
    backspaces_bytes: *mut usize,
) -> i32 {
    with_engine(|engine| {
        let result = engine.process(key_code);
        if !backspaces.is_null() {
            unsafe { *backspaces = result.backspaces; }
        }
        if !backspaces_bytes.is_null() {
            unsafe { *backspaces_bytes = result.backspaces_bytes; }
        }
        if !actual_len.is_null() {
            let copy_len = result.output.len().min(out_len);
            if !out_buf.is_null() && copy_len > 0 {
                unsafe {
                    ptr::copy_nonoverlapping(result.output.as_ptr(), out_buf, copy_len);
                }
            }
            unsafe { *actual_len = copy_len; }
        }
        if result.processed { 1 } else { 0 }
    })
}

/// Xử lý phím backspace. Trả 1 nếu xử lý, 0 nếu không.
#[no_mangle]
pub extern "C" fn vnkey_backspace(
    out_buf: *mut u8,
    out_len: usize,
    actual_len: *mut usize,
    backspaces: *mut usize,
    backspaces_bytes: *mut usize,
) -> i32 {
    with_engine(|engine| {
        let result = engine.process_backspace();
        if !backspaces.is_null() {
            unsafe { *backspaces = result.backspaces; }
        }
        if !backspaces_bytes.is_null() {
            unsafe { *backspaces_bytes = result.backspaces_bytes; }
        }
        if !actual_len.is_null() {
            let copy_len = result.output.len().min(out_len);
            if !out_buf.is_null() && copy_len > 0 {
                unsafe {
                    ptr::copy_nonoverlapping(result.output.as_ptr(), out_buf, copy_len);
                }
            }
            unsafe { *actual_len = copy_len; }
        }
        if result.processed { 1 } else { 0 }
    })
}

/// Đặt lại trạng thái engine.
#[no_mangle]
pub extern "C" fn vnkey_reset() {
    with_engine(|engine| engine.reset());
}

/// Đặt kiểu gõ.
/// 0 = Telex, 1 = SimpleTelex, 2 = VNI, 3 = VIQR, 4 = MsVi
#[no_mangle]
pub extern "C" fn vnkey_set_input_method(method: i32) {
    let im = match method {
        0 => InputMethod::Telex,
        1 => InputMethod::SimpleTelex,
        2 => InputMethod::Vni,
        3 => InputMethod::Viqr,
        4 => InputMethod::MsVi,
        _ => return,
    };
    with_engine(|engine| engine.set_input_method(im));
}

/// Bật/tắt chế độ tiếng Việt.
#[no_mangle]
pub extern "C" fn vnkey_set_viet_mode(enabled: i32) {
    with_engine(|engine| engine.set_viet_mode(enabled != 0));
}

/// Đặt tùy chọn engine.
#[no_mangle]
pub extern "C" fn vnkey_set_options(
    free_marking: i32,
    modern_style: i32,
    spell_check: i32,
    auto_restore: i32,
) {
    with_engine(|engine| {
        engine.options.free_marking = free_marking != 0;
        engine.options.modern_style = modern_style != 0;
        engine.options.spell_check_enabled = spell_check != 0;
        engine.options.auto_non_vn_restore = auto_restore != 0;
    });
}

/// Kiểm tra đang ở đầu từ
#[no_mangle]
pub extern "C" fn vnkey_at_word_beginning() -> i32 {
    with_engine(|engine| if engine.at_word_beginning() { 1 } else { 0 })
}

/// Thêm macro. Trả 1 nếu thành công, 0 nếu thất bại.
#[no_mangle]
pub extern "C" fn vnkey_add_macro(key: *const c_char, value: *const c_char) -> i32 {
    if key.is_null() || value.is_null() {
        return 0;
    }
    let key_str = unsafe { CStr::from_ptr(key) };
    let value_str = unsafe { CStr::from_ptr(value) };
    let key_str = match key_str.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let value_str = match value_str.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    with_engine(|engine| if engine.macro_table.add(key_str, value_str) { 1 } else { 0 })
}

/// Xóa tất cả macro.
#[no_mangle]
pub extern "C" fn vnkey_clear_macros() {
    with_engine(|engine| engine.macro_table.clear());
}

// ==================== API dạng instance ====================
// Dùng cho IME framework (Fcitx5, IBus) quản lý nhiều ngữ cảnh.

/// Handle engine opaque cho caller C.
pub type VnKeyEngine = Engine;

/// Tạo instance engine mới. Caller phải giải phóng bằng vnkey_engine_free.
#[no_mangle]
pub extern "C" fn vnkey_engine_new() -> *mut VnKeyEngine {
    Box::into_raw(Box::new(Engine::new()))
}

/// Giải phóng instance engine.
/// # Safety
/// `engine` phải là con trỏ hợp lệ từ `vnkey_engine_new`, hoặc null.
#[no_mangle]
pub unsafe extern "C" fn vnkey_engine_free(engine: *mut VnKeyEngine) {
    if !engine.is_null() {
        drop(Box::from_raw(engine));
    }
}

/// Xử lý phím trên instance engine cụ thể.
/// Trả 1 nếu xử lý, 0 nếu không.
/// # Safety
/// `engine` phải là con trỏ hợp lệ từ `vnkey_engine_new`.
#[no_mangle]
pub unsafe extern "C" fn vnkey_engine_process(
    engine: *mut VnKeyEngine,
    key_code: u32,
    out_buf: *mut u8,
    out_len: usize,
    actual_len: *mut usize,
    backspaces: *mut usize,
    backspaces_bytes: *mut usize,
) -> i32 {
    if engine.is_null() {
        return 0;
    }
    let engine = &mut *engine;
    let result = engine.process(key_code);
    if !backspaces.is_null() {
        *backspaces = result.backspaces;
    }
    if !backspaces_bytes.is_null() {
        *backspaces_bytes = result.backspaces_bytes;
    }
    if !actual_len.is_null() {
        let copy_len = result.output.len().min(out_len);
        if !out_buf.is_null() && copy_len > 0 {
            ptr::copy_nonoverlapping(result.output.as_ptr(), out_buf, copy_len);
        }
        *actual_len = copy_len;
    }
    if result.processed { 1 } else { 0 }
}

/// Xử lý backspace trên instance engine cụ thể.
/// # Safety
/// `engine` phải là con trỏ hợp lệ từ `vnkey_engine_new`.
#[no_mangle]
pub unsafe extern "C" fn vnkey_engine_backspace(
    engine: *mut VnKeyEngine,
    out_buf: *mut u8,
    out_len: usize,
    actual_len: *mut usize,
    backspaces: *mut usize,
    backspaces_bytes: *mut usize,
) -> i32 {
    if engine.is_null() {
        return 0;
    }
    let engine = &mut *engine;
    let result = engine.process_backspace();
    if !backspaces.is_null() {
        *backspaces = result.backspaces;
    }
    if !backspaces_bytes.is_null() {
        *backspaces_bytes = result.backspaces_bytes;
    }
    if !actual_len.is_null() {
        let copy_len = result.output.len().min(out_len);
        if !out_buf.is_null() && copy_len > 0 {
            ptr::copy_nonoverlapping(result.output.as_ptr(), out_buf, copy_len);
        }
        *actual_len = copy_len;
    }
    if result.processed { 1 } else { 0 }
}

/// Đặt lại instance engine cụ thể.
/// # Safety
/// `engine` phải là con trỏ hợp lệ từ `vnkey_engine_new`.
#[no_mangle]
pub unsafe extern "C" fn vnkey_engine_reset(engine: *mut VnKeyEngine) {
    if !engine.is_null() {
        (*engine).reset();
    }
}

/// Soft reset: lưu trạng thái để backspace sau dấu cách có thể khôi phục từ đã gõ.
/// Gọi khi xử lý phím Space thay vì reset().
/// # Safety
/// `engine` phải là con trỏ hợp lệ từ `vnkey_engine_new`.
#[no_mangle]
pub unsafe extern "C" fn vnkey_engine_soft_reset(engine: *mut VnKeyEngine) {
    if !engine.is_null() {
        (*engine).soft_reset();
    }
}

/// Nạp ngữ cảnh (surrounding text) vào engine để khôi phục trạng thái.
/// `text` là chuỗi UTF-8 kết thúc null chứa phần cuối từ đã commit
/// (vd: "g" nếu từ "giá" bị xoá còn "g").
/// Trả 1 nếu nạp thành công, 0 nếu thất bại.
/// # Safety
/// `engine` phải là con trỏ hợp lệ từ `vnkey_engine_new`.
/// `text` phải là chuỗi C hợp lệ kết thúc null.
#[no_mangle]
pub unsafe extern "C" fn vnkey_engine_feed_context(
    engine: *mut VnKeyEngine,
    text: *const c_char,
) -> i32 {
    if engine.is_null() || text.is_null() {
        return 0;
    }
    let engine = &mut *engine;
    let c_str = CStr::from_ptr(text);
    match c_str.to_str() {
        Ok(s) => if engine.feed_context(s) { 1 } else { 0 },
        Err(_) => 0,
    }
}

/// Đặt kiểu gõ cho instance engine cụ thể.
/// 0 = Telex, 1 = SimpleTelex, 2 = VNI, 3 = VIQR, 4 = MsVi
/// # Safety
/// `engine` phải là con trỏ hợp lệ từ `vnkey_engine_new`.
#[no_mangle]
pub unsafe extern "C" fn vnkey_engine_set_input_method(engine: *mut VnKeyEngine, method: i32) {
    if engine.is_null() {
        return;
    }
    let im = match method {
        0 => InputMethod::Telex,
        1 => InputMethod::SimpleTelex,
        2 => InputMethod::Vni,
        3 => InputMethod::Viqr,
        4 => InputMethod::MsVi,
        _ => return,
    };
    (*engine).set_input_method(im);
}

/// Bật/tắt tiếng Việt trên instance engine cụ thể.
/// # Safety
/// `engine` phải là con trỏ hợp lệ từ `vnkey_engine_new`.
#[no_mangle]
pub unsafe extern "C" fn vnkey_engine_set_viet_mode(engine: *mut VnKeyEngine, enabled: i32) {
    if !engine.is_null() {
        (*engine).set_viet_mode(enabled != 0);
    }
}

/// Đặt tùy chọn cho instance cụ thể.
/// # Safety
/// `engine` phải là con trỏ hợp lệ từ `vnkey_engine_new`.
#[no_mangle]
pub unsafe extern "C" fn vnkey_engine_set_options(
    engine: *mut VnKeyEngine,
    free_marking: i32,
    modern_style: i32,
    spell_check: i32,
    auto_restore: i32,
) {
    if engine.is_null() {
        return;
    }
    let e = &mut *engine;
    e.options.free_marking = free_marking != 0;
    e.options.modern_style = modern_style != 0;
    e.options.spell_check_enabled = spell_check != 0;
    e.options.auto_non_vn_restore = auto_restore != 0;
}

/// Kiểm tra engine đang ở đầu từ.
/// # Safety
/// `engine` phải là con trỏ hợp lệ từ `vnkey_engine_new`.
#[no_mangle]
pub unsafe extern "C" fn vnkey_engine_at_word_beginning(engine: *mut VnKeyEngine) -> i32 {
    if engine.is_null() {
        return 1;
    }
    if (*engine).at_word_beginning() { 1 } else { 0 }
}

// ==================== Chuyển đổi bảng mã ====================

fn charset_from_id(id: i32) -> Option<Charset> {
    match id {
        0 => Some(Charset::Unicode),
        1 => Some(Charset::Utf8),
        2 => Some(Charset::NcrDec),
        3 => Some(Charset::NcrHex),
        4 => Some(Charset::UniDecomposed),
        5 => Some(Charset::WinCP1258),
        6 => Some(Charset::UniCString),
        10 => Some(Charset::Viqr),
        11 => Some(Charset::Utf8Viqr),
        20 => Some(Charset::Tcvn3),
        21 => Some(Charset::Vps),
        22 => Some(Charset::Viscii),
        23 => Some(Charset::Bkhcm1),
        24 => Some(Charset::VietwareF),
        25 => Some(Charset::Isc),
        40 => Some(Charset::VniWin),
        41 => Some(Charset::Bkhcm2),
        42 => Some(Charset::VietwareX),
        43 => Some(Charset::VniMac),
        _ => None,
    }
}

/// Chuyển văn bản UTF-8 sang bảng mã đích.
/// Trả 0 nếu thành công, -1 nếu lỗi.
/// # Safety
/// Tất cả tham số con trỏ phải hợp lệ.
#[no_mangle]
pub unsafe extern "C" fn vnkey_charset_from_utf8(
    input: *const u8,
    input_len: usize,
    charset_id: i32,
    out_buf: *mut u8,
    out_len: usize,
    actual_len: *mut usize,
) -> i32 {
    if input.is_null() || out_buf.is_null() || actual_len.is_null() {
        return -1;
    }
    let cs = match charset_from_id(charset_id) {
        Some(cs) => cs,
        None => return -1,
    };
    let input_slice = std::slice::from_raw_parts(input, input_len);
    let input_str = match std::str::from_utf8(input_slice) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    if cs == Charset::Utf8 {
        let copy_len = input_len.min(out_len);
        ptr::copy_nonoverlapping(input, out_buf, copy_len);
        *actual_len = copy_len;
        return 0;
    }
    match charset::from_utf8(input_str, cs) {
        Ok(converted) => {
            let copy_len = converted.len().min(out_len);
            ptr::copy_nonoverlapping(converted.as_ptr(), out_buf, copy_len);
            *actual_len = copy_len;
            0
        }
        Err(_) => -1,
    }
}

/// Chuyển byte từ bảng mã nguồn sang UTF-8.
/// Trả 0 nếu thành công, -1 nếu lỗi.
/// # Safety
/// Tất cả tham số con trỏ phải hợp lệ.
#[no_mangle]
pub unsafe extern "C" fn vnkey_charset_to_utf8(
    input: *const u8,
    input_len: usize,
    charset_id: i32,
    out_buf: *mut u8,
    out_len: usize,
    actual_len: *mut usize,
) -> i32 {
    if input.is_null() || out_buf.is_null() || actual_len.is_null() {
        return -1;
    }
    let cs = match charset_from_id(charset_id) {
        Some(cs) => cs,
        None => return -1,
    };
    let input_slice = std::slice::from_raw_parts(input, input_len);
    match charset::to_utf8(input_slice, cs) {
        Ok(utf8_string) => {
            let bytes = utf8_string.as_bytes();
            let copy_len = bytes.len().min(out_len);
            ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, copy_len);
            *actual_len = copy_len;
            0
        }
        Err(_) => -1,
    }
}

// ==================== App Charset (per-app charset mapping) ====================

/// Nạp cấu hình app_charset từ chuỗi JSON.
/// Ví dụ: {"wps.exe": 20, "myapp.exe": 23}
/// # Safety
/// `json` phải là chuỗi C hợp lệ kết thúc null.
#[no_mangle]
pub unsafe extern "C" fn vnkey_app_charset_from_json(json: *const c_char) {
    if json.is_null() { return; }
    let c_str = CStr::from_ptr(json);
    if let Ok(s) = c_str.to_str() {
        crate::app_charset::from_json(s);
    }
}

/// Xuất cấu hình app_charset ra chuỗi JSON.
/// Trả chuỗi malloc, caller phải giải phóng bằng vnkey_app_charset_free_string.
#[no_mangle]
pub extern "C" fn vnkey_app_charset_to_json() -> *mut c_char {
    let json = crate::app_charset::to_json();
    match std::ffi::CString::new(json) {
        Ok(cs) => cs.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Giải phóng chuỗi trả về từ vnkey_app_charset_to_json.
/// # Safety
/// `s` phải là con trỏ từ vnkey_app_charset_to_json, hoặc null.
#[no_mangle]
pub unsafe extern "C" fn vnkey_app_charset_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(std::ffi::CString::from_raw(s));
    }
}

/// Cập nhật charset override cho app hiện tại.
/// Truyền tên exe (lowercase, UTF-8, kết thúc null), hoặc null nếu không xác định.
/// # Safety
/// `exe_name` phải là chuỗi C hợp lệ kết thúc null, hoặc null.
#[no_mangle]
pub unsafe extern "C" fn vnkey_app_charset_update(exe_name: *const c_char) {
    let name = if exe_name.is_null() {
        None
    } else {
        CStr::from_ptr(exe_name).to_str().ok()
    };
    crate::app_charset::update_app_charset_for(name);
}

/// Lấy charset override cho app hiện tại.
/// Trả charset_id nếu có override, -1 nếu dùng mặc định.
#[no_mangle]
pub extern "C" fn vnkey_app_charset_get_current() -> i32 {
    crate::app_charset::get_current_app_charset().unwrap_or(-1)
}
