//! Định nghĩa kiểu gõ và xử lý sự kiện phím.
//! Hỗ trợ Telex, VNI, VIQR và kiểu gõ tùy chỉnh.

use crate::vnlexi::{VnLexiName, iso_to_vn_lexi};

/// Các kiểu gõ được hỗ trợ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMethod {
    Telex,
    SimpleTelex,
    Vni,
    Viqr,
    MsVi,
    UserDefined,
}

/// Loại sự kiện phím
#[derive(Debug, Clone, Copy, PartialEq, Eq, num_enum::TryFromPrimitive)]
#[repr(u8)]
pub enum KeyEvType {
    RoofAll = 0,
    RoofA,
    RoofE,
    RoofO,
    HookAll,
    HookUO,
    HookU,
    HookO,
    Bowl,
    Dd,
    Tone0,
    Tone1,
    Tone2,
    Tone3,
    Tone4,
    Tone5,
    TelexW,
    MapChar,
    EscChar,
    Normal,
}

impl KeyEvType {
    /// An toàn: chuyển u8 sang KeyEvType, trả về Normal nếu ngoài phạm vi
    pub fn from_u8(v: u8) -> Self {
        Self::try_from(v).unwrap_or(Self::Normal)
    }
}

/// Phân loại ký tự
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharType {
    Vn,
    WordBreak,
    NonVn,
    Reset,
}

/// Sự kiện phím đã xử lý
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub ev_type: KeyEvType,
    pub ch_type: CharType,
    pub vn_sym: VnLexiName,
    pub key_code: u32,
    pub tone: i32,
}

/// Ánh xạ phím: gán phím tới hành động
#[derive(Debug, Clone, Copy)]
pub struct KeyMapping {
    pub key: u8,
    pub action: i32,
}

pub const VNE_COUNT: i32 = KeyEvType::Normal as i32 + 1;

// ============= Bảng kiểu gõ tích hợp =============

pub static TELEX_MAPPING: &[KeyMapping] = &[
    KeyMapping { key: b'Z', action: KeyEvType::Tone0 as i32 },
    KeyMapping { key: b'S', action: KeyEvType::Tone1 as i32 },
    KeyMapping { key: b'F', action: KeyEvType::Tone2 as i32 },
    KeyMapping { key: b'R', action: KeyEvType::Tone3 as i32 },
    KeyMapping { key: b'X', action: KeyEvType::Tone4 as i32 },
    KeyMapping { key: b'J', action: KeyEvType::Tone5 as i32 },
    KeyMapping { key: b'W', action: KeyEvType::TelexW as i32 },
    KeyMapping { key: b'A', action: KeyEvType::RoofA as i32 },
    KeyMapping { key: b'E', action: KeyEvType::RoofE as i32 },
    KeyMapping { key: b'O', action: KeyEvType::RoofO as i32 },
    KeyMapping { key: b'D', action: KeyEvType::Dd as i32 },
    KeyMapping { key: b'[', action: VNE_COUNT + VnLexiName::oh as i32 },
    KeyMapping { key: b']', action: VNE_COUNT + VnLexiName::uh as i32 },
    KeyMapping { key: b'{', action: VNE_COUNT + VnLexiName::Oh as i32 },
    KeyMapping { key: b'}', action: VNE_COUNT + VnLexiName::Uh as i32 },
];

pub static SIMPLE_TELEX_MAPPING: &[KeyMapping] = &[
    KeyMapping { key: b'Z', action: KeyEvType::Tone0 as i32 },
    KeyMapping { key: b'S', action: KeyEvType::Tone1 as i32 },
    KeyMapping { key: b'F', action: KeyEvType::Tone2 as i32 },
    KeyMapping { key: b'R', action: KeyEvType::Tone3 as i32 },
    KeyMapping { key: b'X', action: KeyEvType::Tone4 as i32 },
    KeyMapping { key: b'J', action: KeyEvType::Tone5 as i32 },
    KeyMapping { key: b'W', action: KeyEvType::HookAll as i32 },
    KeyMapping { key: b'A', action: KeyEvType::RoofA as i32 },
    KeyMapping { key: b'E', action: KeyEvType::RoofE as i32 },
    KeyMapping { key: b'O', action: KeyEvType::RoofO as i32 },
    KeyMapping { key: b'D', action: KeyEvType::Dd as i32 },
];

pub static VNI_MAPPING: &[KeyMapping] = &[
    KeyMapping { key: b'0', action: KeyEvType::Tone0 as i32 },
    KeyMapping { key: b'1', action: KeyEvType::Tone1 as i32 },
    KeyMapping { key: b'2', action: KeyEvType::Tone2 as i32 },
    KeyMapping { key: b'3', action: KeyEvType::Tone3 as i32 },
    KeyMapping { key: b'4', action: KeyEvType::Tone4 as i32 },
    KeyMapping { key: b'5', action: KeyEvType::Tone5 as i32 },
    KeyMapping { key: b'6', action: KeyEvType::RoofAll as i32 },
    KeyMapping { key: b'7', action: KeyEvType::HookUO as i32 },
    KeyMapping { key: b'8', action: KeyEvType::Bowl as i32 },
    KeyMapping { key: b'9', action: KeyEvType::Dd as i32 },
];

