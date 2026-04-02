//! Gửi phím qua SendInput sau khi hook trả về
//!
//! Mặc định dùng Shift+Left để chọn và thay thế ký tự (tránh Chrome autocomplete
//! nuốt VK_BACK). Fallback về VK_BACK cho các app không hỗ trợ Shift+Left (WPS Office, v.v.).
//! Console apps dùng WriteConsoleInputW cho ký tự Unicode (KEYEVENTF_UNICODE bị PSReadLine bỏ qua).
//! Tất cả sự kiện được đánh dấu VNKEY_INJECTED_TAG để hook bỏ qua.

use crate::{PENDING_OUTPUT, VNKEY_INJECTED_TAG};

use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
use windows::Win32::System::Console::*;

/// Dùng VK_BACK thay vì Shift+Left cho ứng dụng hiện tại.
/// Được cập nhật từ hook khi detect foreground app.
static USE_VK_BACK: AtomicBool = AtomicBool::new(false);

/// Foreground là console app (cần WriteConsoleInputW cho Unicode text).
static IS_CONSOLE: AtomicBool = AtomicBool::new(false);

/// Foreground là VM/RDP app — KHÔNG chặn phím, để native pass-through.
/// VMware/VirtualBox/RDP có lớp ảo hóa bàn phím riêng. Nếu hook chặn phím
/// và inject lại qua SendInput, VM nhận phím tổng hợp thay vì phím thật
/// → gõ password login OS trong VM không được, phím bị nuốt.
static IS_VM: AtomicBool = AtomicBool::new(false);

/// Foreground là chính VnKey (WebView2 dialog). Hook pass-through hoàn toàn,
/// Vietnamese input được xử lý qua JavaScript + IPC bên trong WebView2.
static IS_SELF: AtomicBool = AtomicBool::new(false);

/// Firefox-based: cần gửi U+202F (empty char) trước backspace.
/// Spell checker/autocomplete Firefox can thiệp với Shift+Left khi đang
/// có suggestion. Gửi U+202F phá vỡ spell checker state, sau đó
/// Shift+Left hoạt động bình thường. Thêm 1 backspace để xóa U+202F.
/// (Kỹ thuật từ OpenKey: vFixRecommendBrowser)
static FIX_RECOMMEND: AtomicBool = AtomicBool::new(false);

/// Cờ đánh dấu VnKey đang inject phím qua SendInput.
/// Hook kiểm tra cờ này ngoài dwExtraInfo để nhận diện phím của chính mình,
/// phòng trường hợp VMware/RDP/VNC không giữ nguyên dwExtraInfo.
/// An toàn vì hook và SendInput chạy trên cùng thread (message loop).
static SENDING: AtomicBool = AtomicBool::new(false);

pub fn is_sending() -> bool {
    SENDING.load(Ordering::Relaxed)
}

/// Danh sách process cần dùng VK_BACK (Shift+Left không hoạt động)
const VK_BACK_APPS: &[&str] = &[
    "wps.exe", "wpp.exe", "et.exe",     // WPS Office
    "WINWORD.EXE", "EXCEL.EXE", "POWERPNT.EXE", // MS Office
    "librewolf.exe", "waterfox.exe", "mullvad-browser.exe",
    // Electron apps có custom editor (Shift+Left bị chặn/hoạt động sai)
    "Notion.exe",
];

/// Firefox-based browsers: cần gửi U+202F trước backspace.
/// Spell checker/autocomplete can thiệp với Shift+Left.
/// Gửi ký tự rỗng U+202F phá vỡ spell checker state,
/// sau đó Shift+Left hoạt động bình thường.
/// (Kỹ thuật từ OpenKey: vFixRecommendBrowser + SendEmptyCharacter)
const RECOMMEND_FIX_APPS: &[&str] = &[
    "firefox.exe", "zen.exe",
];

