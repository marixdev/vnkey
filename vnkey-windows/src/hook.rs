//! Hook bàn phím cấp thấp để chặn phím toàn cục

use crate::{ENGINE, WM_VNKEY_UPDATE_ICON, VNKEY_INJECTED_TAG};

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64, Ordering};
use vnkey_engine::charset::Charset;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// HHOOK bọc con trỏ thô. Lưu giá trị isize trực tiếp để an toàn luồng.
static HOOK_RAW: AtomicIsize = AtomicIsize::new(0);
static MAIN_THREAD_ID: AtomicU32 = AtomicU32::new(0);
/// Theo dõi cửa sổ nền để phát hiện thay đổi focus
static LAST_FOREGROUND: AtomicIsize = AtomicIsize::new(0);
/// Theo dõi focus element (child window) để phát hiện click vào ô khác trong cùng cửa sổ
static LAST_FOCUS: AtomicIsize = AtomicIsize::new(0);
/// Đặt sau Ctrl+Shift toggle để bỏ qua chu kỳ keyup tiếp theo
static JUST_TOGGLED: AtomicBool = AtomicBool::new(false);
/// Tick count cuối cùng hook nhận được phím (dùng để phát hiện hook bị gỡ)
static HOOK_LAST_SEEN: AtomicU64 = AtomicU64::new(0);

fn get_hook() -> HHOOK {
    HHOOK(HOOK_RAW.load(Ordering::Relaxed) as *mut _)
}

pub fn install_hook() -> Result<(), ()> {
    unsafe {
        MAIN_THREAD_ID.store(
            windows::Win32::System::Threading::GetCurrentThreadId(),
            Ordering::Relaxed,
        );

        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard_proc), None, 0)
            .map_err(|_| ())?;
        HOOK_RAW.store(hook.0 as isize, Ordering::Relaxed);
        Ok(())
    }
}

pub fn uninstall_hook() {
    let hook = get_hook();
    if !hook.0.is_null() {
        let _ = unsafe { UnhookWindowsHookEx(hook) };
        HOOK_RAW.store(0, Ordering::Relaxed);
    }
}

/// Cài đặt lại hook. Gọi định kỳ từ timer để phòng trường hợp
/// Windows gỡ hook do callback vượt quá thời gian cho phép
/// (LowLevelHooksTimeout, mặc định ~300ms).
/// Đây là cách phổ biến các bộ gõ (Unikey, GoTiengViet) dùng để đảm bảo ổn định.
pub fn reinstall_hook() {
    unsafe {
        let old = get_hook();
        if !old.0.is_null() {
            let _ = UnhookWindowsHookEx(old);
        }
        match SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard_proc), None, 0) {
            Ok(hook) => {
                HOOK_RAW.store(hook.0 as isize, Ordering::Relaxed);
            }
            Err(_) => {
                HOOK_RAW.store(0, Ordering::Relaxed);
            }
        }
    }
}

/// Hàm trợ giúp gọi CallNextHookEx với handle hook đã lưu
unsafe fn call_next(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    CallNextHookEx(get_hook(), code, wparam, lparam)
}

