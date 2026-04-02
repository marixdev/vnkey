//! Hook bàn phím cấp thấp để chặn phím toàn cục

use crate::{ENGINE, WM_VNKEY_UPDATE_ICON, VNKEY_INJECTED_TAG};

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64, Ordering};
use std::time::Instant;
use vnkey_engine::charset::Charset;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// HHOOK bọc con trỏ thô. Lưu giá trị isize trực tiếp để an toàn luồng.
static HOOK_RAW: AtomicIsize = AtomicIsize::new(0);
static MOUSE_HOOK_RAW: AtomicIsize = AtomicIsize::new(0);
static MAIN_THREAD_ID: AtomicU32 = AtomicU32::new(0);
/// Theo dõi cửa sổ nền để cập nhật phương thức backspace
static LAST_FOREGROUND: AtomicIsize = AtomicIsize::new(0);
/// Đặt sau Ctrl+Shift toggle để bỏ qua chu kỳ keyup tiếp theo
static JUST_TOGGLED: AtomicBool = AtomicBool::new(false);
/// Ctrl+Shift đang chờ: chỉ toggle khi nhả mà KHÔNG có phím khác xen giữa.
/// Sửa lỗi Ctrl+Shift+N/V kích hoạt toggle sai (#15).
static CTRL_SHIFT_PENDING: AtomicBool = AtomicBool::new(false);
/// Tick count cuối cùng hook nhận được phím (dùng để phát hiện hook bị gỡ)
static HOOK_LAST_SEEN: AtomicU64 = AtomicU64::new(0);
/// Thời điểm phím vật lý cuối cùng (dùng timeout reset engine)
static LAST_PHYSICAL_KEY_TIME: AtomicU32 = AtomicU32::new(0);
/// Epoch cho Instant (khởi tạo khi install_hook)
static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Theo dõi phím đã chặn trên keydown để chặn luôn keyup tương ứng.
/// Khi hook chặn keydown (return LRESULT(1)) và inject qua SendInput,
/// keyup gốc vẫn đến hook. Nếu để keyup qua (call_next), VMware nhận
/// keyup mồ côi (không có keydown gốc) → hỏng trạng thái bàn phím ảo.
static BLOCKED_KEYS: [AtomicBool; 256] = {
    const FALSE: AtomicBool = AtomicBool::new(false);
    [FALSE; 256]
};

fn get_hook() -> HHOOK {
    HHOOK(HOOK_RAW.load(Ordering::Relaxed) as *mut _)
}

fn get_mouse_hook() -> HHOOK {
    HHOOK(MOUSE_HOOK_RAW.load(Ordering::Relaxed) as *mut _)
}

pub fn install_hook() -> Result<(), ()> {
    let epoch = EPOCH.get_or_init(Instant::now);
    // Khởi tạo heartbeat để timer không reinstall ngay sau khi khởi động
    HOOK_LAST_SEEN.store(epoch.elapsed().as_millis() as u64, Ordering::Relaxed);
    unsafe {
        MAIN_THREAD_ID.store(
            windows::Win32::System::Threading::GetCurrentThreadId(),
            Ordering::Relaxed,
        );

        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard_proc), None, 0)
            .map_err(|_| ())?;
        HOOK_RAW.store(hook.0 as isize, Ordering::Relaxed);

        // Mouse hook: reset engine khi click chuột (chuyển field, chuyển tab, v.v.)
        // Giống cách OpenKey xử lý — đảm bảo engine reset khi user click vào ô nhập khác.
        if let Ok(mhook) = SetWindowsHookExW(WH_MOUSE_LL, Some(ll_mouse_proc), None, 0) {
            MOUSE_HOOK_RAW.store(mhook.0 as isize, Ordering::Relaxed);
        }

        Ok(())
    }
}

pub fn uninstall_hook() {
    let hook = get_hook();
    if !hook.0.is_null() {
        let _ = unsafe { UnhookWindowsHookEx(hook) };
        HOOK_RAW.store(0, Ordering::Relaxed);
    }
    let mhook = get_mouse_hook();
    if !mhook.0.is_null() {
        let _ = unsafe { UnhookWindowsHookEx(mhook) };
        MOUSE_HOOK_RAW.store(0, Ordering::Relaxed);
    }
}