/// Console apps: cần VK_BACK + WriteConsoleInputW cho Unicode text.
/// KEYEVENTF_UNICODE bị PSReadLine bỏ qua trong PowerShell.
const CONSOLE_APPS: &[&str] = &[
    "cmd.exe", "powershell.exe", "pwsh.exe",
    "WindowsTerminal.exe", "wt.exe",
    "conhost.exe", // Console host (foreground window khi chạy cmd/ps legacy)
];

/// VM/RDP apps: hook KHÔNG chặn phím, để native pass-through hoàn toàn.
/// VMware fullscreen grab keyboard ở mức thấp. VirtualBox tương tự.
/// RDP client cũng chuyển phím sang máy remote.
/// Gõ tiếng Việt bên trong VM/RDP nên cài IME trong guest OS.
const VM_APPS: &[&str] = &[
    "vmware.exe", "vmplayer.exe", "vmware-vmx.exe",     // VMware
    "vmconnect.exe",                                     // Hyper-V
    "VirtualBox.exe", "VirtualBoxVM.exe",                // VirtualBox
    "mstsc.exe", "msrdc.exe",                            // RDP client
    "vncviewer.exe",                                     // VNC
];

/// Kiểm tra và cập nhật phương thức xuất dựa trên ứng dụng hiện tại.
/// Gọi từ hook mỗi lần foreground thay đổi.
pub fn update_backspace_method() {
    let exe = crate::blacklist::get_foreground_exe_cached();

    // Detect VM/RDP: pass-through hoàn toàn
    let is_vm = exe.as_ref()
        .map(|e| VM_APPS.iter().any(|app| app.eq_ignore_ascii_case(e)))
        .unwrap_or(false);
    IS_VM.store(is_vm, Ordering::Relaxed);
    if is_vm {
        crate::debug_log::log(&format!("  VM_MODE exe={:?}", exe));
    }

    // Detect self (VnKey's own WebView2 windows): hook pass-through,
    // Vietnamese input xử lý qua JS+IPC trong WebView2.
    let is_self = exe.as_ref()
        .map(|e| e.eq_ignore_ascii_case("vnkey.exe"))
        .unwrap_or(false);
    IS_SELF.store(is_self, Ordering::Relaxed);
    if is_self {
        crate::debug_log::log(&format!("  SELF_MODE exe={:?}", exe));
    }

    // Detect Firefox-based: cần gửi U+202F trước Shift+Left
    let fix_rec = exe.as_ref()
        .map(|e| RECOMMEND_FIX_APPS.iter().any(|app| app.eq_ignore_ascii_case(e)))
        .unwrap_or(false);
    FIX_RECOMMEND.store(fix_rec, Ordering::Relaxed);
    if fix_rec {
        crate::debug_log::log(&format!("  FIX_RECOMMEND_MODE exe={:?}", exe));
    }

    let is_console = exe.as_ref()
        .map(|e| CONSOLE_APPS.iter().any(|app| app.eq_ignore_ascii_case(e)))
        .unwrap_or(false);
    IS_CONSOLE.store(is_console, Ordering::Relaxed);
    let use_vk = is_console || exe.as_ref()
        .map(|e| VK_BACK_APPS.iter().any(|app| app.eq_ignore_ascii_case(e)))
        .unwrap_or(false);
    USE_VK_BACK.store(use_vk, Ordering::Relaxed);
    if is_console {
        crate::debug_log::log(&format!("  CONSOLE_MODE exe={:?}", exe));
    }
}

/// Console/Office apps cần VK_BACK thay vì Shift+Left.
/// Hook dùng flag này để quyết định pass-through native hay inject.
pub fn is_using_vk_back() -> bool {
    USE_VK_BACK.load(Ordering::Relaxed)
}

/// Foreground là VM/RDP — hook KHÔNG chặn, để native pass-through.
pub fn is_vm_app() -> bool {
    IS_VM.load(Ordering::Relaxed)
}

/// Foreground là chính VnKey — hook pass-through, JS+IPC xử lý Vietnamese.
pub fn is_self() -> bool {
    IS_SELF.load(Ordering::Relaxed)
}

