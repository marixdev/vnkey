//! Cửa sổ giới thiệu — tao + wry.

use crate::webview::{self, UiEvent};
use std::sync::atomic::{AtomicBool, Ordering};

static INFO_OPEN: AtomicBool = AtomicBool::new(false);

pub fn open_info_window() {
    if INFO_OPEN.swap(true, Ordering::SeqCst) { return; }
    std::thread::spawn(|| {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_info()));
        if let Err(e) = r { eprintln!("[info] panic: {e:?}"); }
        INFO_OPEN.store(false, Ordering::Relaxed);
    });
}

fn build_html() -> String {
    let ver = crate::gui::VERSION;
    let body = format!(r##"
<div class="container" style="text-align:center">
  <div style="padding:12px 0">
    <div style="font-size:48px">⌨</div>
    <h1 style="font-size:22px;color:var(--accent);margin:8px 0">VnKey {ver}</h1>
    <p style="color:var(--text-dim);font-size:13px">Bộ gõ tiếng Việt cho Windows</p>
  </div>
  <div class="group" style="text-align:left">
    <div class="group-title">Thông tin</div>
    <p style="font-size:13px;line-height:1.8;color:var(--text-dim)">
      <b style="color:var(--text)">Tác giả:</b> VnKey Team<br>
      <b style="color:var(--text)">Giấy phép:</b> GPL-3.0<br>
      <b style="color:var(--text)">Engine:</b> vnkey-engine (Rust)<br>
      <b style="color:var(--text)">GUI:</b> WebView2 + tao/wry
    </p>
  </div>
  <div style="display:flex;gap:8px;justify-content:center">
    <button onclick="cmd({{cmd:'url',url:'https://vnkey.app'}})">🌐 Website</button>
    <button onclick="cmd({{cmd:'url',url:'https://github.com/marixdev/vnkey'}})">GitHub</button>
    <button onclick="cmd({{cmd:'close'}})">Đóng</button>
  </div>
</div>
"##);
    webview::html(&body, "")
}

fn handle_ipc(body: &str, proxy: tao::event_loop::EventLoopProxy<UiEvent>) {
    let msg: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v, Err(_) => return,
    };
    let cmd = msg["cmd"].as_str().unwrap_or("");
    match cmd {
        "url" => {
            if let Some(url) = msg["url"].as_str() {
                // Mở URL bằng ShellExecuteW
                use windows::core::*;
                use windows::Win32::Foundation::HWND;
                use windows::Win32::UI::Shell::ShellExecuteW;
                use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
                let wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
                unsafe {
                    ShellExecuteW(HWND::default(), w!("open"),
                        PCWSTR(wide.as_ptr()), PCWSTR::null(), PCWSTR::null(),
                        SW_SHOWNORMAL);
                }
            }
        }
        "close" => { let _ = proxy.send_event(UiEvent::Close); }
        _ => {}
    }
}

fn run_info() {
    let html = build_html();
    webview::run_webview("Giới thiệu VnKey", 360.0, 420.0, &html, handle_ipc);
}