unsafe extern "system" fn ll_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code < 0 {
        return call_next(code, wparam, lparam);
    }

    let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);

    // Cập nhật heartbeat: hook vẫn sống
    HOOK_LAST_SEEN.store(kb.time as u64, Ordering::Relaxed);

    // Bỏ qua phím inject của chính mình (theo magic tag)
    if kb.dwExtraInfo == VNKEY_INJECTED_TAG {
        return call_next(code, wparam, lparam);
    }

    // Chỉ xử lý sự kiện key-down
    if wparam.0 != WM_KEYDOWN as usize && wparam.0 != WM_SYSKEYDOWN as usize {
        return call_next(code, wparam, lparam);
    }

    // Bỏ qua xử lý tiếng Việt nếu ứng dụng nền trong danh sách đen
    if crate::blacklist::is_foreground_blacklisted() {
        return call_next(code, wparam, lparam);
    }

    // Đặt lại engine khi cửa sổ nền hoặc focus element thay đổi
    // (ví dụ: click vào ô chat/comment khác trong cùng trình duyệt)
    {
        let fg = GetForegroundWindow().0 as isize;
        let prev_fg = LAST_FOREGROUND.swap(fg, Ordering::Relaxed);

        // Lấy focus element qua GetGUIThreadInfo (hoạt động từ thread khác,
        // không cần AttachThreadInput như GetFocus)
        let mut gui_info: GUITHREADINFO = std::mem::zeroed();
        gui_info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        let focus_hwnd = if GetGUIThreadInfo(0, &mut gui_info).is_ok() {
            gui_info.hwndFocus.0 as isize
        } else {
            0
        };
        let prev_focus = LAST_FOCUS.swap(focus_hwnd, Ordering::Relaxed);

        let focus_changed = (fg != prev_fg && prev_fg != 0)
            || (focus_hwnd != 0 && focus_hwnd != prev_focus);
        if focus_changed {
            if let Ok(mut guard) = ENGINE.try_lock() {
                if let Some(state) = guard.as_mut() {
                    state.engine.reset();
                }
            }
            // Cập nhật phương thức backspace cho app mới (Shift+Left vs VK_BACK)
            if fg != prev_fg {
                crate::send::update_backspace_method();
            }
        }
    }

    // Bỏ qua phím có Ctrl, Alt hoặc Win (phím tắt hệ thống)
    let ctrl = GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000 != 0;
    let alt = GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000 != 0;
    let win = GetAsyncKeyState(VK_LWIN.0 as i32) as u16 & 0x8000 != 0
           || GetAsyncKeyState(VK_RWIN.0 as i32) as u16 & 0x8000 != 0;

    // Ctrl+Shift: bật/tắt tiếng Việt (chỉ khi dùng toggle mặc định, không phải hotkey tùy chỉnh)
    // Hook cấp thấp nhận VK_LSHIFT (0xA0) hoặc VK_RSHIFT (0xA1), không phải VK_SHIFT (0x10)
    let is_shift = kb.vkCode == VK_SHIFT.0 as u32
        || kb.vkCode == VK_LSHIFT.0 as u32
        || kb.vkCode == VK_RSHIFT.0 as u32;
    if ctrl && is_shift && crate::hotkey::is_toggle_builtin() {
        if let Ok(mut guard) = ENGINE.try_lock() {
            if let Some(state) = guard.as_mut() {
                state.toggle_viet_mode();
                let vm = state.viet_mode;
                drop(guard);
                JUST_TOGGLED.store(true, Ordering::Relaxed);
                let tid = MAIN_THREAD_ID.load(Ordering::Relaxed);
                let _ = PostThreadMessageW(tid, WM_VNKEY_UPDATE_ICON, WPARAM(vm as usize), LPARAM(0));
                crate::config::save();
            }
        }
        return call_next(code, wparam, lparam);
    }

    // Nếu vừa toggle, phím thường đầu tiên xóa cờ.
    // Bỏ qua phát hiện Ctrl/Alt cho phím chỉ là modifier (Ctrl/Shift nhả).
    if kb.vkCode == VK_CONTROL.0 as u32 || kb.vkCode == VK_LCONTROL.0 as u32
        || kb.vkCode == VK_RCONTROL.0 as u32 || kb.vkCode == VK_SHIFT.0 as u32
        || kb.vkCode == VK_LSHIFT.0 as u32 || kb.vkCode == VK_RSHIFT.0 as u32
        || kb.vkCode == VK_MENU.0 as u32 || kb.vkCode == VK_LMENU.0 as u32
        || kb.vkCode == VK_RMENU.0 as u32
        || kb.vkCode == VK_LWIN.0 as u32 || kb.vkCode == VK_RWIN.0 as u32
    {
        return call_next(code, wparam, lparam);
    }

    // Xóa cờ toggle khi gặp phím thực
    let was_toggled = JUST_TOGGLED.swap(false, Ordering::Relaxed);

    if ctrl || alt || win {
        // Nếu vừa toggle và Ctrl vẫn giữ ảo, bỏ qua
        if was_toggled && ctrl && !alt && !win {
            // Tiếp tục xử lý phím bình thường
        } else {
            if let Ok(mut guard) = ENGINE.try_lock() {
                if let Some(state) = guard.as_mut() {
                    state.engine.reset();
                }
            }
            return call_next(code, wparam, lparam);
        }
    }

    let vk = kb.vkCode;

    // Xử lý phím đặc biệt: reset engine khi Enter, Escape, Tab; soft reset khi Space
    match VIRTUAL_KEY(vk as u16) {
        VK_RETURN | VK_ESCAPE | VK_TAB => {
            if let Ok(mut guard) = ENGINE.try_lock() {
                if let Some(state) = guard.as_mut() {
                    state.engine.reset();
                }
            }
            return call_next(code, wparam, lparam);
        }
        VK_SPACE => {
            // Soft reset: lưu trạng thái để backspace sau dấu cách có thể khôi phục
            if let Ok(mut guard) = ENGINE.try_lock() {
                if let Some(state) = guard.as_mut() {
                    state.engine.soft_reset();
                }
            }
            return call_next(code, wparam, lparam);
        }
        _ => {}
    }

    // Xử lý Backspace
    if VIRTUAL_KEY(vk as u16) == VK_BACK {
        return handle_backspace(code, wparam, lparam);
    }

    // Chuyển mã VK sang ký tự ASCII có xét trạng thái Shift
    let ascii = vk_to_ascii(vk, kb.scanCode);
    if ascii == 0 {
        if let Ok(mut guard) = ENGINE.try_lock() {
            if let Some(state) = guard.as_mut() {
                state.engine.reset();
            }
        }
        return call_next(code, wparam, lparam);
    }

    // Xử lý qua engine.
    // LUÔN chặn phím gốc và inject lại qua SendInput.
    // Đảm bảo mọi ký tự trong trường văn bản đều từ cùng
    // một đường (SendInput), nên VK_BACK có thể xóa đúng.
    // Sửa lỗi thanh địa chỉ Chrome và các trường autocomplete khác.
    if let Ok(mut guard) = ENGINE.try_lock() {
        if let Some(state) = guard.as_mut() {
            state.sync_options();
            let result = state.engine.process(ascii as u32);

            if result.processed && (result.backspaces > 0 || !result.output.is_empty()) {
                let utf8_text = String::from_utf8_lossy(&result.output).to_string();
                let (text, raw_bytes) = convert_output(&utf8_text, state.output_charset);
                drop(guard);
                crate::send::send_output(result.backspaces, &text, raw_bytes.as_deref());
            } else {
                drop(guard);
                crate::send::send_char(ascii);
            }
        } else {
            return call_next(code, wparam, lparam);
        }
    } else {
        // Mutex bận (GUI/config đang giữ) → vẫn gửi phím thô
        // để tránh mất ký tự (vì luôn chặn phím gốc ở dưới).
        crate::send::send_char(ascii);
    }

    LRESULT(1) // Luôn chặn phím gốc
}