/// Cài đặt lại hook NẾU hook dường như đã bị Windows gỡ.
/// Gọi định kỳ từ timer (mỗi 5 giây). Chỉ reinstall khi không nhận
/// được sự kiện bàn phím nào trong >5 giây — tức hook có thể đã chết.
/// KHÔNG reinstall khi đang gõ bình thường vì:
/// - Tạo khoảng trống (unhook→rehook) gây mất phím
/// - Trong VMware/RDP, SetWindowsHookEx chậm → khoảng trống dài hơn
/// Chỉ reinstall keyboard hook — mouse hook không bị ảnh hưởng.
pub fn reinstall_hook() {
    // Kiểm tra heartbeat: nếu hook còn nhận event gần đây, bỏ qua
    let last_seen = HOOK_LAST_SEEN.load(Ordering::Relaxed);
    if let Some(epoch) = EPOCH.get() {
        let now_ms = epoch.elapsed().as_millis() as u64;
        let elapsed = now_ms.saturating_sub(last_seen);
        if elapsed < 5000 {
            // Hook vẫn sống, không cần reinstall
            return;
        }
    }

    crate::debug_log::log("HOOK_REINSTALL (hook appears dead)");
    unsafe {
        // Chỉ reinstall keyboard hook, KHÔNG đụng mouse hook.
        // Mouse hook vẫn hoạt động (MOUSE_CLICK events trong log xác nhận).
        // Reinstall mouse hook không cần thiết và gây churn thêm.
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

    // Cập nhật heartbeat SAU reinstall để chống reinstall liên tiếp.
    // Không có dòng này → timer tiếp theo (5s) thấy heartbeat cũ → reinstall lại
    // → vòng lặp reinstall vô tận khi user không gõ.
    if let Some(epoch) = EPOCH.get() {
        HOOK_LAST_SEEN.store(epoch.elapsed().as_millis() as u64, Ordering::Relaxed);
    }
}

/// Hàm trợ giúp gọi CallNextHookEx với handle hook đã lưu
unsafe fn call_next(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    CallNextHookEx(get_hook(), code, wparam, lparam)
}

/// Chặn keydown: đánh dấu VK để chặn luôn keyup gốc và trả LRESULT(1).
/// Phải gọi thay vì `return LRESULT(1)` trực tiếp cho mọi phím bị chặn.
fn block_key(vk: u32) -> LRESULT {
    let idx = vk as usize;
    if idx < 256 {
        BLOCKED_KEYS[idx].store(true, Ordering::Relaxed);
    }
    LRESULT(1)
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
    let hook_start = crate::debug_log::timer_start();

    // Cập nhật heartbeat: hook vẫn sống (dùng Instant monotonic)
    if let Some(epoch) = EPOCH.get() {
        HOOK_LAST_SEEN.store(epoch.elapsed().as_millis() as u64, Ordering::Relaxed);
    }

    // Bỏ qua phím inject của chính mình.
    // Kiểm tra cả dwExtraInfo tag VÀ cờ SENDING để tương thích VMware/RDP/VNC
    // (một số lớp ảo hóa không giữ nguyên dwExtraInfo).
    if kb.dwExtraInfo == VNKEY_INJECTED_TAG || crate::send::is_sending() {
        crate::debug_log::log_hook_event(kb.vkCode, kb.scanCode, kb.flags.0, kb.dwExtraInfo, kb.time,
            &format!("SKIP_OWN wparam=0x{:X}", wparam.0));
        return call_next(code, wparam, lparam);
    }

    // ── BẢO MẬT: VM/RDP pass-through TRƯỚC khi log bất cứ gì ──
    // Khi foreground là VMware/VirtualBox/RDP, user có thể đang gõ password
    // login OS trong guest. Hook KHÔNG được log vkCode/scanCode vì đó là
    // thông tin nhạy cảm. Chỉ cập nhật heartbeat rồi trả luôn.
    if crate::send::is_vm_app() {
        return call_next(code, wparam, lparam);
    }

    // Log phím không phải của mình để debug
    crate::debug_log::log_hook_event(kb.vkCode, kb.scanCode, kb.flags.0, kb.dwExtraInfo, kb.time,
        &format!("INCOMING wparam=0x{:X}", wparam.0));

    // Chỉ xử lý sự kiện key-down
    if wparam.0 != WM_KEYDOWN as usize && wparam.0 != WM_SYSKEYDOWN as usize {
        // Ctrl+Shift toggle: thực hiện khi nhả modifier mà chưa có phím khác xen giữa (#15)
        {
            let is_shift_key = kb.vkCode == VK_SHIFT.0 as u32
                || kb.vkCode == VK_LSHIFT.0 as u32
                || kb.vkCode == VK_RSHIFT.0 as u32;
            let is_ctrl_key = kb.vkCode == VK_CONTROL.0 as u32
                || kb.vkCode == VK_LCONTROL.0 as u32
                || kb.vkCode == VK_RCONTROL.0 as u32;
            if (is_shift_key || is_ctrl_key)
                && CTRL_SHIFT_PENDING.swap(false, Ordering::Relaxed)
            {
                crate::debug_log::log("  CTRL_SHIFT_TOGGLE (keyup => execute pending)");
                if let Ok(mut guard) = ENGINE.try_lock() {
                    if let Some(state) = guard.as_mut() {
                        state.toggle_viet_mode();
                        let vm = state.viet_mode;
                        drop(guard);
                        JUST_TOGGLED.store(true, Ordering::Relaxed);
                        let tid = MAIN_THREAD_ID.load(Ordering::Relaxed);
                        let _ = PostThreadMessageW(tid, WM_VNKEY_UPDATE_ICON, WPARAM(vm as usize), LPARAM(0));
                        crate::config::save();
                        crate::osd::show(if vm { "Tiếng Việt" } else { "English" });
                    }
                }
            }
        }

        // Chặn keyup mồ côi: nếu keydown tương ứng đã bị chặn (inject qua SendInput),
        // keyup gốc cũng phải bị chặn. Nếu không, VMware/RDP nhận keyup mà không có
        // keydown gốc → hỏng trạng thái bàn phím ảo.
        let vk_idx = kb.vkCode as usize;
        if vk_idx < 256 && BLOCKED_KEYS[vk_idx].swap(false, Ordering::Relaxed) {
            crate::debug_log::log_hook_event(kb.vkCode, kb.scanCode, kb.flags.0, kb.dwExtraInfo, kb.time,
                &format!("BLOCK_KEYUP wparam=0x{:X}", wparam.0));
            return LRESULT(1);
        }
        crate::debug_log::log_hook_event(kb.vkCode, kb.scanCode, kb.flags.0, kb.dwExtraInfo, kb.time,
            &format!("SKIP_KEYUP wparam=0x{:X}", wparam.0));
        return call_next(code, wparam, lparam);
    }

    // Bỏ qua xử lý tiếng Việt nếu ứng dụng nền trong danh sách đen
    if crate::blacklist::is_foreground_blacklisted() {
        crate::debug_log::log_hook_event(kb.vkCode, kb.scanCode, kb.flags.0, kb.dwExtraInfo, kb.time,
            "SKIP_BLACKLISTED");
        return call_next(code, wparam, lparam);
    }

    crate::debug_log::log_hook_event(kb.vkCode, kb.scanCode, kb.flags.0, kb.dwExtraInfo, kb.time,
        "PROCESS_KEY");

    // Timeout-based reset: nếu user không gõ > 5 giây, reset engine.
    // KHÔNG dùng focus/foreground change để reset vì autocomplete popup,
    // tooltip, dropdown liên tục thay đổi foreground/hwndFocus gây mất
    // trạng thái giữa chừng (ví dụ: "đc" → "ddc").
    // Các trường hợp chuyển context đã được xử lý bởi:
    // - Alt+Tab/Ctrl+… → modifier check bên dưới reset engine
    // - Enter/Tab/Escape → reset engine
    // - Space → soft reset
    // - Click chuột chuyển field → gap > 5 giây (di chuột + click + gõ)
    {
        let now_tick = kb.time;
        let last_tick = LAST_PHYSICAL_KEY_TIME.swap(now_tick, Ordering::Relaxed);
        let typing_gap = now_tick.wrapping_sub(last_tick);

        if typing_gap > 5000 {
            crate::debug_log::log(&format!("  TIMEOUT_RESET gap={}ms", typing_gap));
            if let Ok(mut guard) = ENGINE.try_lock() {
                if let Some(state) = guard.as_mut() {
                    state.engine.reset();
                }
            }
        }

        // Cập nhật phương thức backspace khi app thay đổi
        let fg = GetForegroundWindow().0 as isize;
        let prev_fg = LAST_FOREGROUND.swap(fg, Ordering::Relaxed);
        if fg != prev_fg {
            let exe = crate::blacklist::get_foreground_exe_cached();
            crate::debug_log::log(&format!("  FG_CHANGED 0x{:X} -> 0x{:X} exe={:?}", prev_fg, fg, exe));
            crate::send::update_backspace_method();
            crate::app_charset::update_app_charset();
        }
    }

    // VnKey's own windows: hook pass-through, Vietnamese xử lý qua JS+IPC.
    // Vẫn cho Ctrl+Shift toggle hoạt động (xử lý ở keyup handler bên trên).
    if crate::send::is_self() {
        crate::debug_log::log("  SELF_PASSTHROUGH");
        return call_next(code, wparam, lparam);
    }

    // Bỏ qua phím có Ctrl, Alt hoặc Win (phím tắt hệ thống)
    let ctrl = GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000 != 0;
    let alt = GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000 != 0;
    let win = GetAsyncKeyState(VK_LWIN.0 as i32) as u16 & 0x8000 != 0
           || GetAsyncKeyState(VK_RWIN.0 as i32) as u16 & 0x8000 != 0;
    let shift = GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000 != 0;

    crate::debug_log::log_modifiers(ctrl, alt, win, shift);

    // Ctrl+Shift: defer toggle đến keyup (#15)
    // Nếu user nhấn Ctrl+Shift+N/V, phím N/V sẽ hủy toggle.
    // Chỉ toggle khi Ctrl+Shift được nhả mà KHÔNG có phím nào khác xen giữa.
    let is_shift_key = kb.vkCode == VK_SHIFT.0 as u32
        || kb.vkCode == VK_LSHIFT.0 as u32
        || kb.vkCode == VK_RSHIFT.0 as u32;
    let is_ctrl_key = kb.vkCode == VK_CONTROL.0 as u32
        || kb.vkCode == VK_LCONTROL.0 as u32
        || kb.vkCode == VK_RCONTROL.0 as u32;
    if crate::hotkey::is_toggle_builtin()
        && ((ctrl && is_shift_key) || (shift && is_ctrl_key))
    {
        crate::debug_log::log("  CTRL_SHIFT_PENDING (set on keydown)");
        CTRL_SHIFT_PENDING.store(true, Ordering::Relaxed);
        return call_next(code, wparam, lparam);
    }

    // Phím tắt tùy chỉnh chuyển Việt/Anh: xử lý ngay trong hook để
    // KHÔNG gây mất focus (RegisterHotKey sẽ kích hoạt hidden window).
    if !crate::hotkey::is_toggle_builtin() {
        if let Ok(hk) = crate::hotkey::HOTKEY_SETTINGS.try_lock() {
            if hk.toggle_vk != 0 && kb.vkCode == hk.toggle_vk {
                let need_ctrl = hk.toggle_mods & 2 != 0;
                let need_alt = hk.toggle_mods & 1 != 0;
                let need_shift = hk.toggle_mods & 4 != 0;
                if ctrl == need_ctrl && alt == need_alt && shift == need_shift && !win {
                    drop(hk);
                    if let Ok(mut guard) = ENGINE.try_lock() {
                        if let Some(state) = guard.as_mut() {
                            state.toggle_viet_mode();
                            let vm = state.viet_mode;
                            drop(guard);
                            JUST_TOGGLED.store(true, Ordering::Relaxed);
                            let tid = MAIN_THREAD_ID.load(Ordering::Relaxed);
                            let _ = PostThreadMessageW(tid, WM_VNKEY_UPDATE_ICON, WPARAM(vm as usize), LPARAM(0));
                            crate::config::save();
                            crate::osd::show(if vm { "Tiếng Việt" } else { "English" });
                        }
                    }
                    // Chặn phím gốc để app đang focus không nhận (vd: Alt+Z không gõ 'z')
                    return block_key(kb.vkCode);
                }
            }
        }
    }

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

    // Phím thực (không phải modifier): hủy pending Ctrl+Shift toggle (#15)
    if CTRL_SHIFT_PENDING.swap(false, Ordering::Relaxed) {
        crate::debug_log::log("  CTRL_SHIFT_CANCEL (non-modifier key pressed)");
    }

    // Xóa cờ toggle khi gặp phím thực
    let was_toggled = JUST_TOGGLED.swap(false, Ordering::Relaxed);

    if ctrl || alt || win {
        // Nếu vừa toggle và Ctrl vẫn giữ ảo, bỏ qua
        if was_toggled && ctrl && !alt && !win {
            // Tiếp tục xử lý phím bình thường
        } else {
            crate::debug_log::log(&format!("  MODIFIER_RESET ctrl={} alt={} win={}", ctrl, alt, win));
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
    // Console apps: native pass-through (call_next).
    // GUI apps (VMware/RDP): inject qua SendInput để tránh lệch luồng
    // giữa native và KEYEVENTF_UNICODE khiến hook bị chết.
    match VIRTUAL_KEY(vk as u16) {
        VK_RETURN | VK_ESCAPE | VK_TAB => {
            if let Ok(mut guard) = ENGINE.try_lock() {
                if let Some(state) = guard.as_mut() {
                    state.engine.reset();
                }
            }
            if crate::send::is_using_vk_back() {
                return call_next(code, wparam, lparam);
            }
            crate::send::send_key(VIRTUAL_KEY(vk as u16));
            crate::debug_log::log_elapsed("HOOK_TOTAL", hook_start);
            return block_key(vk);
        }
        VK_SPACE => {
            if let Ok(mut guard) = ENGINE.try_lock() {
                if let Some(state) = guard.as_mut() {
                    // Khi macro bật: gọi process(0x20) để engine kiểm tra macro expansion
                    if state.macro_enabled && !state.engine.macro_table.is_empty() {
                        state.sync_options();
                        let result = state.engine.process(0x20);
                        if result.processed && (result.backspaces > 0 || !result.output.is_empty()) {
                            let utf8_text = String::from_utf8_lossy(&result.output).to_string();
                            let effective_cs = crate::app_charset::get_current_app_charset()
                                .unwrap_or(state.output_charset);
                            let (text, raw_bytes) = convert_output(&utf8_text, effective_cs);
                            drop(guard);
                            crate::send::send_output(result.backspaces, &text, raw_bytes.as_deref());
                            crate::debug_log::log_elapsed("HOOK_TOTAL", hook_start);
                            return block_key(vk);
                        }
                        // Không khớp macro — process() đã gọi soft_reset() bên trong
                    } else {
                        // Macro tắt: soft reset như cũ
                        state.engine.soft_reset();
                    }
                }
            }
            if crate::send::is_using_vk_back() {
                return call_next(code, wparam, lparam);
            }
            crate::send::send_key(VK_SPACE);
            crate::debug_log::log_elapsed("HOOK_TOTAL", hook_start);
            return block_key(vk);
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
        crate::debug_log::log(&format!("  VK_TO_ASCII=0 → RESET vk=0x{:02X}", vk));
        if let Ok(mut guard) = ENGINE.try_lock() {
            if let Some(state) = guard.as_mut() {
                state.engine.reset();
            }
        }
        // GUI apps: inject qua SendInput để giữ cùng đường với ký tự.
        // Console apps: native pass-through.
        if !crate::send::is_using_vk_back() {
            crate::send::send_key(VIRTUAL_KEY(vk as u16));
            crate::debug_log::log_elapsed("HOOK_TOTAL", hook_start);
            return block_key(vk);
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

            let utf8_text = String::from_utf8_lossy(&result.output).to_string();
            crate::debug_log::log_engine_result(ascii, result.processed, result.backspaces, &utf8_text);

            if result.processed && (result.backspaces > 0 || !result.output.is_empty()) {
                let effective_cs = crate::app_charset::get_current_app_charset()
                    .unwrap_or(state.output_charset);
                let (text, raw_bytes) = convert_output(&utf8_text, effective_cs);
                drop(guard);
                crate::debug_log::log(&format!("  SEND_OUTPUT backs={} text={:?}", result.backspaces, text));
                crate::send::send_output(result.backspaces, &text, raw_bytes.as_deref());
            } else if crate::send::is_using_vk_back() {
                // Console/Office app: KHÔNG block, để phím qua native.
                // KEYEVENTF_UNICODE (VK_PACKET) không hoạt động tốt trong
                // console apps (PowerShell PSReadLine bỏ qua, CMD garbled).
                drop(guard);
                crate::debug_log::log(&format!("  PASS_THROUGH '{}' (console)", ascii as char));
                crate::debug_log::log_elapsed("HOOK_TOTAL", hook_start);
                return call_next(code, wparam, lparam);
            } else {
                // Engine không xử lý: để phím qua native (tương thích Kanata #13)
                drop(guard);
                crate::debug_log::log(&format!("  PASS_THROUGH '{}' (unprocessed)", ascii as char));
                crate::debug_log::log_elapsed("HOOK_TOTAL", hook_start);
                return call_next(code, wparam, lparam);
            }
        } else {
            crate::debug_log::log("  ENGINE_NONE");
            return call_next(code, wparam, lparam);
        }
    } else {
        // Mutex bận (GUI/config đang giữ) → pass-through native
        crate::debug_log::log(&format!("  MUTEX_BUSY → PASS_THROUGH '{}'", ascii as char));
        crate::debug_log::log_elapsed("HOOK_TOTAL", hook_start);
        return call_next(code, wparam, lparam);
    }

    crate::debug_log::log_elapsed("HOOK_TOTAL", hook_start);
    block_key(vk) // Chặn phím gốc (GUI apps)
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
                let cs = crate::app_charset::get_current_app_charset()
                    .unwrap_or(state.output_charset);
                let (text, raw_bytes) = convert_output(&utf8_text, cs);
                send_data = Some((result.backspaces, text, raw_bytes));
            }
        }
    }

    if let Some((backspaces, text, raw_bytes)) = send_data {
        crate::send::send_output(backspaces, &text, raw_bytes.as_deref());
        return block_key(VK_BACK.0 as u32);
    }

    // Backspace thường:
    // - Console/Office apps: native pass-through (tương thích tốt nhất)
    // - GUI apps: inject qua SendInput (tránh lệch luồng VMware/RDP
    //   giữa native VK_BACK và KEYEVENTF_UNICODE)
    if crate::send::is_using_vk_back() {
        return call_next(code, wparam, lparam);
    }
    crate::send::send_backspace();
    block_key(VK_BACK.0 as u32)
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

/// Mouse hook: reset engine khi click chuột.
/// Giống cách OpenKey dùng WH_MOUSE_LL để đảm bảo engine được reset
/// khi user click vào ô nhập khác (chuyển field trong cùng cửa sổ).
unsafe extern "system" fn ll_mouse_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        // Cập nhật heartbeat: mouse hook vẫn sống → keyboard hook cũng nên sống.
        // Ngăn reinstall vô ích khi user dùng chuột nhưng không gõ phím.
        if let Some(epoch) = EPOCH.get() {
            HOOK_LAST_SEEN.store(epoch.elapsed().as_millis() as u64, Ordering::Relaxed);
        }

        match wparam.0 as u32 {
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN => {
                crate::debug_log::log("MOUSE_CLICK → RESET");
                if let Ok(mut guard) = ENGINE.try_lock() {
                    if let Some(state) = guard.as_mut() {
                        state.engine.reset();
                    }
                }
            }
            _ => {}
        }
    }
    CallNextHookEx(get_mouse_hook(), code, wparam, lparam)
}
