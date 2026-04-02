//! Cửa sổ cấu hình — tao + wry (WebView2).

use crate::webview::{self, UiEvent};
use crate::ENGINE;
use std::sync::atomic::{AtomicBool, Ordering};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

static GUI_OPEN: AtomicBool = AtomicBool::new(false);

pub fn open_config_window() {
    if GUI_OPEN.swap(true, Ordering::SeqCst) { return; }
    std::thread::spawn(|| {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_config()));
        if let Err(e) = r { eprintln!("[gui] panic: {e:?}"); }
        GUI_OPEN.store(false, Ordering::Relaxed);
    });
}

// ── CSS index ↔ charset id ──────────────────────────────────────────────────

const CS_IDS: [i32; 11] = [0, 1, 2, 3, 5, 10, 20, 21, 22, 40, 43];
fn cs_index(cs: i32) -> usize { CS_IDS.iter().position(|&v| v == cs).unwrap_or(0) }
fn cs_value(idx: usize) -> i32 { CS_IDS.get(idx).copied().unwrap_or(0) }

// ── Auto-start (registry) ───────────────────────────────────────────────────

fn is_auto_start_enabled() -> bool {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("reg")
        .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "VnKey"])
        .creation_flags(0x08000000).output()
        .map(|o| o.status.success()).unwrap_or(false)
}

fn set_auto_start(enable: bool) {
    use std::os::windows::process::CommandExt;
    if enable {
        let exe = std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        let _ = std::process::Command::new("reg")
            .args(["add", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                   "/v", "VnKey", "/t", "REG_SZ", "/d", &format!("\"{exe}\""), "/f"])
            .creation_flags(0x08000000).output();
    } else {
        let _ = std::process::Command::new("reg")
            .args(["delete", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                   "/v", "VnKey", "/f"])
            .creation_flags(0x08000000).output();
    }
}

fn relaunch_as_admin() {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::Win32::Foundation::HWND;
    use windows::core::*;
    unsafe {
        let mut buf = [0u16; 260];
        let len = windows::Win32::System::LibraryLoader::GetModuleFileNameW(None, &mut buf);
        if len > 0 {
            ShellExecuteW(HWND::default(), w!("runas"), PCWSTR(buf.as_ptr()),
                PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL);
            if let Ok(mw) = windows::Win32::UI::WindowsAndMessaging::FindWindowW(
                w!("VnKeyHiddenWindow"), w!("VnKey")) {
                let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                    mw, windows::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                    windows::Win32::Foundation::WPARAM(0),
                    windows::Win32::Foundation::LPARAM(0));
            }
        }
    }
}

fn relaunch_normal() {
    use windows::core::*;
    unsafe {
        let mut buf = [0u16; 260];
        let len = windows::Win32::System::LibraryLoader::GetModuleFileNameW(None, &mut buf);
        if len > 0 {
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            let mut si: windows::Win32::System::Threading::STARTUPINFOW = std::mem::zeroed();
            si.cb = std::mem::size_of_val(&si) as u32;
            let mut pi: windows::Win32::System::Threading::PROCESS_INFORMATION = std::mem::zeroed();
            let mut cmd: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = windows::Win32::System::Threading::CreateProcessW(
                None, PWSTR(cmd.as_mut_ptr()), None, None, false,
                windows::Win32::System::Threading::PROCESS_CREATION_FLAGS(0),
                None, None, &si, &mut pi);
            let _ = windows::Win32::Foundation::CloseHandle(pi.hProcess);
            let _ = windows::Win32::Foundation::CloseHandle(pi.hThread);
            if let Ok(mw) = windows::Win32::UI::WindowsAndMessaging::FindWindowW(
                w!("VnKeyHiddenWindow"), w!("VnKey")) {
                let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                    mw, windows::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                    windows::Win32::Foundation::WPARAM(0),
                    windows::Win32::Foundation::LPARAM(0));
            }
        }
    }
}

// ── HTML ─────────────────────────────────────────────────────────────────────

