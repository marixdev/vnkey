//! Shared WebView2 helper — tao + wry.
//! Mỗi cửa sổ dialog chạy trên thread riêng với event loop riêng.

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::platform::run_return::EventLoopExtRunReturn;
use tao::platform::windows::EventLoopBuilderExtWindows;
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

/// Sự kiện tùy chỉnh gửi từ IPC handler → event loop
#[derive(Debug)]
pub enum UiEvent {
    Eval(String),
    Show,
    Close,
}

/// Tạo và chạy một cửa sổ webview (blocking, chạy trên thread hiện tại).
pub fn run_webview<F>(title: &str, w: f64, h: f64, html: &str, ipc: F)
where
    F: Fn(&str, EventLoopProxy<UiEvent>) + 'static,
{
    // WebView2 cần COM trên thread hiện tại
    unsafe {
        #[link(name = "ole32")]
        extern "system" {
            fn CoInitializeEx(pvreserved: *const std::ffi::c_void, dwcoinit: u32) -> i32;
        }
        let _ = CoInitializeEx(std::ptr::null(), 0x2); // COINIT_APARTMENTTHREADED
    }

    let mut event_loop = EventLoopBuilder::<UiEvent>::with_user_event()
        .with_any_thread(true)
        .build();
    let proxy = event_loop.create_proxy();

    let window = match WindowBuilder::new()
        .with_title(title)
        .with_inner_size(LogicalSize::new(w, h))
        .with_resizable(false)
        .with_maximizable(false)
        .with_focused(true)
        .with_always_on_top(false)
        .with_visible(false)
        .build(&event_loop)
    {
        Ok(w) => w,
        Err(e) => { eprintln!("[webview] WindowBuilder FAILED: {e}"); return; }
    };

    // Đặt icon từ resource
    {
        use tao::platform::windows::WindowExtWindows;
        use windows::Win32::Foundation::{HWND, WPARAM, LPARAM};
        use windows::Win32::UI::WindowsAndMessaging::*;
        let hwnd = HWND(window.hwnd() as *mut std::ffi::c_void);
        unsafe {
            let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .unwrap_or_default();
            if let Ok(icon) = LoadIconW(hinstance, windows::core::PCWSTR(1 as *const u16)) {
                SendMessageW(hwnd, WM_SETICON, WPARAM(0), LPARAM(icon.0 as isize));
                SendMessageW(hwnd, WM_SETICON, WPARAM(1), LPARAM(icon.0 as isize));
            }
        }
    }

    let proxy_ipc = proxy.clone();
    let proxy_show = proxy.clone();
    let webview = match WebViewBuilder::new()
        .with_html(html)
        .with_background_color((243, 243, 243, 255)) // var(--bg) #f3f3f3
        .with_initialization_script("function cmd(obj){if(window.ipc&&window.ipc.postMessage){window.ipc.postMessage(JSON.stringify(obj));}}\nif(document.readyState==='loading'){document.addEventListener('DOMContentLoaded',function(){window.ipc.postMessage(JSON.stringify({cmd:'__ready'}));});}else{window.ipc.postMessage(JSON.stringify({cmd:'__ready'}));}")
        .with_ipc_handler(move |req| {
            let body = req.body();
            // Khi webview sẵn sàng → hiện cửa sổ
            if body.contains("__ready") {
                let _ = proxy_show.send_event(UiEvent::Show);
                return;
            }
            ipc(body, proxy_ipc.clone());
        })
        .build(&window)
    {
        Ok(w) => w,
        Err(e) => { eprintln!("[webview] WebViewBuilder FAILED: {e}"); return; }
    };

    event_loop.run_return(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UiEvent::Eval(script)) => {
                let _ = webview.evaluate_script(&script);
            }
            Event::UserEvent(UiEvent::Show) => {
                window.set_visible(true);
                window.set_focus();
                // SetForegroundWindow
                use tao::platform::windows::WindowExtWindows;
                use windows::Win32::Foundation::HWND;
                let hwnd = HWND(window.hwnd() as *mut std::ffi::c_void);
                unsafe { let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd); }
            }
            Event::UserEvent(UiEvent::Close) => {
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}

// ── CSS chung cho tất cả cửa sổ ──────────────────────────────────────────

pub const CSS: &str = r##"
:root {
    --bg: #f3f3f3;
    --surface: #ffffff;
    --surface-hover: #e9e9e9;
    --accent: #0067c0;
    --accent-hover: #005499;
    --accent-light: #e1f0ff;
    --text: #1a1a1a;
    --text-dim: #5d5d5d;
    --border: #d1d1d1;
    --danger: #c42b1c;
    --danger-bg: #fde7e9;
    --success: #0f7b0f;
    --radius: 6px;
    --shadow: 0 1px 4px rgba(0,0,0,0.08);
}
* { margin: 0; padding: 0; box-sizing: border-box; }
body {
    font-family: 'Segoe UI Variable', 'Segoe UI', system-ui, sans-serif;
    background: var(--bg);
    color: var(--text);
    font-size: 13px;
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
}
.container { padding: 14px; display: flex; flex-direction: column; gap: 10px; }

/* ── Card/Group ── */
.group {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 10px 12px;
    box-shadow: var(--shadow);
}
.group-title {
    font-size: 11px;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.4px;
    margin-bottom: 6px;
    font-weight: 600;
}

