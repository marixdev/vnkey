//! Debug logging cho hook — ghi lại mọi sự kiện bàn phím để chẩn đoán lỗi.
//! Chỉ bật khi biên dịch với feature "debug_hook" hoặc file %TEMP%\vnkey_debug tồn tại.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::io::Write;
use std::time::Instant;

static ENABLED: AtomicBool = AtomicBool::new(false);
static LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// Khởi tạo debug log: kiểm tra file %TEMP%\vnkey_debug, nếu có thì bật log.
pub fn init() {
    let temp = std::env::var("TEMP").unwrap_or_else(|_| r"C:\Temp".to_string());
    let marker = std::path::Path::new(&temp).join("vnkey_debug");
    if marker.exists() {
        let log_path = std::path::Path::new(&temp).join("vnkey_debug.log");
        if let Ok(f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            if let Ok(mut guard) = LOG_FILE.lock() {
                *guard = Some(f);
            }
            ENABLED.store(true, Ordering::Relaxed);
        }
    }
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn log(msg: &str) {
    if !is_enabled() {
        return;
    }
    if let Ok(mut guard) = LOG_FILE.lock() {
        if let Some(ref mut f) = *guard {
            let _ = writeln!(f, "{}", msg);
            let _ = f.flush();
        }
    }
}

/// Log chi tiết một sự kiện hook
pub fn log_hook_event(
    vk: u32,
    scan: u32,
    flags: u32,
    extra: usize,
    time: u32,
    action: &str,
) {
    if !is_enabled() {
        return;
    }
    let msg = format!(
        "[{}] vk=0x{:02X} scan=0x{:02X} flags=0x{:02X} extra=0x{:X} | {}",
        time, vk, scan, flags, extra, action
    );
    log(&msg);
}

/// Log kết quả engine
pub fn log_engine_result(
    ascii: u8,
    processed: bool,
    backspaces: usize,
    output: &str,
) {
    if !is_enabled() {
        return;
    }
    let msg = format!(
        "  engine: ascii='{}' (0x{:02X}) processed={} backs={} output={:?}",
        ascii as char, ascii, processed, backspaces, output
    );
    log(&msg);
}

/// Log modifier state
pub fn log_modifiers(ctrl: bool, alt: bool, win: bool, shift: bool) {
    if !is_enabled() {
        return;
    }
    if ctrl || alt || win {
        let msg = format!(
            "  modifiers: ctrl={} alt={} win={} shift={}",
            ctrl, alt, win, shift
        );
        log(&msg);
    }
}

/// Bắt đầu đo thời gian xử lý hook
pub fn timer_start() -> Instant {
    Instant::now()
}

/// Log thời gian đã trôi qua (microseconds)
pub fn log_elapsed(label: &str, start: Instant) {
    if !is_enabled() {
        return;
    }
    let elapsed = start.elapsed().as_micros();
    log(&format!("  {} elapsed={}us", label, elapsed));
}
