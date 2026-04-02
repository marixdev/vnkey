//! Icon khay hệ thống và menu ngữ cảnh

use crate::{ENGINE, WM_VNKEY_TRAY};

use std::sync::atomic::{AtomicBool, Ordering};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Cài đặt "Chạy với quyền Admin" — lưu vào config.json
static RUN_AS_ADMIN: AtomicBool = AtomicBool::new(false);

pub fn set_run_as_admin(v: bool) { RUN_AS_ADMIN.store(v, Ordering::Relaxed); }
pub fn get_run_as_admin() -> bool { RUN_AS_ADMIN.load(Ordering::Relaxed) }

// Mã tài nguyên icon (từ resources.rc)
const IDI_ICON_V: u16 = 101;
const IDI_ICON_E: u16 = 102;

// Mã lệnh menu
const IDM_VIET_MODE: u16 = 100;
// Menu con kiểu gõ
const IDM_IM_TELEX: u16 = 200;
const IDM_IM_STELEX: u16 = 201;
const IDM_IM_VNI: u16 = 202;
const IDM_IM_VIQR: u16 = 203;
const IDM_IM_MSVI: u16 = 204;
// Menu con bảng mã
const IDM_CS_UNICODE: u16 = 250;
const IDM_CS_UTF8: u16 = 251;
const IDM_CS_NCRDEC: u16 = 252;
const IDM_CS_NCRHEX: u16 = 253;
const IDM_CS_CP1258: u16 = 255;
const IDM_CS_VIQR: u16 = 260;
const IDM_CS_TCVN3: u16 = 270;
const IDM_CS_VPS: u16 = 271;
const IDM_CS_VISCII: u16 = 272;
const IDM_CS_VNU: u16 = 275;
const IDM_CS_VNIWIN: u16 = 290;
const IDM_CS_VNIMAC: u16 = 293;
// Tùy chọn
const IDM_OPT_SPELL: u16 = 300;
const IDM_OPT_FREE: u16 = 301;
const IDM_OPT_MODERN: u16 = 302;
const IDM_CONFIG: u16 = 400;
const IDM_CONVERTER: u16 = 401;
const IDM_BLACKLIST: u16 = 402;
const IDM_INFO: u16 = 403;
const IDM_HOTKEY: u16 = 404;
const IDM_APPCS: u16 = 405;
const IDM_MACROTBL: u16 = 406;
const IDM_OPT_EDE: u16 = 303;
const IDM_OPT_MACRO: u16 = 304;
const IDM_RUNAS: u16 = 500;
const IDM_EXIT: u16 = 900;

/// Kiểm tra xem tiến trình hiện tại có đang chạy với quyền Admin không.
pub fn is_elevated() -> bool {
    use windows::Win32::Security::*;
    unsafe {
        let mut token = HANDLE::default();
        let proc = windows::Win32::System::Threading::GetCurrentProcess();
        if windows::Win32::System::Threading::OpenProcessToken(proc, TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        let _ = windows::Win32::Foundation::CloseHandle(token);
        ok.is_ok() && elevation.TokenIsElevated != 0
    }
}

pub fn create_tray_icon(hwnd: HWND, viet_mode: bool) {
    let icon_id = if viet_mode { IDI_ICON_V } else { IDI_ICON_E };
    let mut tip = [0u16; 128];
    let tip_str: Vec<u16> = "VnKey - Bộ gõ tiếng Việt"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let copy_len = tip_str.len().min(tip.len());
    tip[..copy_len].copy_from_slice(&tip_str[..copy_len]);

    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_TIP | NIF_ICON,
        uCallbackMessage: WM_VNKEY_TRAY,
        szTip: tip,
        ..Default::default()
    };

    unsafe {
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
            .unwrap_or_default();
        // Bắt đầu với icon đúng theo viet_mode đã lưu
        nid.hIcon = LoadIconW(hinstance, PCWSTR(icon_id as *const u16))
            .unwrap_or_else(|_| LoadIconW(None, IDI_APPLICATION).unwrap_or_default());
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        let _ = Shell_NotifyIconW(NIM_SETVERSION, &nid);
    }
}