pub static VIQR_MAPPING: &[KeyMapping] = &[
    KeyMapping { key: b'0', action: KeyEvType::Tone0 as i32 },
    KeyMapping { key: b'\'', action: KeyEvType::Tone1 as i32 },
    KeyMapping { key: b'`', action: KeyEvType::Tone2 as i32 },
    KeyMapping { key: b'?', action: KeyEvType::Tone3 as i32 },
    KeyMapping { key: b'~', action: KeyEvType::Tone4 as i32 },
    KeyMapping { key: b'.', action: KeyEvType::Tone5 as i32 },
    KeyMapping { key: b'^', action: KeyEvType::RoofAll as i32 },
    KeyMapping { key: b'+', action: KeyEvType::HookUO as i32 },
    KeyMapping { key: b'*', action: KeyEvType::HookUO as i32 },
    KeyMapping { key: b'(', action: KeyEvType::Bowl as i32 },
    KeyMapping { key: b'D', action: KeyEvType::Dd as i32 },
    KeyMapping { key: b'\\', action: KeyEvType::EscChar as i32 },
];

/// Ký hiệu ngắt từ
static WORD_BREAK_SYMS: &[u8] = &[
    b',', b';', b':', b'.', b'"', b'\'', b'!', b'?', b' ',
    b'<', b'>', b'=', b'+', b'-', b'*', b'/', b'\\',
    b'_', b'@', b'#', b'$', b'%', b'&', b'(', b')', b'{', b'}', b'[', b']', b'|',
];

/// Bộ xử lý đầu vào: chuyển mã phím thành sự kiện
pub struct InputProcessor {
    im: InputMethod,
    key_map: [i32; 256],
}

impl InputProcessor {
    pub fn new() -> Self {
        let mut proc = Self {
            im: InputMethod::Telex,
            key_map: [KeyEvType::Normal as i32; 256],
        };
        proc.set_im(InputMethod::Telex);
        proc
    }

    pub fn get_im(&self) -> InputMethod {
        self.im
    }

    pub fn set_im(&mut self, im: InputMethod) {
        self.im = im;
        let mapping = match im {
            InputMethod::Telex => TELEX_MAPPING,
            InputMethod::SimpleTelex => SIMPLE_TELEX_MAPPING,
            InputMethod::Vni => VNI_MAPPING,
            InputMethod::Viqr => VIQR_MAPPING,
            InputMethod::MsVi | InputMethod::UserDefined => {
                // Đặt lại bình thường rồi thoát
                self.key_map = [KeyEvType::Normal as i32; 256];
                return;
            }
        };
        self.use_built_in(mapping);
    }

    pub fn set_user_key_map(&mut self, map: &[i32; 256]) {
        self.im = InputMethod::UserDefined;
        self.key_map = *map;
    }

    fn use_built_in(&mut self, map: &[KeyMapping]) {
        self.key_map = [KeyEvType::Normal as i32; 256];
        for entry in map {
            self.key_map[entry.key as usize] = entry.action;
            if entry.action < VNE_COUNT {
                let ch = entry.key;
                if ch.is_ascii_lowercase() {
                    self.key_map[ch.to_ascii_uppercase() as usize] = entry.action;
                } else if ch.is_ascii_uppercase() {
                    self.key_map[ch.to_ascii_lowercase() as usize] = entry.action;
                }
            }
        }
    }

    /// Phân loại mã phím thành loại ký tự
    pub fn get_char_type(&self, key_code: u32) -> CharType {
        if key_code > 255 {
            return CharType::NonVn;
        }
        let ch = key_code as u8;
        if ch <= 32 {
            return CharType::Reset;
        }
        if WORD_BREAK_SYMS.contains(&ch) {
            return CharType::WordBreak;
        }
        match ch {
            b'a'..=b'z' | b'A'..=b'Z' => {
                // j, f, w mặc định là NonVn (kiểu gõ có thể ghi đè)
                match ch.to_ascii_lowercase() {
                    b'j' | b'f' | b'w' => CharType::NonVn,
                    _ => CharType::Vn,
                }
            }
            _ => CharType::NonVn,
        }
    }

    /// Chuyển mã phím thành sự kiện phím
    pub fn key_code_to_event(&self, key_code: u32) -> KeyEvent {
        if key_code > 255 {
            let vn_sym = iso_to_vn_lexi(key_code);
            return KeyEvent {
                ev_type: KeyEvType::Normal,
                ch_type: if vn_sym == VnLexiName::NonVnChar { CharType::NonVn } else { CharType::Vn },
                vn_sym,
                key_code,
                tone: 0,
            };
        }

        let ch_type = self.get_char_type(key_code);
        let ev_action = self.key_map[key_code as usize];
        let mut tone = 0i32;

        let ev_type;
        let vn_sym;

        if ev_action >= KeyEvType::Tone0 as i32 && ev_action <= KeyEvType::Tone5 as i32 {
            tone = ev_action - KeyEvType::Tone0 as i32;
            ev_type = KeyEvType::from_u8(ev_action as u8);
            vn_sym = iso_to_vn_lexi(key_code);
        } else if ev_action >= VNE_COUNT {
            // Ánh xạ ký tự: action chứa mã VnLexiName
            let mapped_sym = VnLexiName::from_i16((ev_action - VNE_COUNT) as i16);
            ev_type = KeyEvType::MapChar;
            vn_sym = mapped_sym;
            return KeyEvent {
                ev_type,
                ch_type: CharType::Vn,
                vn_sym,
                key_code,
                tone,
            };
        } else {
            ev_type = KeyEvType::from_u8(ev_action as u8);
            vn_sym = iso_to_vn_lexi(key_code);
        }

        KeyEvent {
            ev_type,
            ch_type,
            vn_sym,
            key_code,
            tone,
        }
    }
}

impl Default for InputProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::KeyEvType;

    #[test]
    fn key_event_conversion_rejects_out_of_range_values() {
        assert_eq!(KeyEvType::from_u8(KeyEvType::RoofAll as u8), KeyEvType::RoofAll);
        assert_eq!(KeyEvType::from_u8(KeyEvType::Normal as u8), KeyEvType::Normal);
        assert_eq!(KeyEvType::from_u8(u8::MAX), KeyEvType::Normal);
    }
}
