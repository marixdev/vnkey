#![windows_subsystem = "windows"]

mod app_charset;
mod blacklist;
mod config;
mod converter;
pub mod debug_log;
mod gui;
mod hook;
mod hotkey;
mod info;
mod osd;
mod tray;
mod send;
mod webview;

use std::sync::Mutex;
use vnkey_engine::{Engine, InputMethod, Options};

use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::Foundation::*;
use windows::core::*;

/// Tin nhắn tuỳ chỉnh để gửi ký tự qua SendInput
pub const WM_VNKEY_OUTPUT: u32 = WM_USER + 1;
/// Tin nhắn tuỳ chỉnh cho lệnh menu tray
pub const WM_VNKEY_TRAY: u32 = WM_USER + 2;

/// Giá trị đánh dấu phím do VnKey tự inject ("VNKY")
pub const VNKEY_INJECTED_TAG: usize = 0x564E4B59;

/// Tin nhắn mở cửa sổ cấu hình (từ instance thứ 2)
pub const WM_VNKEY_SHOW_CONFIG: u32 = WM_USER + 3;

/// Tin nhắn cập nhật icon tray khi chuyển chế độ
pub const WM_VNKEY_UPDATE_ICON: u32 = WM_USER + 4;

/// Tin nhắn đăng ký lại phím tắt (chạy trên thread chính)
pub const WM_VNKEY_REREGISTER_HOTKEYS: u32 = WM_USER + 5;

/// Trạng thái engine toàn cục, bảo vệ bởi mutex
pub static ENGINE: Mutex<Option<EngineState>> = Mutex::new(None);

/// Dữ liệu đầu ra chờ gửi sau khi hook trả về
pub static PENDING_OUTPUT: Mutex<Option<PendingOutput>> = Mutex::new(None);

pub struct PendingOutput {
    pub backspaces: usize,
    pub text: String,
    pub raw_bytes: Option<Vec<u8>>,
}

pub struct EngineState {
    pub engine: Engine,
    pub input_method: i32,
    pub output_charset: i32,
    pub viet_mode: bool,
    pub spell_check: bool,
    pub free_marking: bool,
    pub modern_style: bool,
    pub ede_mode: bool,
}

impl EngineState {
    fn new() -> Self {
        let mut engine = Engine::new();
        engine.set_input_method(InputMethod::Telex);
        engine.set_viet_mode(true);
        engine.options = Options {
            free_marking: true,
            modern_style: true,
            spell_check_enabled: true,
            auto_non_vn_restore: true,
            macro_enabled: false,
            strict_spell_check: false,
            ede_mode: false,
        };

        Self {
            engine,
            input_method: 0,
            output_charset: 1,
            viet_mode: true,
            spell_check: true,
            free_marking: true,
            modern_style: true,
            ede_mode: false,
        }
    }

    pub fn set_input_method(&mut self, im: i32) {
        self.input_method = im;
        let method = match im {
            0 => InputMethod::Telex,
            1 => InputMethod::SimpleTelex,
            2 => InputMethod::Vni,
            3 => InputMethod::Viqr,
            4 => InputMethod::MsVi,
            _ => InputMethod::Telex,
        };
        self.engine.set_input_method(method);
    }

    pub fn toggle_viet_mode(&mut self) {
        self.viet_mode = !self.viet_mode;
        self.engine.set_viet_mode(self.viet_mode);
        self.engine.reset();
    }

    pub fn sync_options(&mut self) {
        self.engine.options.free_marking = self.free_marking;
        self.engine.options.modern_style = self.modern_style;
        self.engine.options.spell_check_enabled = self.spell_check;
        self.engine.options.ede_mode = self.ede_mode;
    }
}

