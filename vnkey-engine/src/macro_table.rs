//! Bảng macro: ánh xạ viết tắt sang văn bản tiếng Việt đầy đủ.

use std::collections::HashMap;

const MAX_MACRO_LEN: usize = 256;
const MAX_MACRO_ENTRIES: usize = 1024;

/// Một mục macro
#[derive(Debug, Clone)]
struct MacroEntry {
    key: String,
    value: String,
}

/// Bảng tra macro, so khớp không phân biệt hoa/thường
pub struct MacroTable {
    entries: Vec<MacroEntry>,
    lookup: HashMap<String, usize>,
}

impl MacroTable {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            lookup: HashMap::new(),
        }
    }

    /// Thêm mục macro. Trả false nếu bảng đầy hoặc key quá dài.
    pub fn add(&mut self, key: &str, value: &str) -> bool {
        if self.entries.len() >= MAX_MACRO_ENTRIES || key.len() > MAX_MACRO_LEN {
            return false;
        }
        let lower_key = key.to_lowercase();
        if let Some(&idx) = self.lookup.get(&lower_key) {
            self.entries[idx].value = value.to_string();
        } else {
            let idx = self.entries.len();
            self.entries.push(MacroEntry {
                key: key.to_string(),
                value: value.to_string(),
            });
            self.lookup.insert(lower_key, idx);
        }
        true
    }

    /// Tra macro theo key (không phân biệt hoa/thường)
    pub fn lookup(&self, key: &str) -> Option<&str> {
        let lower_key = key.to_lowercase();
        self.lookup.get(&lower_key).map(|&idx| self.entries[idx].value.as_str())
    }

    /// Xóa macro theo key
    pub fn remove(&mut self, key: &str) -> bool {
        let lower_key = key.to_lowercase();
        if let Some(&idx) = self.lookup.get(&lower_key) {
            self.entries.swap_remove(idx);
            self.lookup.remove(&lower_key);
            // Cập nhật chỉ mục của mục đã hoán đổi
            if idx < self.entries.len() {
                let swapped_key = self.entries[idx].key.to_lowercase();
                self.lookup.insert(swapped_key, idx);
            }
            true
        } else {
            false
        }
    }

    /// Xóa tất cả macro
    pub fn clear(&mut self) {
        self.entries.clear();
        self.lookup.clear();
    }

    /// Số lượng macro
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Tải macro từ văn bản. Định dạng: mỗi dòng "key\tvalue"
    pub fn load_from_text(&mut self, text: &str) {
        self.clear();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('\t') {
                let key = key.trim();
                let value = value.trim();
                if !key.is_empty() && !value.is_empty() {
                    self.add(key, value);
                }
            }
        }
    }

    /// Xuất tất cả macro thành văn bản. Định dạng: mỗi dòng "key\tvalue"
    pub fn to_text(&self) -> String {
        let mut result = String::new();
        for entry in &self.entries {
            result.push_str(&entry.key);
            result.push('\t');
            result.push_str(&entry.value);
            result.push('\n');
        }
        result
    }

    /// Trả danh sách tất cả macro dưới dạng JSON array.
    /// Định dạng: [{"key":"bc","value":"báo cáo"}, ...]
    pub fn to_json(&self) -> String {
        let mut result = String::from("[");
        for (i, entry) in self.entries.iter().enumerate() {
            if i > 0 {
                result.push(',');
            }
            result.push_str("{\"key\":\"");
            for ch in entry.key.chars() {
                match ch {
                    '"' => result.push_str("\\\""),
                    '\\' => result.push_str("\\\\"),
                    '\n' => result.push_str("\\n"),
                    _ => result.push(ch),
                }
            }
            result.push_str("\",\"value\":\"");
            for ch in entry.value.chars() {
                match ch {
                    '"' => result.push_str("\\\""),
                    '\\' => result.push_str("\\\\"),
                    '\n' => result.push_str("\\n"),
                    _ => result.push(ch),
                }
            }
            result.push_str("\"}");
        }
        result.push(']');
        result
    }

    /// Tải macro từ JSON array: [{"key":"bc","value":"báo cáo"}, ...]
    pub fn load_from_json(&mut self, json: &str) {
        self.clear();
        // Trích xuất từng object {key, value} đơn giản
        let mut pos = 0;
        let bytes = json.as_bytes();
        while pos < bytes.len() {
            // Tìm "key":"
            if let Some(k_start) = json[pos..].find("\"key\":\"") {
                let k_start = pos + k_start + 7; // bỏ "key":"
                if let Some(k_end) = json[k_start..].find('"') {
                    let key = &json[k_start..k_start + k_end];
                    let after_key = k_start + k_end;
                    // Tìm "value":"
                    if let Some(v_start) = json[after_key..].find("\"value\":\"") {
                        let v_start = after_key + v_start + 9;
                        if let Some(v_end) = json[v_start..].find('"') {
                            let value = &json[v_start..v_start + v_end];
                            if !key.is_empty() && !value.is_empty() {
                                // Unescape
                                let key = key.replace("\\\"", "\"").replace("\\n", "\n").replace("\\\\", "\\");
                                let value = value.replace("\\\"", "\"").replace("\\n", "\n").replace("\\\\", "\\");
                                self.add(&key, &value);
                            }
                            pos = v_start + v_end + 1;
                            continue;
                        }
                    }
                }
            }
            break;
        }
    }
}

impl Default for MacroTable {
    fn default() -> Self {
        Self::new()
    }
}