pub fn send_pending_output() {
    let pending = {
        let Ok(mut guard) = PENDING_OUTPUT.lock() else { return };
        guard.take()
    };

    if let Some(output) = pending {
        if let Some(ref raw_bytes) = output.raw_bytes {
            send_backspaces_and_raw(output.backspaces, raw_bytes);
        } else {
            send_backspaces_and_text(output.backspaces, &output.text);
        }
    }
}

/// Gửi đầu ra trực tiếp (gọi từ hook callback để chuyển ngay).
pub fn send_output(backspaces: usize, text: &str, raw_bytes: Option<&[u8]>) {
    if let Some(raw) = raw_bytes {
        send_backspaces_and_raw(backspaces, raw);
    } else {
        send_backspaces_and_text(backspaces, text);
    }
}

/// Inject VK_BACK qua SendInput thay vì để native pass-through.
/// Đảm bảo backspace đi cùng đường SendInput như các ký tự,
/// tránh VMware/RDP bị lệch luồng giữa native VK_BACK và KEYEVENTF_UNICODE.
pub fn send_backspace() {
    let inputs = [
        make_key_input(VK_BACK, KEYBD_EVENT_FLAGS(0), VNKEY_INJECTED_TAG),
        make_key_input(VK_BACK, KEYEVENTF_KEYUP, VNKEY_INJECTED_TAG),
    ];
    unsafe {
        SENDING.store(true, Ordering::Relaxed);
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        SENDING.store(false, Ordering::Relaxed);
    }
}

/// Inject một phím VK bất kỳ qua SendInput (down+up).
/// Dùng cho Space, Enter, Tab, Escape, v.v. trong VMware/RDP
/// để tránh lệch luồng giữa native và SendInput.
pub fn send_key(vk: VIRTUAL_KEY) {
    let inputs = [
        make_key_input(vk, KEYBD_EVENT_FLAGS(0), VNKEY_INJECTED_TAG),
        make_key_input(vk, KEYEVENTF_KEYUP, VNKEY_INJECTED_TAG),
    ];
    unsafe {
        SENDING.store(true, Ordering::Relaxed);
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        SENDING.store(false, Ordering::Relaxed);
    }
}

/// Tạo chuỗi INPUT cho backspace: Shift+Left (select) nếu có text thay thế,
/// hoặc VK_BACK thuần nếu không.
fn build_backspace_inputs(inputs: &mut Vec<INPUT>, backspaces: usize, has_replacement: bool) {
    let force_vk_back = USE_VK_BACK.load(Ordering::Relaxed);

    if backspaces > 0 && has_replacement && !force_vk_back {
        // Dùng Shift+Left để chọn ký tự, rồi gõ văn bản thay thế.
        // Tránh VK_BACK vì Chrome Omnibox autocomplete chặn nó
        // (backspace đầu tiên xóa gợi ý thay vì xóa ký tự).
        inputs.push(make_key_input(VK_SHIFT, KEYBD_EVENT_FLAGS(0), VNKEY_INJECTED_TAG));
        for _ in 0..backspaces {
            inputs.push(make_key_input(VK_LEFT, KEYEVENTF_EXTENDEDKEY, VNKEY_INJECTED_TAG));
            inputs.push(make_key_input(VK_LEFT, KEYBD_EVENT_FLAGS(KEYEVENTF_KEYUP.0 | KEYEVENTF_EXTENDEDKEY.0), VNKEY_INJECTED_TAG));
        }
        inputs.push(make_key_input(VK_SHIFT, KEYEVENTF_KEYUP, VNKEY_INJECTED_TAG));
    } else {
        for _ in 0..backspaces {
            inputs.push(make_key_input(VK_BACK, KEYBD_EVENT_FLAGS(0), VNKEY_INJECTED_TAG));
            inputs.push(make_key_input(VK_BACK, KEYEVENTF_KEYUP, VNKEY_INJECTED_TAG));
        }
    }
}