unsafe fn handle_backspace(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let mut send_data: Option<(usize, String, Option<Vec<u8>>)> = None;
    if let Ok(mut guard) = ENGINE.try_lock() {
        if let Some(state) = guard.as_mut() {
            let result = state.engine.process_backspace();

            if result.processed && result.backspaces > 1 {
                let utf8_text = String::from_utf8_lossy(&result.output).to_string();
                let cs = state.output_charset;
                let (text, raw_bytes) = convert_output(&utf8_text, cs);
                send_data = Some((result.backspaces, text, raw_bytes));
            }
        }
    }

    if let Some((backspaces, text, raw_bytes)) = send_data {
        crate::send::send_output(backspaces, &text, raw_bytes.as_deref());
        return LRESULT(1);
    }

    call_next(code, wparam, lparam)
}

/// Chuyển mã phím ảo sang ký tự ASCII
/// Dùng ToUnicode API để hỗ trợ mọi bố cục bàn phím (AZERTY, QWERTZ, v.v.)
unsafe fn vk_to_ascii(vk: u32, scan_code: u32) -> u8 {
    let shift = GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000 != 0;
    let caps = GetKeyState(VK_CAPITAL.0 as i32) & 1 != 0;
    let upper = shift ^ caps;

    // Phím chữ (VK_A=0x41 đến VK_Z=0x5A): giống nhau trên mọi layout
    if (0x41..=0x5A).contains(&vk) {
        return if upper { vk as u8 } else { vk as u8 + 32 };
    }

    // Các phím còn lại: dùng ToUnicode để tra theo layout thực tế
    let mut key_state = [0u8; 256];
    if shift {
        key_state[VK_SHIFT.0 as usize] = 0x80;
    }
    if caps {
        key_state[VK_CAPITAL.0 as usize] = 0x01;
    }
    let mut buf = [0u16; 4];
    let ret = ToUnicode(vk, scan_code, Some(&key_state), &mut buf, 0);
    if ret == 1 && buf[0] > 0 && buf[0] <= 127 {
        return buf[0] as u8;
    }

    0
}

/// Chuyển đầu ra UTF-8 của engine sang bảng mã đích.
/// Trả (text, raw_bytes): nếu raw_bytes là Some, gửi byte thô thay vì Unicode.
fn convert_output(utf8_text: &str, charset_id: i32) -> (String, Option<Vec<u8>>) {
    let charset = match charset_id {
        0 => Charset::Unicode,
        1 => Charset::Utf8,
        2 => Charset::NcrDec,
        3 => Charset::NcrHex,
        5 => Charset::WinCP1258,
        10 => Charset::Viqr,
        20 => Charset::Tcvn3,
        21 => Charset::Vps,
        22 => Charset::Viscii,
        23 => Charset::Bkhcm1,
        24 => Charset::VietwareF,
        25 => Charset::Isc,
        40 => Charset::VniWin,
        41 => Charset::Bkhcm2,
        42 => Charset::VietwareX,
        43 => Charset::VniMac,
        _ => return (utf8_text.to_string(), None),
    };

    // Với bảng mã họ Unicode, gửi dưới dạng UTF-16
    match charset {
        Charset::Unicode | Charset::Utf8 => {
            return (utf8_text.to_string(), None);
        }
        _ => {}
    }

    // Chuyển sang bảng mã đích
    match vnkey_engine::charset::from_utf8(utf8_text, charset) {
        Ok(bytes) => (String::new(), Some(bytes)),
        Err(_) => (utf8_text.to_string(), None),
    }
}
