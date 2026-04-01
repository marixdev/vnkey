//! Bảng mã theo ứng dụng — logic portable, không phụ thuộc platform.
//!
//! Mỗi app có thể dùng bảng mã khác nhau.
//! Ví dụ: app1.exe → TCVN3, app2.exe → BKHCM1, còn lại dùng mặc định.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{LazyLock, Mutex};

/// Danh sách bảng mã theo ứng dụng: exe_name (lowercase) → charset_id
pub static APP_CHARSETS: LazyLock<Mutex<HashMap<String, i32>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Charset override cho app hiện tại (-1 = dùng mặc định)
static CURRENT_APP_CHARSET: AtomicI32 = AtomicI32::new(-1);

/// Tra charset override cho app hiện tại. Trả về None nếu dùng mặc định.
pub fn get_current_app_charset() -> Option<i32> {
    let v = CURRENT_APP_CHARSET.load(Ordering::Relaxed);
    if v < 0 { None } else { Some(v) }
}

/// Cập nhật charset override dựa trên tên exe (gọi từ platform khi foreground thay đổi).
/// Truyền exe_name đã lowercase.
pub fn update_app_charset_for(exe_name: Option<&str>) {
    let cs = exe_name
        .and_then(|exe| {
            APP_CHARSETS.lock().ok()?.get(exe).copied()
        })
        .unwrap_or(-1);
    CURRENT_APP_CHARSET.store(cs, Ordering::Relaxed);
}

// ── Bảng mã hỗ trợ ─────────────────────────────────────────────────────

pub const CS_IDS: [i32; 11] = [0, 1, 2, 3, 5, 10, 20, 21, 22, 40, 43];
pub const CS_NAMES: [&str; 11] = [
    "Unicode", "UTF-8", "NCR Decimal", "NCR Hex", "CP-1258",
    "VIQR", "TCVN3 (ABC)", "VPS", "VISCII", "VNI Windows", "VNI Mac",
];

pub fn cs_name(id: i32) -> &'static str {
    CS_IDS.iter().zip(CS_NAMES.iter())
        .find(|(&cid, _)| cid == id)
        .map(|(_, name)| *name)
        .unwrap_or("UTF-8")
}

pub fn cs_index(id: i32) -> usize {
    CS_IDS.iter().position(|&v| v == id).unwrap_or(1)
}

pub fn cs_value(idx: usize) -> i32 {
    CS_IDS.get(idx).copied().unwrap_or(1)
}

// ── Serialization cho config.json ───────────────────────────────────────

/// Xuất danh sách app_charset thành chuỗi JSON object.
/// Ví dụ: {"wps.exe": 20, "myapp.exe": 23}
pub fn to_json() -> String {
    let map = match APP_CHARSETS.lock() {
        Ok(m) => m,
        Err(_) => return "{}".to_string(),
    };
    if map.is_empty() {
        return "{}".to_string();
    }
    let entries: Vec<String> = map.iter()
        .map(|(exe, cs)| format!("\"{}\": {}", exe, cs))
        .collect();
    format!("{{{}}}", entries.join(", "))
}

/// Nạp danh sách app_charset từ chuỗi JSON object.
/// Chấp nhận: {"wps.exe": 20, "myapp.exe": 23}
pub fn from_json(s: &str) {
    let mut map = match APP_CHARSETS.lock() {
        Ok(m) => m,
        Err(_) => return,
    };
    map.clear();

    let s = s.trim();
    let s = s.strip_prefix('{').unwrap_or(s);
    let s = s.strip_suffix('}').unwrap_or(s);
    for pair in s.split(',') {
        let pair = pair.trim();
        if let Some(colon) = pair.find(':') {
            let key = pair[..colon].trim().trim_matches('"').to_ascii_lowercase();
            let val = pair[colon + 1..].trim().trim_matches('"');
            if let Ok(cs) = val.parse::<i32>() {
                if !key.is_empty() {
                    map.insert(key, cs);
                }
            }
        }
    }
}