pub fn update_tray_icon(hwnd: HWND, viet_mode: bool) {
    let icon_id = if viet_mode { IDI_ICON_V } else { IDI_ICON_E };
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_ICON,
        ..Default::default()
    };
    unsafe {
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
            .unwrap_or_default();
        nid.hIcon = LoadIconW(hinstance, PCWSTR(icon_id as *const u16))
            .unwrap_or_else(|_| LoadIconW(None, IDI_APPLICATION).unwrap_or_default());
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

pub fn remove_tray_icon(hwnd: HWND) {
    let nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        ..Default::default()
    };
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

pub fn handle_tray_message(hwnd: HWND, lparam: LPARAM) {
    let msg = (lparam.0 & 0xFFFF) as u32;
    match msg {
        WM_RBUTTONUP => show_context_menu(hwnd),
        WM_LBUTTONUP => {
            // Nhấp trái: chuyển chế độ tiếng Việt
            let viet_mode;
            if let Ok(mut guard) = ENGINE.lock() {
                if let Some(state) = guard.as_mut() {
                    state.toggle_viet_mode();
                    viet_mode = state.viet_mode;
                } else {
                    return;
                }
            } else {
                return;
            }
            update_tray_icon(hwnd, viet_mode);
            crate::config::save();
        }
        WM_LBUTTONDBLCLK => {
            // Nhấp đôi mở cửa sổ cấu hình
            crate::gui::open_config_window();
        }
        _ => {}
    }
}

fn show_context_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu().unwrap();

        // Đọc trạng thái hiện tại
        let (viet_mode, im, cs, spell, free, modern, ede, macro_en) = {
            let guard = match ENGINE.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            match guard.as_ref() {
                Some(s) => (
                    s.viet_mode,
                    s.input_method,
                    s.output_charset,
                    s.spell_check,
                    s.free_marking,
                    s.modern_style,
                    s.ede_mode,
                    s.macro_enabled,
                ),
                None => return,
            }
        };

        // Chuyển chế độ Việt — hiện phím tắt thực tế
        let shortcut = {
            match crate::hotkey::HOTKEY_SETTINGS.lock() {
                Ok(hk) => {
                    if hk.toggle_vk == 0 && hk.toggle_mods == 0 {
                        "Ctrl+Shift".to_string()
                    } else {
                        crate::hotkey::hotkey_display_text(hk.toggle_vk, hk.toggle_mods)
                    }
                }
                Err(_) => "Ctrl+Shift".to_string(),
            }
        };
        let label_str = if viet_mode {
            format!("Tiếng Việt ({shortcut})")
        } else {
            format!("English ({shortcut})")
        };
        let label_wide: Vec<u16> = label_str.encode_utf16().chain(std::iter::once(0)).collect();
        AppendMenuW(menu, MF_STRING, IDM_VIET_MODE as usize, PCWSTR(label_wide.as_ptr())).ok();

        AppendMenuW(menu, MF_SEPARATOR, 0, None).ok();

        // -- Menu con kiểu gõ --
        let im_sub = CreatePopupMenu().unwrap();
        let im_items: [(u16, PCWSTR); 5] = [
            (IDM_IM_TELEX, w!("Telex")),
            (IDM_IM_STELEX, w!("Simple Telex")),
            (IDM_IM_VNI, w!("VNI")),
            (IDM_IM_VIQR, w!("VIQR")),
            (IDM_IM_MSVI, w!("MS Vietnamese")),
        ];
        for (id, text) in &im_items {
            let flags = MF_STRING
                | if *id == IDM_IM_TELEX + im as u16 {
                    MF_CHECKED
                } else {
                    MF_UNCHECKED
                };
            AppendMenuW(im_sub, flags, *id as usize, *text).ok();
        }
        AppendMenuW(menu, MF_POPUP, im_sub.0 as usize, w!("Kiểu gõ")).ok();

        // -- Menu con bảng mã --
        let cs_sub = CreatePopupMenu().unwrap();
        let cs_items: [(u16, i32, PCWSTR); 12] = [
            (IDM_CS_UNICODE, 0, w!("Unicode")),
            (IDM_CS_UTF8, 1, w!("UTF-8")),
            (IDM_CS_NCRDEC, 2, w!("NCR Decimal")),
            (IDM_CS_NCRHEX, 3, w!("NCR Hex")),
            (IDM_CS_CP1258, 5, w!("Windows CP-1258")),
            (IDM_CS_VIQR, 10, w!("VIQR")),
            (IDM_CS_TCVN3, 20, w!("TCVN3 (ABC)")),
            (IDM_CS_VPS, 21, w!("VPS")),
            (IDM_CS_VISCII, 22, w!("VISCII")),
            (IDM_CS_VNU, 25, w!("VNU")),
            (IDM_CS_VNIWIN, 40, w!("VNI Windows")),
            (IDM_CS_VNIMAC, 43, w!("VNI Mac")),
        ];
        for (id, cs_val, text) in &cs_items {
            let flags = MF_STRING
                | if *cs_val == cs {
                    MF_CHECKED
                } else {
                    MF_UNCHECKED
                };
            AppendMenuW(cs_sub, flags, *id as usize, *text).ok();
        }
        AppendMenuW(menu, MF_POPUP, cs_sub.0 as usize, w!("Bảng mã")).ok();

        AppendMenuW(menu, MF_SEPARATOR, 0, None).ok();

        // Tùy chọn
        let opt_items: [(u16, PCWSTR, bool); 5] = [
            (IDM_OPT_SPELL, w!("Kiểm tra chính tả"), spell),
            (IDM_OPT_FREE, w!("Bỏ dấu tự do"), free),
            (IDM_OPT_MODERN, w!("Kiểu mới (oà, uý)"), modern),
            (IDM_OPT_EDE, w!("Tiếng Tây Nguyên (Êđê)"), ede),
            (IDM_OPT_MACRO, w!("Gõ tắt (Auto-text)"), macro_en),
        ];
        for (id, text, checked) in &opt_items {
            let flags = MF_STRING | if *checked { MF_CHECKED } else { MF_UNCHECKED };
            AppendMenuW(menu, flags, *id as usize, *text).ok();
        }

        AppendMenuW(menu, MF_SEPARATOR, 0, None).ok();

        // Cửa sổ cấu hình
        AppendMenuW(menu, MF_STRING, IDM_CONFIG as usize, w!("⚙ Cấu hình...")).ok();

        // Chuyển mã
        AppendMenuW(menu, MF_STRING, IDM_CONVERTER as usize, w!("🔄 Chuyển bảng mã...")).ok();

        // Loại trừ
        AppendMenuW(menu, MF_STRING, IDM_BLACKLIST as usize, w!("🚫 Loại trừ ứng dụng...")).ok();

        // Bảng mã theo app
        AppendMenuW(menu, MF_STRING, IDM_APPCS as usize, w!("📋 Bảng mã theo app...")).ok();

        // Gõ tắt
        AppendMenuW(menu, MF_STRING, IDM_MACROTBL as usize, w!("📝 Gõ tắt...")).ok();

        // Giới thiệu
        AppendMenuW(menu, MF_STRING, IDM_INFO as usize, w!("ℹ Giới thiệu...")).ok();

        // Phím tắt
        AppendMenuW(menu, MF_STRING, IDM_HOTKEY as usize, w!("⌨ Gán phím...")).ok();

        // Chạy với quyền Admin (toggle)
        {
            let checked = get_run_as_admin();
            let flags = MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED };
            AppendMenuW(menu, flags, IDM_RUNAS as usize,
                w!("🛡 Chạy với quyền Admin")).ok();
        }

        AppendMenuW(menu, MF_SEPARATOR, 0, None).ok();

        // Thoát
        AppendMenuW(menu, MF_STRING, IDM_EXIT as usize, w!("Thoát")).ok();

        // Hiện menu tại vị trí con trỏ
        let mut pt = windows::Win32::Foundation::POINT::default();
        windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt).ok();

        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(menu, TPM_BOTTOMALIGN | TPM_LEFTALIGN, pt.x, pt.y, 0, hwnd, None);
        PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0)).ok();

        DestroyMenu(menu).ok();
    }
}