fn build_html(im: i32, cs: i32, spell: bool, free: bool, modern: bool,
              ede: bool, auto_start: bool, run_admin: bool) -> String {
    let cs_idx = cs_index(cs);
    let cs_names = ["Unicode","UTF-8","NCR Decimal","NCR Hex","CP-1258",
        "VIQR","TCVN3 (ABC)","VPS","VISCII","VNU","VNI Windows","VNI Mac"];
    let cs_opts: String = cs_names.iter().enumerate().map(|(i, n)|
        if i == cs_idx { format!(r#"<option value="{i}" selected>{n}</option>"#) }
        else { format!(r#"<option value="{i}">{n}</option>"#) }
    ).collect();

    let im_names = ["Telex","Simple Telex","VNI","VIQR","MS Vietnamese"];
    let im_opts: String = im_names.iter().enumerate().map(|(i, n)|
        if i == im as usize { format!(r#"<option value="{i}" selected>{n}</option>"#) }
        else { format!(r#"<option value="{i}">{n}</option>"#) }
    ).collect();

    fn ck(v: bool) -> &'static str { if v { "checked" } else { "" } }

    let body = format!(r##"
<div class="container">
  <div style="display:flex;align-items:center;gap:10px">
    <div style="width:36px;height:36px;background:var(--accent);border-radius:8px;display:flex;align-items:center;justify-content:center;color:#fff;font-size:18px;font-weight:700">V</div>
    <div>
      <div style="font-weight:600;font-size:14px">VnKey</div>
      <div style="font-size:11px;color:var(--text-dim)">Phiên bản {ver}</div>
    </div>
    <span style="flex:1"></span>
    <span class="link" onclick="cmd({{cmd:'info'}})">Giới thiệu</span>
  </div>
  <div style="display:flex;gap:12px;align-items:stretch">
    <div style="flex:1;display:flex;flex-direction:column;gap:8px;min-width:0">
      <div class="row" style="gap:8px">
        <div class="group">
          <div class="group-title">Bảng mã</div>
          <select id="cs" onchange="cmd({{cmd:'cs',v:+this.value}})">{cs_opts}</select>
        </div>
        <div class="group">
          <div class="group-title">Kiểu gõ</div>
          <select id="im" onchange="cmd({{cmd:'im',v:+this.value}})">{im_opts}</select>
        </div>
      </div>
      <div class="group" style="flex:1">
        <div class="group-title">Tùy chọn</div>
        <div class="checkbox-grid">
          <label class="cb-item"><input type="checkbox" {spell} onchange="cmd({{cmd:'spell',v:this.checked}})">Kiểm tra chính tả</label>
          <label class="cb-item"><input type="checkbox" {free} onchange="cmd({{cmd:'free',v:this.checked}})">Bỏ dấu tự do</label>
          <label class="cb-item"><input type="checkbox" {modern} onchange="cmd({{cmd:'modern',v:this.checked}})">Kiểu mới (oà, uý)</label>
          <label class="cb-item"><input type="checkbox" {ede} onchange="cmd({{cmd:'ede',v:this.checked}})">Tiếng Tây Nguyên (Êđê)</label>
          <label class="cb-item"><input type="checkbox" {autostart} onchange="cmd({{cmd:'autostart',v:this.checked}})">Khởi động cùng Windows</label>
          <label class="cb-item"><input type="checkbox" {admin} onchange="cmd({{cmd:'admin',v:this.checked}})">Chạy với quyền Admin</label>
        </div>
      </div>
    </div>
    <div style="width:150px;display:flex;flex-direction:column;gap:6px">
      <div class="group-title" style="margin:0;padding-left:2px">Công cụ</div>
      <button class="full tool-btn" onclick="cmd({{cmd:'blacklist'}})"><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="4.93" y1="4.93" x2="19.07" y2="19.07"/></svg> Loại trừ</button>
      <button class="full tool-btn" onclick="cmd({{cmd:'converter'}})"><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="17 1 21 5 17 9"/><path d="M3 11V9a4 4 0 0 1 4-4h14"/><polyline points="7 23 3 19 7 15"/><path d="M21 13v2a4 4 0 0 1-4 4H3"/></svg> Chuyển mã</button>
      <button class="full tool-btn" onclick="cmd({{cmd:'hotkey'}})"><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="4" width="20" height="16" rx="2"/><path d="M6 8h.01M10 8h.01M14 8h.01M18 8h.01M6 12h.01M10 12h.01M14 12h.01M18 12h.01M8 16h8"/></svg> Gán phím</button>
      <button class="full tool-btn" onclick="cmd({{cmd:'appcs'}})"><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18"/><path d="M9 21V9"/></svg> Bảng mã theo app</button>
      <div style="flex:1"></div>
      <button class="full primary" onclick="cmd({{cmd:'close'}})"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg> Đóng</button>
      <button class="full danger" onclick="cmd({{cmd:'exit'}})"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/></svg> Thoát</button>
    </div>
  </div>
</div>
"##,
        ver = VERSION,
        spell = ck(spell), free = ck(free), modern = ck(modern),
        ede = ck(ede), autostart = ck(auto_start), admin = ck(run_admin),
    );

    webview::html(&body, "")
}

// ── IPC handler ──────────────────────────────────────────────────────────────

fn handle_ipc(body: &str, proxy: tao::event_loop::EventLoopProxy<UiEvent>) {
    let msg: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v, Err(_) => return,
    };
    let cmd = msg["cmd"].as_str().unwrap_or("");
    match cmd {
        "cs" => {
            let idx = msg["v"].as_i64().unwrap_or(0) as usize;
            if let Ok(mut g) = ENGINE.lock() {
                if let Some(s) = g.as_mut() { s.output_charset = cs_value(idx); }
            }
            crate::config::save();
        }
        "im" => {
            let idx = msg["v"].as_i64().unwrap_or(0) as i32;
            if let Ok(mut g) = ENGINE.lock() {
                if let Some(s) = g.as_mut() { s.set_input_method(idx); }
            }
            crate::config::save();
        }
        "spell" => {
            let v = msg["v"].as_bool().unwrap_or(true);
            if let Ok(mut g) = ENGINE.lock() {
                if let Some(s) = g.as_mut() { s.spell_check = v; s.sync_options(); }
            }
            crate::config::save();
        }
        "free" => {
            let v = msg["v"].as_bool().unwrap_or(true);
            if let Ok(mut g) = ENGINE.lock() {
                if let Some(s) = g.as_mut() { s.free_marking = v; s.sync_options(); }
            }
            crate::config::save();
        }
        "modern" => {
            let v = msg["v"].as_bool().unwrap_or(true);
            if let Ok(mut g) = ENGINE.lock() {
                if let Some(s) = g.as_mut() { s.modern_style = v; s.sync_options(); }
            }
            crate::config::save();
        }
        "ede" => {
            let v = msg["v"].as_bool().unwrap_or(false);
            if let Ok(mut g) = ENGINE.lock() {
                if let Some(s) = g.as_mut() { s.ede_mode = v; s.sync_options(); }
            }
            crate::config::save();
        }
        "autostart" => { set_auto_start(msg["v"].as_bool().unwrap_or(false)); }
        "admin" => {
            let v = msg["v"].as_bool().unwrap_or(false);
            crate::tray::set_run_as_admin(v);
            crate::config::save();
            if v && !crate::tray::is_elevated() { relaunch_as_admin(); }
            else if !v && crate::tray::is_elevated() { relaunch_normal(); }
        }
        "blacklist" => { crate::blacklist::open_blacklist_window(); }
        "converter" => { crate::converter::open_converter_window(); }
        "hotkey" => { crate::hotkey::open_hotkey_window(); }
        "appcs" => { crate::app_charset::open_app_charset_window(); }
        "info" => { crate::info::open_info_window(); }
        "close" => { let _ = proxy.send_event(UiEvent::Close); }
        "exit" => {
            use windows::Win32::Foundation::*;
            use windows::Win32::UI::WindowsAndMessaging::*;
            use windows::core::w;
            unsafe {
                if let Ok(mw) = FindWindowW(w!("VnKeyHiddenWindow"), w!("VnKey")) {
                    let _ = PostMessageW(mw, WM_CLOSE, WPARAM(0), LPARAM(0));
                }
            }
        }
        _ => {}
    }
}

fn run_config() {
    let (im, cs, spell, free, modern, ede) = {
        let g = ENGINE.lock().unwrap_or_else(|e| e.into_inner());
        match g.as_ref() {
            Some(s) => (s.input_method, s.output_charset, s.spell_check, s.free_marking, s.modern_style, s.ede_mode),
            None => (0, 1, true, true, true, false),
        }
    };
    let auto_start = is_auto_start_enabled();
    let run_admin = crate::tray::get_run_as_admin();
    let html = build_html(im, cs, spell, free, modern, ede, auto_start, run_admin);
    webview::run_webview(&format!("VnKey {VERSION}"), 560.0, 330.0, &html, handle_ipc);
}