fn main() {
    // Kiểm tra instance duy nhất qua named mutex
    let mutex_name = w!("Global\\VnKeyInputMethod");
    let already_running;
    unsafe {
        let result = windows::Win32::System::Threading::CreateMutexW(None, true, mutex_name);
        already_running = match result {
            Ok(_) => windows::Win32::Foundation::GetLastError() == ERROR_ALREADY_EXISTS,
            Err(_) => return,
        };
    }

    // Nếu đã chạy, báo instance hiện tại mở cửa sổ cấu hình
    if already_running {
        unsafe {
            if let Ok(hwnd) = FindWindowW(w!("VnKeyHiddenWindow"), w!("VnKey")) {
                let _ = PostMessageW(hwnd, WM_VNKEY_SHOW_CONFIG, WPARAM(0), LPARAM(0));
            }
        }
        return;
    }

    // Khởi tạo debug log (tạo file %TEMP%\vnkey_debug để bật)
    debug_log::init();
    debug_log::log("=== VnKey started ===");

    // Khởi tạo engine
    *ENGINE.lock().unwrap_or_else(|e| e.into_inner()) = Some(EngineState::new());

    // Tải cài đặt đã lưu
    config::load();

    // Tự nâng quyền Admin nếu đã bật tùy chọn và chưa chạy elevated
    if tray::get_run_as_admin() && !tray::is_elevated() {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        unsafe {
            let mut buf = [0u16; 260];
            let len = windows::Win32::System::LibraryLoader::GetModuleFileNameW(None, &mut buf);
            if len > 0 {
                ShellExecuteW(HWND::default(), w!("runas"), PCWSTR(buf.as_ptr()),
                    PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL);
                return; // thoát instance thường, instance admin sẽ chạy
            }
        }
    }

    // Tạo cửa sổ ẩn cho message pump
    let hwnd = create_hidden_window();
    if hwnd == HWND::default() {
        return;
    }

    // Cài đặt hook bàn phím
    let hook = hook::install_hook();
    if hook.is_err() {
        return;
    }

    // Tạo biểu tượng khay hệ thống
    {
        let viet_mode = ENGINE.lock().ok()
            .and_then(|g| g.as_ref().map(|s| s.viet_mode))
            .unwrap_or(true);
        tray::create_tray_icon(hwnd, viet_mode);
    }

    // Đăng ký phím tắt đã lưu (bật/tắt + chuyển mã)
    hotkey::register_hotkeys(hwnd);

    // Timer cài đặt lại hook định kỳ (mỗi 5 giây).
    // Phòng trường hợp Windows gỡ hook do callback vượt thời gian cho phép
    // (LowLevelHooksTimeout). Đây là lỗi phổ biến trên trình duyệt nặng
    // như Facebook, khiến bộ gõ "mất tiếng" cho đến khi bật/tắt lại.
    const TIMER_HOOK_HEALTH: usize = 1;
    unsafe {
        SetTimer(hwnd, TIMER_HOOK_HEALTH, 5000, None);
    }

    // Hiện cửa sổ cấu hình lần chạy đầu
    gui::open_config_window();

    // Vòng lặp tin nhắn
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
            if msg.message == WM_VNKEY_OUTPUT {
                send::send_pending_output();
            } else if msg.message == WM_VNKEY_UPDATE_ICON {
                let viet_mode = msg.wParam.0 != 0;
                tray::update_tray_icon(hwnd, viet_mode);
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // Dọn dẹp
    hook::uninstall_hook();
    tray::remove_tray_icon(hwnd);
}

fn create_hidden_window() -> HWND {
    unsafe {
        let class_name = w!("VnKeyHiddenWindow");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            lpszClassName: class_name,
            hInstance: windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .unwrap_or_default()
                .into(),
            ..Default::default()
        };
        RegisterClassW(&wc);

        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("VnKey"),
            WINDOW_STYLE::default(),
            0, 0, 0, 0,
            None,
            None,
            wc.hInstance,
            None,
        )
        .unwrap_or_default()
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_VNKEY_OUTPUT => {
            send::send_pending_output();
            LRESULT(0)
        }
        WM_VNKEY_TRAY => {
            tray::handle_tray_message(hwnd, lparam);
            LRESULT(0)
        }
        WM_VNKEY_SHOW_CONFIG => {
            gui::open_config_window();
            LRESULT(0)
        }
        WM_VNKEY_UPDATE_ICON => {
            let viet_mode = wparam.0 != 0;
            tray::update_tray_icon(hwnd, viet_mode);
            LRESULT(0)
        }
        WM_VNKEY_REREGISTER_HOTKEYS => {
            hotkey::unregister_hotkeys(hwnd);
            hotkey::register_hotkeys(hwnd);
            LRESULT(0)
        }
        WM_HOTKEY => {
            let id = wparam.0 as i32;
            if id == crate::hotkey::HOTKEY_ID_TOGGLE {
                if let Ok(mut guard) = ENGINE.lock() {
                    if let Some(state) = guard.as_mut() {
                        state.toggle_viet_mode();
                        let vm = state.viet_mode;
                        drop(guard);
                        tray::update_tray_icon(hwnd, vm);
                        config::save();
                        osd::show(if vm { "Tiếng Việt" } else { "English" });
                    }
                }
            } else if id == crate::hotkey::HOTKEY_ID_CONVERT {
                converter::convert_clipboard();
            } else if let Some(ch) = crate::hotkey::custom_hotkey_for_id(id) {
                use crate::hotkey::HkKind;
                match ch.kind {
                    HkKind::Cs => {
                        if let Ok(mut guard) = ENGINE.lock() {
                            if let Some(state) = guard.as_mut() {
                                state.output_charset = ch.id;
                            }
                        }
                        config::save();
                        let cs_names = ["Unicode","UTF-8","NCR Decimal","NCR Hex","","CP-1258",
                            "","","","","VIQR","","","","","","","","","","TCVN3 (ABC)",
                            "VPS","VISCII","","","VNU","","","","","","","","","","","","","","","VNI Windows",
                            "","","VNI Mac"];
                        let name = cs_names.get(ch.id as usize).unwrap_or(&"?");
                        osd::show(&format!("Bảng mã: {name}"));
                    }
                    HkKind::Im => {
                        if let Ok(mut guard) = ENGINE.lock() {
                            if let Some(state) = guard.as_mut() {
                                state.set_input_method(ch.id);
                            }
                        }
                        config::save();
                        let im_names = ["Telex","Simple Telex","VNI","VIQR","MS Vietnamese"];
                        let name = im_names.get(ch.id as usize).unwrap_or(&"?");
                        osd::show(&format!("Kiểu gõ: {name}"));
                    }
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            tray::handle_menu_command(hwnd, wparam.0 as u16);
            LRESULT(0)
        }
        WM_TIMER => {
            // Cài đặt lại hook định kỳ để phòng hook bị Windows gỡ
            hook::reinstall_hook();
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