/// Lấy đường dẫn exe hiện tại.
fn get_exe_path() -> Option<Vec<u16>> {
    let mut buf = [0u16; 260];
    let len = unsafe { windows::Win32::System::LibraryLoader::GetModuleFileNameW(None, &mut buf) };
    if len == 0 { return None; }
    Some(buf.to_vec())
}

/// Khởi chạy lại VnKey với quyền Admin qua UAC prompt.
fn relaunch_as_admin() {
    if let Some(buf) = get_exe_path() {
        unsafe {
            ShellExecuteW(HWND::default(), w!("runas"), PCWSTR(buf.as_ptr()),
                PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL);
            PostQuitMessage(0);
        }
    }
}

/// Khởi chạy lại VnKey bình thường (không quyền Admin).
fn relaunch_normal() {
    if let Some(buf) = get_exe_path() {
        unsafe {
            ShellExecuteW(HWND::default(), w!("open"), PCWSTR(buf.as_ptr()),
                PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL);
            PostQuitMessage(0);
        }
    }
}

pub fn handle_menu_command(hwnd: HWND, id: u16) {
    let mut guard = match ENGINE.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let state = match guard.as_mut() {
        Some(s) => s,
        None => return,
    };

    match id {
        IDM_VIET_MODE => {
            state.toggle_viet_mode();
            let vm = state.viet_mode;
            drop(guard);
            update_tray_icon(hwnd, vm);
            crate::config::save();
        }
        // Kiểu gõ
        IDM_IM_TELEX => { state.set_input_method(0); drop(guard); crate::config::save(); }
        IDM_IM_STELEX => { state.set_input_method(1); drop(guard); crate::config::save(); }
        IDM_IM_VNI => { state.set_input_method(2); drop(guard); crate::config::save(); }
        IDM_IM_VIQR => { state.set_input_method(3); drop(guard); crate::config::save(); }
        IDM_IM_MSVI => { state.set_input_method(4); drop(guard); crate::config::save(); }
        // Bảng mã
        IDM_CS_UNICODE => { state.output_charset = 0; drop(guard); crate::config::save(); }
        IDM_CS_UTF8 => { state.output_charset = 1; drop(guard); crate::config::save(); }
        IDM_CS_NCRDEC => { state.output_charset = 2; drop(guard); crate::config::save(); }
        IDM_CS_NCRHEX => { state.output_charset = 3; drop(guard); crate::config::save(); }
        IDM_CS_CP1258 => { state.output_charset = 5; drop(guard); crate::config::save(); }
        IDM_CS_VIQR => { state.output_charset = 10; drop(guard); crate::config::save(); }
        IDM_CS_TCVN3 => { state.output_charset = 20; drop(guard); crate::config::save(); }
        IDM_CS_VPS => { state.output_charset = 21; drop(guard); crate::config::save(); }
        IDM_CS_VISCII => { state.output_charset = 22; drop(guard); crate::config::save(); }
        IDM_CS_VNU => { state.output_charset = 25; drop(guard); crate::config::save(); }
        IDM_CS_VNIWIN => { state.output_charset = 40; drop(guard); crate::config::save(); }
        IDM_CS_VNIMAC => { state.output_charset = 43; drop(guard); crate::config::save(); }
        // Tùy chọn
        IDM_OPT_SPELL => {
            state.spell_check = !state.spell_check;
            state.sync_options();
            drop(guard);
            crate::config::save();
        }
        IDM_OPT_FREE => {
            state.free_marking = !state.free_marking;
            state.sync_options();
            drop(guard);
            crate::config::save();
        }
        IDM_OPT_MODERN => {
            state.modern_style = !state.modern_style;
            state.sync_options();
            drop(guard);
            crate::config::save();
        }
        IDM_OPT_EDE => {
            state.ede_mode = !state.ede_mode;
            state.sync_options();
            drop(guard);
            crate::config::save();
        }
        IDM_OPT_MACRO => {
            state.macro_enabled = !state.macro_enabled;
            state.sync_options();
            drop(guard);
            crate::config::save();
        }
        IDM_CONFIG => {
            drop(guard);
            crate::gui::open_config_window();
            return;
        }
        IDM_CONVERTER => {
            drop(guard);
            crate::converter::open_converter_window();
            return;
        }
        IDM_BLACKLIST => {
            drop(guard);
            crate::blacklist::open_blacklist_window();
            return;
        }
        IDM_INFO => {
            drop(guard);
            crate::info::open_info_window();
            return;
        }
        IDM_HOTKEY => {
            drop(guard);
            crate::hotkey::open_hotkey_window();
            return;
        }
        IDM_APPCS => {
            drop(guard);
            crate::app_charset::open_app_charset_window();
            return;
        }
        IDM_MACROTBL => {
            drop(guard);
            crate::gui::open_macro_window();
            return;
        }
        IDM_RUNAS => {
            drop(guard);
            let new_val = !get_run_as_admin();
            set_run_as_admin(new_val);
            crate::config::save_now();
            if new_val && !is_elevated() {
                relaunch_as_admin();
            } else if !new_val && is_elevated() {
                relaunch_normal();
            }
            return;
        }
        IDM_EXIT => unsafe {
            PostQuitMessage(0);
        },
        _ => {}
    }
}