fn send_backspaces_and_text(backspaces: usize, text: &str) {
    if IS_CONSOLE.load(Ordering::Relaxed) {
        send_console_output(backspaces, text.encode_utf16().collect::<Vec<u16>>().as_slice());
        return;
    }

    let mut inputs: Vec<INPUT> = Vec::new();
    let fix_rec = FIX_RECOMMEND.load(Ordering::Relaxed);
    let force_vk_back = USE_VK_BACK.load(Ordering::Relaxed);
    let effective_backspaces;

    // Firefox: gửi U+202F (empty char) trước để phá vỡ spell checker state.
    // Thêm 1 backspace để xóa U+202F. (Kỹ thuật từ OpenKey)
    if fix_rec && backspaces > 0 && !text.is_empty() && !force_vk_back {
        inputs.push(make_unicode_input(0x202F, KEYBD_EVENT_FLAGS(0), VNKEY_INJECTED_TAG));
        inputs.push(make_unicode_input(0x202F, KEYEVENTF_KEYUP, VNKEY_INJECTED_TAG));
        effective_backspaces = backspaces + 1;
    } else {
        effective_backspaces = backspaces;
    }

    build_backspace_inputs(&mut inputs, effective_backspaces, !text.is_empty());

    // Văn bản dưới dạng ký tự Unicode (KEYEVENTF_UNICODE)
    for ch in text.encode_utf16() {
        inputs.push(make_unicode_input(ch, KEYBD_EVENT_FLAGS(0), VNKEY_INJECTED_TAG));
        inputs.push(make_unicode_input(ch, KEYEVENTF_KEYUP, VNKEY_INJECTED_TAG));
    }

    if !inputs.is_empty() {
        unsafe {
            let t = crate::debug_log::timer_start();
            SENDING.store(true, Ordering::Relaxed);
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            SENDING.store(false, Ordering::Relaxed);
            crate::debug_log::log_elapsed(&format!("SEND_TEXT n={}", inputs.len()), t);
        }
    }
}

fn send_backspaces_and_raw(backspaces: usize, raw_bytes: &[u8]) {
    if IS_CONSOLE.load(Ordering::Relaxed) {
        let chars: Vec<u16> = raw_bytes.iter().map(|&b| b as u16).collect();
        send_console_output(backspaces, &chars);
        return;
    }

    let mut inputs: Vec<INPUT> = Vec::new();
    let fix_rec = FIX_RECOMMEND.load(Ordering::Relaxed);
    let force_vk_back = USE_VK_BACK.load(Ordering::Relaxed);
    let effective_backspaces;

    if fix_rec && backspaces > 0 && !raw_bytes.is_empty() && !force_vk_back {
        inputs.push(make_unicode_input(0x202F, KEYBD_EVENT_FLAGS(0), VNKEY_INJECTED_TAG));
        inputs.push(make_unicode_input(0x202F, KEYEVENTF_KEYUP, VNKEY_INJECTED_TAG));
        effective_backspaces = backspaces + 1;
    } else {
        effective_backspaces = backspaces;
    }

    build_backspace_inputs(&mut inputs, effective_backspaces, !raw_bytes.is_empty());

    for &b in raw_bytes {
        inputs.push(make_unicode_input(b as u16, KEYBD_EVENT_FLAGS(0), VNKEY_INJECTED_TAG));
        inputs.push(make_unicode_input(b as u16, KEYEVENTF_KEYUP, VNKEY_INJECTED_TAG));
    }

    if !inputs.is_empty() {
        unsafe {
            let t = crate::debug_log::timer_start();
            SENDING.store(true, Ordering::Relaxed);
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            SENDING.store(false, Ordering::Relaxed);
            crate::debug_log::log_elapsed(&format!("SEND_RAW n={}", inputs.len()), t);
        }
    }
}

