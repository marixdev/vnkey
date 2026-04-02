//! VnKey Engine — Bộ xử lý gõ tiếng Việt đa nền tảng
//!
//! Viết bằng Rust, hỗ trợ Telex, VNI, VIQR và nhiều bảng mã đầu ra.

pub mod vnlexi;
pub mod input;
pub mod engine;
pub mod charset;
pub mod macro_table;
pub mod ffi;
pub mod app_charset;

pub use engine::{Engine, OutputType};
pub use input::InputMethod;
pub use vnlexi::{VnLexiName, VowelSeq, ConSeq};

/// Tùy chọn cho bộ xử lý gõ tiếng Việt
#[derive(Debug, Clone)]
pub struct Options {
    pub free_marking: bool,
    pub modern_style: bool,
    pub macro_enabled: bool,
    pub spell_check_enabled: bool,
    pub auto_non_vn_restore: bool,
    pub strict_spell_check: bool,
    pub ede_mode: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            free_marking: true,
            modern_style: true,
            macro_enabled: false,
            spell_check_enabled: true,
            auto_non_vn_restore: true,
            strict_spell_check: false,
            ede_mode: false,
        }
    }
}