/* ── Inputs ── */
select, input[type="text"], textarea {
    width: 100%;
    padding: 6px 10px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text);
    font-size: 13px;
    font-family: inherit;
    outline: none;
    transition: border-color 0.15s;
}
select:focus, input[type="text"]:focus, textarea:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
}
select { cursor: pointer; }
select option { background: var(--surface); color: var(--text); }
textarea { resize: vertical; min-height: 80px; }

/* ── Checkbox ── */
.checkbox-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 4px 14px; }
.cb-item {
    display: flex; align-items: center; gap: 7px;
    cursor: pointer; padding: 2px 0; font-size: 13px;
}
.cb-item input[type="checkbox"] {
    -webkit-appearance: none; appearance: none;
    width: 18px; height: 18px; flex-shrink: 0;
    border: 1.5px solid var(--border); border-radius: 4px;
    background: var(--surface); cursor: pointer;
    display: grid; place-content: center;
    transition: all 0.15s;
}
.cb-item input[type="checkbox"]::after {
    content: '✓'; font-size: 12px; font-weight: 700;
    color: #fff; transform: scale(0);
    transition: transform 0.12s;
}
.cb-item input[type="checkbox"]:checked {
    background: var(--accent); border-color: var(--accent);
}
.cb-item input[type="checkbox"]:checked::after { transform: scale(1); }

/* ── Buttons ── */
button, .btn {
    display: inline-flex; align-items: center; justify-content: center; gap: 5px;
    padding: 6px 14px;
    border: 1px solid var(--border); border-radius: 4px;
    background: var(--surface); color: var(--text);
    font-size: 13px; font-family: inherit;
    cursor: pointer; transition: all 0.12s;
    white-space: nowrap;
    box-shadow: var(--shadow);
}
button:hover { background: var(--surface-hover); border-color: #bbb; }
button:active { transform: scale(0.97); box-shadow: none; }
button.primary {
    background: var(--accent); color: #fff;
    border-color: var(--accent); font-weight: 600;
}
button.primary:hover { background: var(--accent-hover); border-color: var(--accent-hover); }
button.danger { border-color: var(--danger); color: var(--danger); background: var(--surface); }
button.danger:hover { background: var(--danger); color: #fff; }
button.full { width: 100%; }
button:disabled { opacity: 0.4; cursor: not-allowed; }

/* ── Layout ── */
.row { display: flex; gap: 10px; }
.row > * { flex: 1; }
.col-2 { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }

/* ── Header ── */
.header { text-align: center; padding-bottom: 2px; }
.header h1 {
    font-size: 16px; font-weight: 600; color: var(--accent);
}

/* ── Link ── */
.link {
    color: var(--accent); cursor: pointer; text-decoration: none;
    font-size: 12px;
}
.link:hover { text-decoration: underline; }

/* ── Status ── */
.status { font-size: 12px; color: var(--text-dim); min-height: 16px; }
.status.ok { color: var(--success); }
.status.err { color: var(--danger); }

/* ── Tool button (icon left-aligned) ── */
.tool-btn { justify-content: flex-start; padding-left: 12px; }
.tool-btn svg { flex-shrink: 0; opacity: 0.7; }

/* ── Hotkey box ── */
.hotkey-box {
    display: flex; align-items: center;
    background: var(--surface); border: 1px solid var(--border);
    border-radius: 4px; padding: 4px 10px;
    font-size: 12px; font-weight: 600;
    min-width: 110px; justify-content: center;
    box-shadow: var(--shadow);
    transition: border-color 0.15s;
    white-space: nowrap;
}
.hotkey-box.capturing {
    border-color: var(--accent); color: var(--accent);
    animation: pulse 1.2s ease-in-out infinite;
}
@keyframes pulse {
    0%, 100% { opacity: 1; } 50% { opacity: 0.5; }
}

/* ── Hotkey row ── */
.hk-row {
    display: flex; align-items: center; gap: 6px;
    padding: 3px 0;
}
.hk-row + .hk-row { border-top: 1px solid var(--bg); }
.hk-label { flex: 1; font-size: 13px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.hk-row button { padding: 3px 10px; font-size: 12px; }

/* ── List ── */
.list-box {
    background: var(--surface); border: 1px solid var(--border);
    border-radius: 4px; max-height: 160px; overflow-y: auto;
    min-height: 80px; box-shadow: inset 0 1px 2px rgba(0,0,0,0.04);
}
.list-box .item {
    padding: 5px 10px; cursor: pointer; transition: background 0.1s;
    border-bottom: 1px solid #eee;
}
.list-box .item:last-child { border-bottom: none; }
.list-box .item:hover { background: var(--accent-light); }
.list-box .item.selected { background: var(--accent); color: #fff; }
.list-box .empty { padding: 14px; text-align: center; color: var(--text-dim); font-size: 12px; }

/* ── Scrollbar ── */
::-webkit-scrollbar { width: 6px; }
::-webkit-scrollbar-thumb { background: #c0c0c0; border-radius: 3px; }
::-webkit-scrollbar-thumb:hover { background: #a0a0a0; }
"##;

/// Tạo HTML hoàn chỉnh từ body + script
pub fn html(body: &str, script: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><style>{CSS}</style></head>
<body>{body}
<script>{script}</script>
</body></html>"#
    )
}