/// Console: gửi VK_BACK qua SendInput, rồi Unicode text qua WriteConsoleInputW.
/// KEYEVENTF_UNICODE bị PSReadLine (PowerShell) bỏ qua, nên phải dùng
/// WriteConsoleInputW với KEY_EVENT_RECORD chứa UnicodeChar.
fn send_console_output(backspaces: usize, chars: &[u16]) {
    // Gửi VK_BACK qua SendInput (hook sẽ SKIP_OWN)
    if backspaces > 0 {
        let mut inputs: Vec<INPUT> = Vec::new();
        for _ in 0..backspaces {
            inputs.push(make_key_input(VK_BACK, KEYBD_EVENT_FLAGS(0), VNKEY_INJECTED_TAG));
            inputs.push(make_key_input(VK_BACK, KEYEVENTF_KEYUP, VNKEY_INJECTED_TAG));
        }
        unsafe {
            SENDING.store(true, Ordering::Relaxed);
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            SENDING.store(false, Ordering::Relaxed);
        }
    }

    // Gửi Unicode text qua WriteConsoleInputW
    if !chars.is_empty() {
        write_console_chars(chars);
    }
}

/// Attach vào console của foreground process và gửi Unicode chars
/// qua WriteConsoleInputW. Đây là cách duy nhất đáng tin cậy để
/// gửi Unicode (Vietnamese) vào PowerShell/CMD.
fn write_console_chars(chars: &[u16]) {
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 { return; }

        let _ = FreeConsole();
        if AttachConsole(pid).is_err() {
            crate::debug_log::log(&format!("  CONSOLE_ATTACH_FAILED pid={}", pid));
            return;
        }

        // Mở CONIN$ trực tiếp — đáng tin cậy hơn GetStdHandle khi attach
        let conin = windows::Win32::Storage::FileSystem::CreateFileW(
            windows::core::w!("CONIN$"),
            (windows::Win32::Storage::FileSystem::FILE_GENERIC_READ
                | windows::Win32::Storage::FileSystem::FILE_GENERIC_WRITE).0,
            windows::Win32::Storage::FileSystem::FILE_SHARE_READ
                | windows::Win32::Storage::FileSystem::FILE_SHARE_WRITE,
            None,
            windows::Win32::Storage::FileSystem::OPEN_EXISTING,
            windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
            None,
        );

        match conin {
            Ok(handle) => {
                let mut records: Vec<INPUT_RECORD> = Vec::with_capacity(chars.len() * 2);
                for &ch in chars {
                    // Key down
                    let mut rec: INPUT_RECORD = std::mem::zeroed();
                    rec.EventType = KEY_EVENT as u16;
                    rec.Event.KeyEvent.bKeyDown = windows::Win32::Foundation::BOOL(1);
                    rec.Event.KeyEvent.wRepeatCount = 1;
                    rec.Event.KeyEvent.uChar.UnicodeChar = ch;
                    records.push(rec);

                    // Key up
                    let mut rec: INPUT_RECORD = std::mem::zeroed();
                    rec.EventType = KEY_EVENT as u16;
                    rec.Event.KeyEvent.bKeyDown = windows::Win32::Foundation::BOOL(0);
                    rec.Event.KeyEvent.wRepeatCount = 1;
                    rec.Event.KeyEvent.uChar.UnicodeChar = ch;
                    records.push(rec);
                }

                let mut written: u32 = 0;
                let result = WriteConsoleInputW(
                    windows::Win32::Foundation::HANDLE(handle.0),
                    &records,
                    &mut written,
                );
                let _ = windows::Win32::Foundation::CloseHandle(handle);
                crate::debug_log::log(&format!(
                    "  CONSOLE_WRITE chars={} records={} written={} ok={}",
                    chars.len(), records.len(), written, result.is_ok()
                ));
            }
            Err(e) => {
                crate::debug_log::log(&format!("  CONSOLE_OPEN_FAILED: {}", e));
            }
        }

        let _ = FreeConsole();
    }
}

fn make_key_input(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS, extra: usize) -> INPUT {
    let scan = unsafe { MapVirtualKeyW(vk.0 as u32, MAPVK_VK_TO_VSC) as u16 };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: extra,
            },
        },
    }
}

fn make_unicode_input(ch: u16, flags: KEYBD_EVENT_FLAGS, extra: usize) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: ch,
                dwFlags: KEYEVENTF_UNICODE | flags,
                time: 0,
                dwExtraInfo: extra,
            },
        },
    }
}
