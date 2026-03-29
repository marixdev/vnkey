//! Engine xử lý phím — máy trạng thái nhận phím và tạo đầu ra tiếng Việt.

use crate::vnlexi::*;
use crate::vnlexi::VowelSeq::*;
use crate::vnlexi::ConSeq::*;
use crate::input::*;
use crate::Options;
use crate::macro_table::MacroTable;

const MAX_BUFFER: usize = 128;

/// Loại đầu ra từ engine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputType {
    Char,
    Key,
}

/// Phân loại dạng từ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordForm {
    NonVn,
    Empty,
    C,   // chỉ phụ âm
    V,   // chỉ nguyên âm
    CV,  // phụ âm + nguyên âm
    VC,  // nguyên âm + phụ âm
    CVC, // phụ âm + nguyên âm + phụ âm
}

/// Mục đệm theo dõi trạng thái ghép từ
#[derive(Debug, Clone)]
struct WordInfo {
    form: WordForm,
    c1_offset: i32,
    v_offset: i32,
    c2_offset: i32,
    // vseq khi dạng từ có nguyên âm, cseq khi chỉ có phụ âm
    vseq: VowelSeq,
    cseq: ConSeq,
    // Thông tin ký hiệu hiện tại
    caps: bool,
    tone: i32,
    vn_sym: VnLexiName,
    key_code: u32,
}

impl Default for WordInfo {
    fn default() -> Self {
        Self {
            form: WordForm::Empty,
            c1_offset: -1,
            v_offset: -1,
            c2_offset: -1,
            vseq: VowelSeq::Nil,
            cseq: ConSeq::Nil,
            caps: false,
            tone: 0,
            vn_sym: VnLexiName::NonVnChar,
            key_code: 0,
        }
    }
}

/// Mục đệm gõ phím
#[derive(Debug, Clone)]
struct KeyBufEntry {
    ev: KeyEvent,
    _converted: bool,
}

/// Kết quả xử lý phím
#[derive(Debug, Clone)]
pub struct ProcessResult {
    pub backspaces: usize,
    pub output: Vec<u8>,
    pub out_type: OutputType,
    pub processed: bool,
}

/// Engine gõ tiếng Việt chính
pub struct Engine {
    buffer: Vec<WordInfo>,
    current: i32,
    change_pos: i32,
    backs: usize,
    single_mode: bool,
    to_escape: bool,
    reverted: bool,

    key_strokes: Vec<KeyBufEntry>,
    key_current: i32,

    /// Trạng thái lưu cho phục hồi soft-reset (backspace sau dấu cách)
    saved_current: i32,
    saved_key_current: i32,
    saved_single_mode: bool,
    saved_to_escape: bool,
    has_saved_state: bool,

    pub input: InputProcessor,
    pub options: Options,
    pub viet_key: bool,
    pub macro_table: MacroTable,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            buffer: vec![WordInfo::default(); MAX_BUFFER],
            current: -1,
            change_pos: 0,
            backs: 0,
            single_mode: false,
            to_escape: false,
            reverted: false,

            key_strokes: Vec::with_capacity(MAX_BUFFER),
            key_current: -1,

            saved_current: -1,
            saved_key_current: -1,
            saved_single_mode: false,
            saved_to_escape: false,
            has_saved_state: false,

            input: InputProcessor::new(),
            options: Options::default(),
            viet_key: true,
            macro_table: MacroTable::new(),
        }
    }

    /// Đặt kiểu gõ
    pub fn set_input_method(&mut self, im: InputMethod) {
        self.input.set_im(im);
    }

    /// Bật/tắt chế độ tiếng Việt
    pub fn set_viet_mode(&mut self, enabled: bool) {
        self.viet_key = enabled;
    }

    /// Đặt lại trạng thái engine
    pub fn reset(&mut self) {
        self.current = -1;
        self.key_current = -1;
        self.single_mode = false;
        self.to_escape = false;
        self.has_saved_state = false;
    }

    /// Soft reset: lưu trạng thái để backspace có thể khôi phục
    pub fn soft_reset(&mut self) {
        self.saved_current = self.current;
        self.saved_key_current = self.key_current;
        self.saved_single_mode = self.single_mode;
        self.saved_to_escape = self.to_escape;
        self.has_saved_state = self.current >= 0;
        self.current = -1;
        self.key_current = -1;
        self.single_mode = false;
        self.to_escape = false;
    }

    /// Khôi phục trạng thái từ soft reset
    fn restore_saved_state(&mut self) -> bool {
        if self.has_saved_state {
            self.current = self.saved_current;
            self.key_current = self.saved_key_current;
            self.single_mode = self.saved_single_mode;
            self.to_escape = self.saved_to_escape;
            self.has_saved_state = false;
            true
        } else {
            false
        }
    }

    /// Kiểm tra đang ở đầu từ hay không
    pub fn at_word_beginning(&self) -> bool {
        self.current < 0
    }

    /// Nạp ngữ cảnh trước (surrounding text) vào engine mà không tạo đầu ra.
    /// Dùng để khôi phục trạng thái khi preedit bắt đầu giữa từ đã commit.
    /// Trả true nếu nạp thành công (ít nhất 1 ký tự được xử lý).
    pub fn feed_context(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        self.reset();
        let mut fed = false;
        for ch in text.chars() {
            let code = ch as u32;
            // Chỉ nạp ký tự ASCII in được
            if code < 0x21 || code > 0x7E {
                // Ký tự không phải ASCII → reset và bỏ qua
                self.reset();
                fed = false;
                continue;
            }
            let _ = self.process(code);
            fed = true;
        }
        // Reset output state nhưng giữ buffer state
        self.backs = 0;
        self.change_pos = self.current + 1;
        fed
    }

    /// Xử lý một lần gõ phím
    pub fn process(&mut self, key_code: u32) -> ProcessResult {
        self.has_saved_state = false; // Phím mới bất kỳ đều hủy trạng thái đã lưu
        self.prepare_buffer();
        self.backs = 0;
        self.change_pos = self.current + 1;
        self.reverted = false;

        let ev = self.input.key_code_to_event(key_code);

        let ret;
        if !self.to_escape {
            ret = self.dispatch_event(ev.clone());
        } else {
            self.to_escape = false;
            if self.current < 0 || ev.ev_type == KeyEvType::Normal || ev.ev_type == KeyEvType::EscChar {
                ret = self.process_append(ev.clone());
            } else {
                self.current -= 1;
                self.process_append(ev.clone());
                self.mark_change(self.current);
                ret = true;
            }
        }

        // Xử lý khi tắt kiểm tra chính tả
        if self.viet_key
            && self.current >= 0
            && self.buf(self.current).form == WordForm::NonVn
            && ev.ch_type == CharType::Vn
            && (!self.options.spell_check_enabled || self.single_mode)
        {
            self.process_no_spell_check(&ev);
        }

        // Thêm phím vào bộ đệm gõ
        if self.current >= 0 {
            self.key_current += 1;
            let entry = KeyBufEntry {
                ev: {
                    let mut e = ev;
                    e.ch_type = self.input.get_char_type(e.key_code);
                    e
                },
                _converted: ret,
            };
            if (self.key_current as usize) < self.key_strokes.len() {
                self.key_strokes[self.key_current as usize] = entry;
            } else {
                self.key_strokes.push(entry);
            }
        }

        if !ret {
            // Thử khôi phục phím gốc khi từ chuyển sang NonVn
            if self.try_auto_restore() {
                let output = self.write_output();
                return ProcessResult {
                    backspaces: self.backs,
                    output,
                    out_type: OutputType::Char,
                    processed: true,
                };
            }
            return ProcessResult {
                backspaces: 0,
                output: Vec::new(),
                out_type: OutputType::Char,
                processed: false,
            };
        }

        let output = self.write_output();
        ProcessResult {
            backspaces: self.backs,
            output,
            out_type: OutputType::Char,
            processed: true,
        }
    }

    /// Xử lý phím Backspace
    pub fn process_backspace(&mut self) -> ProcessResult {
        if !self.viet_key || self.current < 0 {
            // Thử khôi phục từ soft reset (backspace sau dấu cách)
            if self.restore_saved_state() {
                return ProcessResult {
                    backspaces: 0,
                    output: Vec::new(),
                    out_type: OutputType::Char,
                    processed: false,
                };
            }
            return ProcessResult {
                backspaces: 0,
                output: Vec::new(),
                out_type: OutputType::Char,
                processed: false,
            };
        }

        self.backs = 0;
        self.change_pos = self.current + 1;
        self.mark_change(self.current);

        let form = self.buf(self.current).form;
        let prev_form = if self.current > 0 { self.buf(self.current - 1).form } else { WordForm::Empty };

        if self.current == 0
            || form == WordForm::Empty
            || form == WordForm::NonVn
            || form == WordForm::C
            || prev_form == WordForm::C
            || prev_form == WordForm::CVC
            || prev_form == WordForm::VC
        {
            self.current -= 1;
            self.synch_key_stroke_buffer();
            return ProcessResult {
                backspaces: self.backs,
                output: Vec::new(),
                out_type: OutputType::Char,
                processed: self.backs > 1,
            };
        }

        let v_offset = self.buf(self.current).v_offset;
        let v_end = self.current - v_offset;
        let vs = self.buf(v_end).vseq;
        let v_start = v_end - vseq_info(vs).len as i32 + 1;
        let new_vs = self.buf(self.current - 1).vseq;
        let cur_tone_pos = v_start + self.get_tone_position(vs, v_end == self.current);
        let new_tone_pos = v_start + self.get_tone_position(new_vs, true);
        let tone = self.buf(cur_tone_pos).tone;

        if tone == 0 || cur_tone_pos == new_tone_pos
            || (cur_tone_pos == self.current && self.buf(self.current).tone != 0)
        {
            self.current -= 1;
            self.synch_key_stroke_buffer();
            return ProcessResult {
                backspaces: self.backs,
                output: Vec::new(),
                out_type: OutputType::Char,
                processed: self.backs > 1,
            };
        }

        self.mark_change(new_tone_pos);
        self.buffer[new_tone_pos as usize].tone = tone;
        self.mark_change(cur_tone_pos);
        self.buffer[cur_tone_pos as usize].tone = 0;
        self.current -= 1;
        self.synch_key_stroke_buffer();

        let output = self.write_output();
        ProcessResult {
            backspaces: self.backs,
            output,
            out_type: OutputType::Char,
            processed: true,
        }
    }

    // ============= Trợ giúp nội bộ =============

    fn buf(&self, idx: i32) -> &WordInfo {
        debug_assert!(idx >= 0 && (idx as usize) < self.buffer.len(),
            "buf index out of range: {idx}");
        &self.buffer[idx as usize]
    }

    fn dispatch_event(&mut self, ev: KeyEvent) -> bool {
        match ev.ev_type {
            KeyEvType::RoofAll | KeyEvType::RoofA | KeyEvType::RoofE | KeyEvType::RoofO => {
                self.process_roof(ev)
            }
            KeyEvType::HookAll | KeyEvType::HookUO | KeyEvType::HookU | KeyEvType::HookO | KeyEvType::Bowl => {
                self.process_hook(ev)
            }
            KeyEvType::Dd => self.process_dd(ev),
            KeyEvType::Tone0 | KeyEvType::Tone1 | KeyEvType::Tone2
            | KeyEvType::Tone3 | KeyEvType::Tone4 | KeyEvType::Tone5 => {
                self.process_tone(ev)
            }
            KeyEvType::TelexW => self.process_telex_w(ev),
            KeyEvType::MapChar => self.process_map_char(ev),
            KeyEvType::EscChar => self.process_esc_char(ev),
            KeyEvType::Normal => self.process_append(ev),
        }
    }

    fn get_tone_position(&self, vs: VowelSeq, terminated: bool) -> i32 {
        let info = vseq_info(vs);
        if info.len == 1 {
            return 0;
        }
        if info.roof_pos != -1 {
            return info.roof_pos;
        }
        if info.hook_pos != -1 {
            if vs == VS_UHOH || vs == VS_UHOHI || vs == VS_UHOHU {
                return 1;
            }
            return info.hook_pos;
        }
        if info.len == 3 {
            return 1;
        }
        if self.options.modern_style && (vs == VS_OA || vs == VS_OE || vs == VS_UY) {
            return 1;
        }
        if terminated { 0 } else { 1 }
    }

    fn mark_change(&mut self, pos: i32) {
        if pos < self.change_pos {
            self.backs += self.get_seq_steps(pos, self.change_pos - 1);
            self.change_pos = pos;
        }
    }

    fn get_seq_steps(&self, first: i32, last: i32) -> usize {
        if last < first { return 0; }
        // Với UTF-8, mỗi ký tự = 1 bước cho backspace
        // Các bảng mã đa byte cần tính khác
        (last - first + 1) as usize
    }

    fn prepare_buffer(&mut self) {
        if self.current >= 0 && (self.current as usize) + 10 >= self.buffer.len() {
            // Tìm ranh giới từ để xóa mục cũ
            let mut rid = (self.current / 2) as usize;
            while rid < self.current as usize && self.buffer[rid].form != WordForm::Empty {
                rid += 1;
            }
            if rid == self.current as usize {
                self.current = -1;
            } else {
                rid += 1;
                self.buffer.drain(..rid);
                self.current -= rid as i32;
                // Đệm lại cho đủ MAX_BUFFER
                while self.buffer.len() < MAX_BUFFER {
                    self.buffer.push(WordInfo::default());
                }
            }
        }
    }

    fn synch_key_stroke_buffer(&mut self) {
        if self.key_current >= 0 {
            self.key_current -= 1;
        }
        if self.current >= 0 && self.buf(self.current).form == WordForm::Empty {
            while self.key_current >= 0
                && self.key_strokes[self.key_current as usize].ev.ch_type != CharType::WordBreak
            {
                self.key_current -= 1;
            }
        }
    }

    /// Khôi phục phím gốc khi từ không phải tiếng Việt (auto_non_vn_restore)
    fn try_auto_restore(&mut self) -> bool {
        if !self.options.auto_non_vn_restore
            || !self.viet_key
            || !self.options.spell_check_enabled
        {
            return false;
        }
        if self.current < 0 || self.buf(self.current).form != WordForm::NonVn {
            return false;
        }

        // Tìm vị trí bắt đầu từ trong buffer
        let mut word_start = self.current;
        while word_start > 0 && self.buf(word_start - 1).form != WordForm::Empty {
            word_start -= 1;
        }

        // Kiểm tra xem có ký tự nào đã bị biến đổi tiếng Việt không
        let mut has_modification = false;
        for i in word_start..self.current {
            let entry = &self.buffer[i as usize];
            if entry.tone != 0 {
                has_modification = true;
                break;
            }
            match entry.vn_sym {
                VnLexiName::ar | VnLexiName::er | VnLexiName::or
                | VnLexiName::oh | VnLexiName::uh | VnLexiName::ab
                | VnLexiName::dd => {
                    has_modification = true;
                    break;
                }
                _ => {}
            }
        }

        if !has_modification {
            return false;
        }

        // Tìm vị trí bắt đầu keystroke cho từ này
        let mut ks_start = self.key_current;
        while ks_start > 0
            && self.key_strokes[(ks_start - 1) as usize].ev.ch_type != CharType::WordBreak
        {
            ks_start -= 1;
        }
        let ks_count = (self.key_current - ks_start + 1) as usize;

        // Số ký tự trên màn hình cần xóa (không bao gồm ký tự mới vừa thêm, bị suppress)
        let on_screen = (self.current - word_start) as usize;

        // Ghi lại buffer với phím gốc ASCII
        self.current = word_start - 1;
        for i in 0..ks_count {
            self.current += 1;
            let idx = self.current as usize;
            let kc = self.key_strokes[(ks_start as usize) + i].ev.key_code;
            self.buffer[idx] = WordInfo::default();
            self.buffer[idx].form = WordForm::NonVn;
            self.buffer[idx].key_code = kc;
            self.buffer[idx].vn_sym = VnLexiName::NonVnChar;
        }

        self.backs = on_screen;
        self.change_pos = word_start;
        true
    }

    fn write_output(&self) -> Vec<u8> {
        let mut result = Vec::new();
        for i in self.change_pos..=self.current {
            let entry = &self.buffer[i as usize];
            if entry.vn_sym != VnLexiName::NonVnChar {
                // Chuyển vnSym + dấu + hoa/thường thành ký tự Unicode
                let ch = vn_sym_to_char(entry.vn_sym, entry.tone, entry.caps);
                let mut buf = [0u8; 4];
                let s = ch.encode_utf8(&mut buf);
                result.extend_from_slice(s.as_bytes());
            } else {
                // Truyền thẳng ký tự ASCII
                if entry.key_code < 128 {
                    result.push(entry.key_code as u8);
                } else {
                    let mut buf = [0u8; 4];
                    let ch = char::from_u32(entry.key_code).unwrap_or('?');
                    let s = ch.encode_utf8(&mut buf);
                    result.extend_from_slice(s.as_bytes());
                }
            }
        }
        result
    }

    // ============= Phương thức xử lý chính =============

    fn process_append(&mut self, ev: KeyEvent) -> bool {
        match ev.ch_type {
            CharType::Reset => {
                self.soft_reset();
                false
            }
            CharType::WordBreak => {
                self.single_mode = false;
                self.process_word_end(ev)
            }
            CharType::NonVn => {
                self.current += 1;
                let idx = self.current as usize;
                self.buffer[idx].form = WordForm::NonVn;
                self.buffer[idx].c1_offset = -1;
                self.buffer[idx].c2_offset = -1;
                self.buffer[idx].v_offset = -1;
                self.buffer[idx].key_code = ev.key_code;
                self.buffer[idx].vn_sym = ev.vn_sym.to_lower();
                self.buffer[idx].tone = 0;
                self.buffer[idx].caps = self.buffer[idx].vn_sym != ev.vn_sym;
                false
            }
            CharType::Vn => {
                if ev.vn_sym.is_vowel() {
                    let vowel = std_vn_no_tone(ev.vn_sym.to_lower());
                    // Kiểm tra u sau q hoặc i sau g có là phụ âm không
                    if self.current >= 0 && self.buf(self.current).form == WordForm::C {
                        let cseq = self.buf(self.current).cseq;
                        if (cseq == ConSeq::CS_Q && vowel == VnLexiName::u)
                            || (cseq == ConSeq::CS_G && vowel == VnLexiName::i)
                        {
                            return self.append_consonant(ev);
                        }
                    }
                    self.append_vowel(ev)
                } else {
                    self.append_consonant(ev)
                }
            }
        }
    }

    fn process_word_end(&mut self, ev: KeyEvent) -> bool {
        self.current += 1;
        let idx = self.current as usize;
        self.buffer[idx] = WordInfo::default();
        self.buffer[idx].form = WordForm::Empty;
        self.buffer[idx].key_code = ev.key_code;
        false
    }

    fn append_vowel(&mut self, ev: KeyEvent) -> bool {
        self.current += 1;
        let idx = self.current as usize;

        let lower_sym = ev.vn_sym.to_lower();
        let can_sym = std_vn_no_tone(lower_sym);
        let new_tone = get_tone(lower_sym);

        self.buffer[idx].vn_sym = can_sym;
        self.buffer[idx].caps = lower_sym != ev.vn_sym;
        self.buffer[idx].tone = new_tone;
        self.buffer[idx].key_code = ev.key_code;

        if self.current == 0 || !self.viet_key {
            self.buffer[idx].form = WordForm::V;
            self.buffer[idx].c1_offset = -1;
            self.buffer[idx].c2_offset = -1;
            self.buffer[idx].v_offset = 0;
            self.buffer[idx].vseq = lookup_vseq1(can_sym);
            if !self.viet_key {
                return false;
            }
            self.mark_change(self.current);
            return true;
        }

        let prev_idx = (self.current - 1) as usize;
        let prev_form = self.buffer[prev_idx].form;

        match prev_form {
            WordForm::Empty => {
                self.buffer[idx].form = WordForm::V;
                self.buffer[idx].c1_offset = -1;
                self.buffer[idx].c2_offset = -1;
                self.buffer[idx].v_offset = 0;
                self.buffer[idx].vseq = lookup_vseq1(can_sym);
            }
            WordForm::NonVn | WordForm::CVC | WordForm::VC => {
                self.buffer[idx].form = WordForm::NonVn;
                self.buffer[idx].c1_offset = -1;
                self.buffer[idx].c2_offset = -1;
                self.buffer[idx].v_offset = -1;
            }
            WordForm::V | WordForm::CV => {
                let prev_vs = self.buffer[prev_idx].vseq;
                let prev_info = vseq_info(prev_vs);
                let prev_tone_pos = (self.current - 1) - (prev_info.len as i32 - 1)
                    + self.get_tone_position(prev_vs, true);
                let tone = self.buffer[prev_tone_pos as usize].tone;

                let new_vs = if lower_sym != can_sym && tone != 0 {
                    VowelSeq::Nil
                } else if prev_info.len == 3 {
                    VowelSeq::Nil
                } else if prev_info.len == 2 {
                    lookup_vseq(prev_info.v[0], prev_info.v[1], can_sym)
                } else {
                    lookup_vseq2(prev_info.v[0], can_sym)
                };

                // Kiểm tra CV hợp lệ
                let new_vs = if new_vs != VowelSeq::Nil && prev_form == WordForm::CV {
                    let c1_off = self.buffer[prev_idx].c1_offset;
                    let cs = self.buffer[(self.current - 1 - c1_off) as usize].cseq;
                    if !is_valid_cv(cs, new_vs) { VowelSeq::Nil } else { new_vs }
                } else {
                    new_vs
                };

                if new_vs == VowelSeq::Nil {
                    self.buffer[idx].form = WordForm::NonVn;
                    self.buffer[idx].c1_offset = -1;
                    self.buffer[idx].c2_offset = -1;
                    self.buffer[idx].v_offset = -1;
                } else {
                    self.buffer[idx].form = prev_form;
                    self.buffer[idx].c1_offset = if prev_form == WordForm::CV {
                        self.buffer[prev_idx].c1_offset + 1
                    } else {
                        -1
                    };
                    self.buffer[idx].c2_offset = -1;
                    self.buffer[idx].v_offset = 0;
                    self.buffer[idx].vseq = new_vs;
                    self.buffer[idx].tone = 0;

                    // Xử lý đổi vị trí dấu thanh
                    if tone == 0 {
                        if new_tone != 0 {
                            let tone_pos = self.get_tone_position(new_vs, true)
                                + (self.current - 1) - prev_info.len as i32 + 1;
                            self.mark_change(tone_pos);
                            self.buffer[tone_pos as usize].tone = new_tone;
                            return true;
                        }
                    } else {
                        let new_tone_pos = self.get_tone_position(new_vs, true)
                            + (self.current - 1) - prev_info.len as i32 + 1;
                        if new_tone_pos != prev_tone_pos {
                            self.mark_change(prev_tone_pos);
                            self.buffer[prev_tone_pos as usize].tone = 0;
                            self.mark_change(new_tone_pos);
                            let t = if new_tone != 0 { new_tone } else { tone };
                            self.buffer[new_tone_pos as usize].tone = t;
                            return true;
                        }
                        if new_tone != 0 && new_tone != tone {
                            self.mark_change(prev_tone_pos);
                            self.buffer[prev_tone_pos as usize].tone = new_tone;
                            return true;
                        }
                    }
                }
            }
            WordForm::C => {
                let new_vs = lookup_vseq1(can_sym);
                let cs = self.buffer[prev_idx].cseq;
                if !is_valid_cv(cs, new_vs) {
                    self.buffer[idx].form = WordForm::NonVn;
                    self.buffer[idx].c1_offset = -1;
                    self.buffer[idx].c2_offset = -1;
                    self.buffer[idx].v_offset = -1;
                } else {
                    self.buffer[idx].form = WordForm::CV;
                    self.buffer[idx].c1_offset = 1;
                    self.buffer[idx].c2_offset = -1;
                    self.buffer[idx].v_offset = 0;
                    self.buffer[idx].vseq = new_vs;

                    // Chuyển dấu từ tiền tố gi
                    if cs == ConSeq::CS_GI && self.buffer[prev_idx].tone != 0 {
                        if self.buffer[idx].tone == 0 {
                            self.buffer[idx].tone = self.buffer[prev_idx].tone;
                        }
                        self.mark_change(self.current - 1);
                        self.buffer[prev_idx].tone = 0;
                        return true;
                    }
                }
            }
        }

        if ev.key_code < 128 && (ev.key_code as u8).is_ascii_alphabetic() {
            return false;
        }
        self.mark_change(self.current);
        true
    }

    fn append_consonant(&mut self, ev: KeyEvent) -> bool {
        self.current += 1;
        let idx = self.current as usize;
        let lower_sym = ev.vn_sym.to_lower();

        self.buffer[idx].vn_sym = lower_sym;
        self.buffer[idx].caps = lower_sym != ev.vn_sym;
        self.buffer[idx].key_code = ev.key_code;
        self.buffer[idx].tone = 0;

        if self.current == 0 || !self.viet_key {
            self.buffer[idx].form = WordForm::C;
            self.buffer[idx].c1_offset = 0;
            self.buffer[idx].c2_offset = -1;
            self.buffer[idx].v_offset = -1;
            self.buffer[idx].cseq = lookup_cseq1(lower_sym);
            return false;
        }

        let prev_idx = (self.current - 1) as usize;
        let prev_form = self.buffer[prev_idx].form;

        match prev_form {
            WordForm::NonVn => {
                self.buffer[idx].form = WordForm::NonVn;
                self.buffer[idx].c1_offset = -1;
                self.buffer[idx].c2_offset = -1;
                self.buffer[idx].v_offset = -1;
                return false;
            }
            WordForm::Empty => {
                self.buffer[idx].form = WordForm::C;
                self.buffer[idx].c1_offset = 0;
                self.buffer[idx].c2_offset = -1;
                self.buffer[idx].v_offset = -1;
                self.buffer[idx].cseq = lookup_cseq1(lower_sym);
                return false;
            }
            WordForm::V | WordForm::CV => {
                let prev_vs = self.buffer[prev_idx].vseq;
                let mut new_vs = prev_vs;

                // Xử lý tự hoàn thành ư+ơ
                if prev_vs == VS_UOH || prev_vs == VS_UHO {
                    new_vs = VS_UHOH;
                }

                let c1 = if self.buffer[prev_idx].c1_offset != -1 {
                    self.buffer[(self.current - 1 - self.buffer[prev_idx].c1_offset) as usize].cseq
                } else {
                    ConSeq::Nil
                };

                let new_cs = lookup_cseq1(lower_sym);
                let is_valid = is_valid_cvc(c1, new_vs, new_cs);

                if is_valid {
                    // Xử lý u+o -> ư+ơ tự hoàn thành
                    let mut complex = false;
                    if prev_vs == VS_UHO {
                        self.mark_change(self.current - 1);
                        self.buffer[prev_idx].vn_sym = VnLexiName::oh;
                        self.buffer[prev_idx].vseq = VS_UHOH;
                        complex = true;
                    } else if prev_vs == VS_UOH {
                        self.mark_change(self.current - 2);
                        self.buffer[(self.current - 2) as usize].vn_sym = VnLexiName::uh;
                        self.buffer[(self.current - 2) as usize].vseq = VS_UH;
                        self.buffer[prev_idx].vseq = VS_UHOH;
                        complex = true;
                    }

                    if prev_form == WordForm::V {
                        self.buffer[idx].form = WordForm::VC;
                        self.buffer[idx].c1_offset = -1;
                    } else {
                        self.buffer[idx].form = WordForm::CVC;
                        self.buffer[idx].c1_offset = self.buffer[prev_idx].c1_offset + 1;
                    }
                    self.buffer[idx].c2_offset = 0;
                    self.buffer[idx].v_offset = 1;
                    self.buffer[idx].cseq = new_cs;

                    // Đặt lại vị trí dấu nếu cần
                    let old_info = vseq_info(prev_vs);
                    let old_tp = (self.current - 1) - (old_info.len as i32 - 1)
                        + self.get_tone_position(prev_vs, true);
                    if self.buffer[old_tp as usize].tone != 0 {
                        let new_info = vseq_info(new_vs);
                        let new_tp = (self.current - 1) - (new_info.len as i32 - 1)
                            + self.get_tone_position(new_vs, false);
                        if new_tp != old_tp {
                            self.mark_change(new_tp);
                            self.buffer[new_tp as usize].tone = self.buffer[old_tp as usize].tone;
                            self.mark_change(old_tp);
                            self.buffer[old_tp as usize].tone = 0;
                            return true;
                        }
                    }

                    if complex {
                        return true;
                    }
                } else {
                    self.buffer[idx].form = WordForm::NonVn;
                    self.buffer[idx].c1_offset = -1;
                    self.buffer[idx].c2_offset = -1;
                    self.buffer[idx].v_offset = -1;
                }
                return false;
            }
            WordForm::C | WordForm::VC | WordForm::CVC => {
                let prev_cs = self.buffer[prev_idx].cseq;
                let cs_info = cseq_info(prev_cs);

                let new_cs = if cs_info.len == 3 {
                    ConSeq::Nil
                } else if cs_info.len == 2 {
                    lookup_cseq(cs_info.c[0], cs_info.c[1], lower_sym)
                } else {
                    lookup_cseq2(cs_info.c[0], lower_sym)
                };

                // Kiểm tra CVC cho dạng VC/CVC
                let new_cs = if new_cs != ConSeq::Nil && (prev_form == WordForm::VC || prev_form == WordForm::CVC) {
                    let c1 = if self.buffer[prev_idx].c1_offset != -1 {
                        self.buffer[(self.current - 1 - self.buffer[prev_idx].c1_offset) as usize].cseq
                    } else {
                        ConSeq::Nil
                    };
                    let v_idx = (self.current - 1) - self.buffer[prev_idx].v_offset;
                    let vs = self.buffer[v_idx as usize].vseq;
                    if !is_valid_cvc(c1, vs, new_cs) { ConSeq::Nil } else { new_cs }
                } else {
                    new_cs
                };

                if new_cs == ConSeq::Nil {
                    self.buffer[idx].form = WordForm::NonVn;
                    self.buffer[idx].c1_offset = -1;
                    self.buffer[idx].c2_offset = -1;
                    self.buffer[idx].v_offset = -1;
                } else {
                    match prev_form {
                        WordForm::C => {
                            self.buffer[idx].form = WordForm::C;
                            self.buffer[idx].c1_offset = 0;
                            self.buffer[idx].c2_offset = -1;
                            self.buffer[idx].v_offset = -1;
                        }
                        WordForm::VC => {
                            self.buffer[idx].form = WordForm::VC;
                            self.buffer[idx].c1_offset = -1;
                            self.buffer[idx].c2_offset = 0;
                            self.buffer[idx].v_offset = self.buffer[prev_idx].v_offset + 1;
                        }
                        WordForm::CVC => {
                            self.buffer[idx].form = WordForm::CVC;
                            self.buffer[idx].c1_offset = self.buffer[prev_idx].c1_offset + 1;
                            self.buffer[idx].c2_offset = 0;
                            self.buffer[idx].v_offset = self.buffer[prev_idx].v_offset + 1;
                        }
                        _ => {}
                    }
                    self.buffer[idx].cseq = new_cs;
                }
                return false;
            }
        }
    }

    fn process_tone(&mut self, ev: KeyEvent) -> bool {
        if self.current < 0 || !self.viet_key {
            return self.process_append(ev);
        }

        // Đặc biệt: dấu với phụ âm gi/gin
        let form = self.buf(self.current).form;
        let cseq = self.buf(self.current).cseq;
        if form == WordForm::C && (cseq == CS_GI || cseq == CS_GIN) {
            let p = if cseq == CS_GI { self.current } else { self.current - 1 };
            if self.buf(p).tone == 0 && ev.tone == 0 {
                return self.process_append(ev);
            }
            self.mark_change(p);
            if self.buf(p).tone == ev.tone {
                self.buffer[p as usize].tone = 0;
                self.single_mode = false;
                self.process_append(ev);
                self.reverted = true;
                return true;
            }
            self.buffer[p as usize].tone = ev.tone;
            return true;
        }

        if self.buf(self.current).v_offset < 0 {
            return self.process_append(ev);
        }

        let v_end = self.current - self.buf(self.current).v_offset;
        let vs = self.buf(v_end).vseq;
        let info = vseq_info(vs);

        if self.options.spell_check_enabled && !self.options.free_marking && !info.complete {
            return self.process_append(ev);
        }

        // Kiểm tra dấu tương thích với phụ âm cuối
        if form == WordForm::VC || form == WordForm::CVC {
            let cs = self.buf(self.current).cseq;
            if (cs == CS_C || cs == CS_CH || cs == CS_P || cs == CS_T)
                && (ev.tone == 2 || ev.tone == 3 || ev.tone == 4)
            {
                return self.process_append(ev);
            }
        }

        let tone_offset = self.get_tone_position(vs, v_end == self.current);
        let tone_pos = v_end - (info.len as i32 - 1) + tone_offset;

        if self.buf(tone_pos).tone == 0 && ev.tone == 0 {
            return self.process_append(ev);
        }

        if self.buf(tone_pos).tone == ev.tone {
            // Bỏ dấu thanh
            self.mark_change(tone_pos);
            self.buffer[tone_pos as usize].tone = 0;
            self.single_mode = false;
            self.process_append(ev);
            self.reverted = true;
            return true;
        }

        self.mark_change(tone_pos);
        self.buffer[tone_pos as usize].tone = ev.tone;
        true
    }

    fn process_roof(&mut self, ev: KeyEvent) -> bool {
        if !self.viet_key || self.current < 0 || self.buf(self.current).v_offset < 0 {
            return self.process_append(ev);
        }

        let target = match ev.ev_type {
            KeyEvType::RoofA => VnLexiName::ar,
            KeyEvType::RoofE => VnLexiName::er,
            KeyEvType::RoofO => VnLexiName::or,
            _ => VnLexiName::NonVnChar,
        };

        let v_end = self.current - self.buf(self.current).v_offset;
        let vs = self.buf(v_end).vseq;
        let v_start = v_end - (vseq_info(vs).len as i32 - 1);
        let cur_tone_pos = v_start + self.get_tone_position(vs, v_end == self.current);
        let tone = self.buf(cur_tone_pos).tone;

        // Đặc biệt: ư+ơ -> uô
        let mut double_change_uo = false;
        let new_vs = if vs == VS_UHO || vs == VS_UHOH || vs == VS_UHOI || vs == VS_UHOHI {
            double_change_uo = true;
            lookup_vseq(VnLexiName::u, VnLexiName::or, vseq_info(vs).v[2])
        } else {
            vseq_info(vs).with_roof
        };

        if new_vs == VowelSeq::Nil {
            if vseq_info(vs).roof_pos == -1 {
                return self.process_append(ev);
            }
            // Bỏ dấu mũ
            let roof_pos = vseq_info(vs).roof_pos;
            let cur_ch = self.buf(v_start + roof_pos).vn_sym;
            if target != VnLexiName::NonVnChar && cur_ch != target {
                return self.process_append(ev);
            }
            let new_ch = match cur_ch {
                VnLexiName::ar => VnLexiName::a,
                VnLexiName::er => VnLexiName::e,
                _ => VnLexiName::o,
            };
            let change_pos = v_start + roof_pos;
            if !self.options.free_marking && change_pos != self.current {
                return self.process_append(ev);
            }
            self.mark_change(change_pos);
            self.buffer[change_pos as usize].vn_sym = new_ch;

            // Tính lại chuỗi
            let recalc_vs = match vseq_info(vs).len {
                3 => lookup_vseq(
                    self.buf(v_start).vn_sym,
                    self.buf(v_start + 1).vn_sym,
                    self.buf(v_start + 2).vn_sym,
                ),
                2 => lookup_vseq2(self.buf(v_start).vn_sym, self.buf(v_start + 1).vn_sym),
                _ => lookup_vseq1(self.buf(v_start).vn_sym),
            };

            let p_info = vseq_info(recalc_vs);
            for j in 0..p_info.len {
                self.buffer[(v_start + j as i32) as usize].vseq = p_info.sub[j];
            }

            let new_tone_pos = v_start + self.get_tone_position(recalc_vs, v_end == self.current);
            if cur_tone_pos != new_tone_pos && tone != 0 {
                self.mark_change(new_tone_pos);
                self.buffer[new_tone_pos as usize].tone = tone;
                self.mark_change(cur_tone_pos);
                self.buffer[cur_tone_pos as usize].tone = 0;
            }

            self.single_mode = false;
            self.process_append(ev);
            self.reverted = true;
            return true;
        }

        // Thêm dấu mũ
        let p_info = vseq_info(new_vs);
        if target != VnLexiName::NonVnChar && p_info.v[p_info.roof_pos as usize] != target {
            return self.process_append(ev);
        }

        // Kiểm tra CVC hợp lệ
        let c1 = if self.buf(self.current).c1_offset != -1 {
            self.buf(self.current - self.buf(self.current).c1_offset).cseq
        } else {
            ConSeq::Nil
        };
        let c2 = if self.buf(self.current).c2_offset != -1 {
            self.buf(self.current - self.buf(self.current).c2_offset).cseq
        } else {
            ConSeq::Nil
        };
        if !is_valid_cvc(c1, new_vs, c2) {
            return self.process_append(ev);
        }

        let change_pos = if double_change_uo { v_start } else { v_start + p_info.roof_pos };
        if !self.options.free_marking && change_pos != self.current {
            return self.process_append(ev);
        }
        self.mark_change(change_pos);
        if double_change_uo {
            self.buffer[v_start as usize].vn_sym = VnLexiName::u;
            self.buffer[(v_start + 1) as usize].vn_sym = VnLexiName::or;
        } else {
            self.buffer[change_pos as usize].vn_sym = p_info.v[p_info.roof_pos as usize];
        }

        for j in 0..p_info.len {
            self.buffer[(v_start + j as i32) as usize].vseq = p_info.sub[j];
        }

        let new_tone_pos = v_start + self.get_tone_position(new_vs, v_end == self.current);
        if cur_tone_pos != new_tone_pos && tone != 0 {
            self.mark_change(new_tone_pos);
            self.buffer[new_tone_pos as usize].tone = tone;
            self.mark_change(cur_tone_pos);
            self.buffer[cur_tone_pos as usize].tone = 0;
        }

        true
    }

    fn process_hook(&mut self, ev: KeyEvent) -> bool {
        if !self.viet_key || self.current < 0 || self.buf(self.current).v_offset < 0 {
            return self.process_append(ev);
        }

        let v_end = self.current - self.buf(self.current).v_offset;
        let vs = self.buf(v_end).vseq;
        let v = vseq_info(vs).v;

        // Xử lý UO đặc biệt
        if vseq_info(vs).len > 1
            && ev.ev_type != KeyEvType::Bowl
            && (v[0] == VnLexiName::u || v[0] == VnLexiName::uh)
            && (v[1] == VnLexiName::o || v[1] == VnLexiName::oh || v[1] == VnLexiName::or)
        {
            return self.process_hook_with_uo(ev);
        }

        let v_start = v_end - (vseq_info(vs).len as i32 - 1);
        let cur_tone_pos = v_start + self.get_tone_position(vs, v_end == self.current);
        let tone = self.buf(cur_tone_pos).tone;

        let new_vs = vseq_info(vs).with_hook;
        if new_vs == VowelSeq::Nil {
            if vseq_info(vs).hook_pos == -1 {
                return self.process_append(ev);
            }
            // Bỏ dấu móc
            let hook_pos = vseq_info(vs).hook_pos;
            let cur_ch = self.buf(v_start + hook_pos).vn_sym;
            let new_ch = match cur_ch {
                VnLexiName::ab => VnLexiName::a,
                VnLexiName::uh => VnLexiName::u,
                _ => VnLexiName::o,
            };
            let change_pos = v_start + hook_pos;
            if !self.options.free_marking && change_pos != self.current {
                return self.process_append(ev);
            }

            // Kiểm tra loại sự kiện trùng khớp
            match ev.ev_type {
                KeyEvType::HookU => { if cur_ch != VnLexiName::uh { return self.process_append(ev); } }
                KeyEvType::HookO => { if cur_ch != VnLexiName::oh { return self.process_append(ev); } }
                KeyEvType::Bowl => { if cur_ch != VnLexiName::ab { return self.process_append(ev); } }
                _ => {
                    if ev.ev_type == KeyEvType::HookUO && cur_ch == VnLexiName::ab {
                        return self.process_append(ev);
                    }
                }
            }

            self.mark_change(change_pos);
            self.buffer[change_pos as usize].vn_sym = new_ch;

            let recalc_vs = match vseq_info(vs).len {
                3 => lookup_vseq(
                    self.buf(v_start).vn_sym,
                    self.buf(v_start + 1).vn_sym,
                    self.buf(v_start + 2).vn_sym,
                ),
                2 => lookup_vseq2(self.buf(v_start).vn_sym, self.buf(v_start + 1).vn_sym),
                _ => lookup_vseq1(self.buf(v_start).vn_sym),
            };

            let p_info = vseq_info(recalc_vs);
            for j in 0..p_info.len {
                self.buffer[(v_start + j as i32) as usize].vseq = p_info.sub[j];
            }

            let new_tone_pos = v_start + self.get_tone_position(recalc_vs, v_end == self.current);
            if cur_tone_pos != new_tone_pos && tone != 0 {
                self.mark_change(new_tone_pos);
                self.buffer[new_tone_pos as usize].tone = tone;
                self.mark_change(cur_tone_pos);
                self.buffer[cur_tone_pos as usize].tone = 0;
            }

            self.single_mode = false;
            self.process_append(ev);
            self.reverted = true;
            return true;
        }

        // Thêm dấu móc
        let p_info = vseq_info(new_vs);
        match ev.ev_type {
            KeyEvType::HookU => {
                if p_info.v[p_info.hook_pos as usize] != VnLexiName::uh {
                    return self.process_append(ev);
                }
            }
            KeyEvType::HookO => {
                if p_info.v[p_info.hook_pos as usize] != VnLexiName::oh {
                    return self.process_append(ev);
                }
            }
            KeyEvType::Bowl => {
                if p_info.v[p_info.hook_pos as usize] != VnLexiName::ab {
                    return self.process_append(ev);
                }
            }
            _ => {
                if ev.ev_type == KeyEvType::HookUO && p_info.v[p_info.hook_pos as usize] == VnLexiName::ab {
                    return self.process_append(ev);
                }
            }
        }

        // Kiểm tra CVC hợp lệ
        let c1 = if self.buf(self.current).c1_offset != -1 {
            self.buf(self.current - self.buf(self.current).c1_offset).cseq
        } else {
            ConSeq::Nil
        };
        let c2 = if self.buf(self.current).c2_offset != -1 {
            self.buf(self.current - self.buf(self.current).c2_offset).cseq
        } else {
            ConSeq::Nil
        };
        if !is_valid_cvc(c1, new_vs, c2) {
            return self.process_append(ev);
        }

        let change_pos = v_start + p_info.hook_pos;
        if !self.options.free_marking && change_pos != self.current {
            return self.process_append(ev);
        }
        self.mark_change(change_pos);
        self.buffer[change_pos as usize].vn_sym = p_info.v[p_info.hook_pos as usize];

        for j in 0..p_info.len {
            self.buffer[(v_start + j as i32) as usize].vseq = p_info.sub[j];
        }

        let new_tone_pos = v_start + self.get_tone_position(new_vs, v_end == self.current);
        if cur_tone_pos != new_tone_pos && tone != 0 {
            self.mark_change(new_tone_pos);
            self.buffer[new_tone_pos as usize].tone = tone;
            self.mark_change(cur_tone_pos);
            self.buffer[cur_tone_pos as usize].tone = 0;
        }

        true
    }

    fn process_hook_with_uo(&mut self, ev: KeyEvent) -> bool {
        if !self.options.free_marking && self.buf(self.current).v_offset != 0 {
            return self.process_append(ev);
        }

        let v_end = self.current - self.buf(self.current).v_offset;
        let vs = self.buf(v_end).vseq;
        let v_start = v_end - (vseq_info(vs).len as i32 - 1);
        let v = vseq_info(vs).v;
        let cur_tone_pos = v_start + self.get_tone_position(vs, v_end == self.current);
        let tone = self.buf(cur_tone_pos).tone;

        let mut hook_removed = false;
        let new_vs;

        match ev.ev_type {
            KeyEvType::HookU => {
                if v[0] == VnLexiName::u {
                    new_vs = vseq_info(vs).with_hook;
                    self.mark_change(v_start);
                    self.buffer[v_start as usize].vn_sym = VnLexiName::uh;
                } else {
                    new_vs = lookup_vseq(VnLexiName::u, VnLexiName::o, v[2]);
                    self.mark_change(v_start);
                    self.buffer[v_start as usize].vn_sym = VnLexiName::u;
                    self.buffer[(v_start + 1) as usize].vn_sym = VnLexiName::o;
                    hook_removed = true;
                }
            }
            KeyEvType::HookO => {
                if v[1] == VnLexiName::o || v[1] == VnLexiName::or {
                    // Đặc biệt: th + o -> ơ
                    if v_end == self.current && vseq_info(vs).len == 2
                        && self.buf(self.current).form == WordForm::CV
                        && self.buf(self.current - 2).cseq == CS_TH
                    {
                        new_vs = vseq_info(vs).with_hook;
                        self.mark_change(v_start + 1);
                        self.buffer[(v_start + 1) as usize].vn_sym = VnLexiName::oh;
                    } else {
                        new_vs = lookup_vseq(VnLexiName::uh, VnLexiName::oh, v[2]);
                        if v[0] == VnLexiName::u {
                            self.mark_change(v_start);
                            self.buffer[v_start as usize].vn_sym = VnLexiName::uh;
                            self.buffer[(v_start + 1) as usize].vn_sym = VnLexiName::oh;
                        } else {
                            self.mark_change(v_start + 1);
                            self.buffer[(v_start + 1) as usize].vn_sym = VnLexiName::oh;
                        }
                    }
                } else {
                    new_vs = lookup_vseq(VnLexiName::u, VnLexiName::o, v[2]);
                    if v[0] == VnLexiName::uh {
                        self.mark_change(v_start);
                        self.buffer[v_start as usize].vn_sym = VnLexiName::u;
                        self.buffer[(v_start + 1) as usize].vn_sym = VnLexiName::o;
                    } else {
                        self.mark_change(v_start + 1);
                        self.buffer[(v_start + 1) as usize].vn_sym = VnLexiName::o;
                    }
                    hook_removed = true;
                }
            }
            _ => {
                // HookAll / HookUO
                if v[0] == VnLexiName::u {
                    if v[1] == VnLexiName::o || v[1] == VnLexiName::or {
                        if (vs == VS_UO || vs == VS_UOR) && v_end == self.current
                            && self.buf(self.current).form == WordForm::CV
                            && self.buf(self.current - 2).cseq == CS_TH
                        {
                            new_vs = VS_UOH;
                            self.mark_change(v_start + 1);
                            self.buffer[(v_start + 1) as usize].vn_sym = VnLexiName::oh;
                        } else {
                            let tmp = vseq_info(vs).with_hook;
                            new_vs = vseq_info(tmp).with_hook;
                            self.mark_change(v_start);
                            self.buffer[v_start as usize].vn_sym = VnLexiName::uh;
                            self.buffer[(v_start + 1) as usize].vn_sym = VnLexiName::oh;
                        }
                    } else {
                        new_vs = vseq_info(vs).with_hook;
                        self.mark_change(v_start);
                        self.buffer[v_start as usize].vn_sym = VnLexiName::uh;
                    }
                } else {
                    // v[0] == uh
                    if v[1] == VnLexiName::o {
                        new_vs = vseq_info(vs).with_hook;
                        self.mark_change(v_start + 1);
                        self.buffer[(v_start + 1) as usize].vn_sym = VnLexiName::oh;
                    } else {
                        // ư+ơ -> uo (bỏ dấu)
                        new_vs = lookup_vseq(VnLexiName::u, VnLexiName::o, v[2]);
                        self.mark_change(v_start);
                        self.buffer[v_start as usize].vn_sym = VnLexiName::u;
                        self.buffer[(v_start + 1) as usize].vn_sym = VnLexiName::o;
                        hook_removed = true;
                    }
                }
            }
        }

        let p_info = vseq_info(new_vs);
        for j in 0..p_info.len {
            self.buffer[(v_start + j as i32) as usize].vseq = p_info.sub[j];
        }

        let new_tone_pos = v_start + self.get_tone_position(new_vs, v_end == self.current);
        if cur_tone_pos != new_tone_pos && tone != 0 {
            self.mark_change(new_tone_pos);
            self.buffer[new_tone_pos as usize].tone = tone;
            self.mark_change(cur_tone_pos);
            self.buffer[cur_tone_pos as usize].tone = 0;
        }

        if hook_removed {
            self.single_mode = false;
            self.process_append(ev);
            self.reverted = true;
        }

        true
    }

    fn process_dd(&mut self, ev: KeyEvent) -> bool {
        if !self.viet_key || self.current < 0 {
            return self.process_append(ev);
        }

        // Cho phép dd trong chuỗi không phải tiếng Việt (viết tắt)
        if self.buf(self.current).form == WordForm::NonVn
            && self.buf(self.current).vn_sym == VnLexiName::d
            && (self.current == 0
                || self.buf(self.current - 1).vn_sym == VnLexiName::NonVnChar
                || !self.buf(self.current - 1).vn_sym.is_vowel())
        {
            self.single_mode = true;
            let pos = self.current;
            self.mark_change(pos);
            self.buffer[pos as usize].cseq = CS_DD;
            self.buffer[pos as usize].vn_sym = VnLexiName::dd;
            self.buffer[pos as usize].form = WordForm::C;
            self.buffer[pos as usize].c1_offset = 0;
            self.buffer[pos as usize].c2_offset = -1;
            self.buffer[pos as usize].v_offset = -1;
            return true;
        }

        if self.buf(self.current).c1_offset < 0 {
            return self.process_append(ev);
        }

        let pos = self.current - self.buf(self.current).c1_offset;
        if !self.options.free_marking && pos != self.current {
            return self.process_append(ev);
        }

        if self.buf(pos).cseq == CS_D {
            self.mark_change(pos);
            self.buffer[pos as usize].cseq = CS_DD;
            self.buffer[pos as usize].vn_sym = VnLexiName::dd;
            return true;
        }

        if self.buf(pos).cseq == CS_DD {
            // Bỏ đđ
            self.mark_change(pos);
            self.buffer[pos as usize].cseq = CS_D;
            self.buffer[pos as usize].vn_sym = VnLexiName::d;
            self.single_mode = false;
            self.process_append(ev);
            self.reverted = true;
            return true;
        }

        self.process_append(ev)
    }

    fn process_map_char(&mut self, ev: KeyEvent) -> bool {
        let mut mev = ev.clone();
        // Áp dụng capslock
        // (Đơn giản hóa — xử lý capslock do tầng nền tảng thực hiện)
        let ret = self.process_append(mev.clone());
        if !self.viet_key {
            return ret;
        }

        if self.current >= 0
            && self.buf(self.current).form != WordForm::Empty
            && self.buf(self.current).form != WordForm::NonVn
        {
            return true;
        }

        if self.current < 0 {
            return false;
        }

        // mapChar không áp dụng được — hoàn tác và thử như bình thường
        self.current -= 1;
        let mut undo = false;

        if self.current >= 0 {
            let entry_form = self.buf(self.current).form;
            if entry_form != WordForm::Empty && entry_form != WordForm::NonVn {
                let prev_sym = {
                    let e = &self.buffer[self.current as usize];
                    if e.caps { VnLexiName::change_case(e.vn_sym) } else { e.vn_sym }
                };
                // GHI CHÚ: logic hoàn tác đơn giản hóa
                if prev_sym == ev.vn_sym {
                    self.mark_change(self.current);
                    self.current -= 1;
                    undo = true;
                }
            }
        }

        mev.ev_type = KeyEvType::Normal;
        mev.ch_type = self.input.get_char_type(ev.key_code);
        mev.vn_sym = iso_to_vn_lexi(ev.key_code);
        let ret = self.process_append(mev);
        if undo {
            self.single_mode = false;
            self.reverted = true;
            return true;
        }
        ret
    }

    fn process_telex_w(&mut self, ev: KeyEvent) -> bool {
        if !self.viet_key {
            return self.process_append(ev);
        }

        // Kiểm tra ww → hoàn tác ư thành ww TRƯỚC khi thử hook.
        // Chỉ áp dụng cho ư tạo bởi phím 'w' đứng đơn (không phải hook u→ư trong từ).
        // Nếu để process_hook chạy trước, nó sẽ "bỏ móc" uh→u rồi append w → ra "uw" thay vì "ww".
        if self.current >= 0
            && self.buf(self.current).vn_sym == VnLexiName::uh
            && self.buf(self.current).form == WordForm::V
            && (self.buf(self.current).key_code == b'w' as u32
                || self.buf(self.current).key_code == b'W' as u32)
        {
            // ww → hoàn tác ư thành w
            self.mark_change(self.current);
            self.buffer[self.current as usize].form = WordForm::NonVn;
            self.buffer[self.current as usize].vn_sym = VnLexiName::NonVnChar;
            self.buffer[self.current as usize].key_code = b'w' as u32;
            self.single_mode = false;
            self.process_append(ev);
            self.reverted = true;
            return true;
        }

        // Thử móc khi có nguyên âm để sửa (u→ư, o→ơ, a→ă)
        if self.current >= 0 && self.buf(self.current).v_offset >= 0 {
            let mut hook_ev = ev.clone();
            hook_ev.ev_type = KeyEvType::HookAll;
            let ret = self.process_hook(hook_ev);
            if ret {
                return ret;
            }
        }

        // Không có nguyên âm để móc — tạo 'ư' (Telex chuẩn)
        // w → ư
        self.current += 1;
        let idx = self.current as usize;
        self.buffer[idx].form = WordForm::V;
        self.buffer[idx].vn_sym = VnLexiName::uh;
        self.buffer[idx].caps = ev.key_code < 0x61; // uppercase check
        self.buffer[idx].tone = 0;
        self.buffer[idx].key_code = ev.key_code;
        self.buffer[idx].v_offset = 0;
        self.buffer[idx].c1_offset = -1;
        self.buffer[idx].c2_offset = -1;
        self.buffer[idx].vseq = lookup_vseq1(VnLexiName::uh);
        self.mark_change(self.current);
        true
    }

    fn process_esc_char(&mut self, ev: KeyEvent) -> bool {
        if self.viet_key
            && self.current >= 0
            && self.buf(self.current).form != WordForm::Empty
            && self.buf(self.current).form != WordForm::NonVn
        {
            self.to_escape = true;
        }
        self.process_append(ev)
    }

    fn process_no_spell_check(&mut self, _ev: &KeyEvent) {
        let idx = self.current as usize;
        if self.buffer[idx].vn_sym.is_vowel() {
            self.buffer[idx].form = WordForm::V;
            self.buffer[idx].v_offset = 0;
            self.buffer[idx].vseq = lookup_vseq1(self.buffer[idx].vn_sym);
            self.buffer[idx].c1_offset = -1;
            self.buffer[idx].c2_offset = -1;
        } else {
            self.buffer[idx].form = WordForm::C;
            self.buffer[idx].c1_offset = 0;
            self.buffer[idx].c2_offset = -1;
            self.buffer[idx].v_offset = -1;
            self.buffer[idx].cseq = lookup_cseq1(self.buffer[idx].vn_sym);
        }
    }

    pub fn set_single_mode(&mut self) {
        self.single_mode = true;
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

// ============= Ánh xạ ký tự sang Unicode =============

/// Chuyển VnLexiName + dấu + hoa/thường thành ký tự Unicode
fn vn_sym_to_char(sym: VnLexiName, tone: i32, caps: bool) -> char {
    use VnLexiName::*;

    // Lấy ký tự cơ sở với dấu phụ
    let base = std_vn_no_tone(if caps { sym.to_upper() } else { sym });

    // Ánh xạ VnLexiName cơ sở sang ký tự Unicode cơ sở
    let base_char = match base {
        A | a => if caps { 'A' } else { 'a' },
        Ar | ar => if caps { '\u{00C2}' } else { '\u{00E2}' }, // Â â
        Ab | ab => if caps { '\u{0102}' } else { '\u{0103}' }, // Ă ă
        E | e => if caps { 'E' } else { 'e' },
        Er | er => if caps { '\u{00CA}' } else { '\u{00EA}' }, // Ê ê
        I | VnLexiName::i => if caps { 'I' } else { 'i' },
        O | o => if caps { 'O' } else { 'o' },
        Or | or => if caps { '\u{00D4}' } else { '\u{00F4}' }, // Ô ô
        Oh | oh => if caps { '\u{01A0}' } else { '\u{01A1}' }, // Ơ ơ
        U | u => if caps { 'U' } else { 'u' },
        Uh | uh => if caps { '\u{01AF}' } else { '\u{01B0}' }, // Ư ư
        Y | y => if caps { 'Y' } else { 'y' },
        // Phụ âm
        B | VnLexiName::b => if caps { 'B' } else { 'b' },
        C | VnLexiName::c => if caps { 'C' } else { 'c' },
        D | VnLexiName::d => if caps { 'D' } else { 'd' },
        DD | dd => if caps { '\u{0110}' } else { '\u{0111}' }, // Đ đ
        F | VnLexiName::f => if caps { 'F' } else { 'f' },
        G | VnLexiName::g => if caps { 'G' } else { 'g' },
        H | VnLexiName::h => if caps { 'H' } else { 'h' },
        J | VnLexiName::j => if caps { 'J' } else { 'j' },
        K | VnLexiName::k => if caps { 'K' } else { 'k' },
        L | VnLexiName::l => if caps { 'L' } else { 'l' },
        M | VnLexiName::m => if caps { 'M' } else { 'm' },
        N | VnLexiName::n => if caps { 'N' } else { 'n' },
        P | VnLexiName::p => if caps { 'P' } else { 'p' },
        Q | VnLexiName::q => if caps { 'Q' } else { 'q' },
        R | VnLexiName::r => if caps { 'R' } else { 'r' },
        S | VnLexiName::s => if caps { 'S' } else { 's' },
        T | VnLexiName::t => if caps { 'T' } else { 't' },
        V | VnLexiName::v => if caps { 'V' } else { 'v' },
        W | VnLexiName::w => if caps { 'W' } else { 'w' },
        X | VnLexiName::x => if caps { 'X' } else { 'x' },
        Z | VnLexiName::z => if caps { 'Z' } else { 'z' },
        _ => '?',
    };

    // Áp dụng dấu thanh
    if tone == 0 {
        return base_char;
    }

    apply_tone(base_char, tone)
}

/// Áp dụng dấu thanh (1-5) cho ký tự tiếng Việt cơ sở
fn apply_tone(base: char, tone: i32) -> char {
    // Dấu thanh tiếng Việt trong Unicode:
    // 1 = sắc, 2 = huyền, 3 = hỏi, 4 = ngã, 5 = nặng
    
    // Bảng ánh xạ đầy đủ
    match (base, tone) {
        // Họ a
        ('a', 1) => '\u{00E1}', ('a', 2) => '\u{00E0}', ('a', 3) => '\u{1EA3}', ('a', 4) => '\u{00E3}', ('a', 5) => '\u{1EA1}',
        ('A', 1) => '\u{00C1}', ('A', 2) => '\u{00C0}', ('A', 3) => '\u{1EA2}', ('A', 4) => '\u{00C3}', ('A', 5) => '\u{1EA0}',
        // Họ â
        ('\u{00E2}', 1) => '\u{1EA5}', ('\u{00E2}', 2) => '\u{1EA7}', ('\u{00E2}', 3) => '\u{1EA9}', ('\u{00E2}', 4) => '\u{1EAB}', ('\u{00E2}', 5) => '\u{1EAD}',
        ('\u{00C2}', 1) => '\u{1EA4}', ('\u{00C2}', 2) => '\u{1EA6}', ('\u{00C2}', 3) => '\u{1EA8}', ('\u{00C2}', 4) => '\u{1EAA}', ('\u{00C2}', 5) => '\u{1EAC}',
        // Họ ă
        ('\u{0103}', 1) => '\u{1EAF}', ('\u{0103}', 2) => '\u{1EB1}', ('\u{0103}', 3) => '\u{1EB3}', ('\u{0103}', 4) => '\u{1EB5}', ('\u{0103}', 5) => '\u{1EB7}',
        ('\u{0102}', 1) => '\u{1EAE}', ('\u{0102}', 2) => '\u{1EB0}', ('\u{0102}', 3) => '\u{1EB2}', ('\u{0102}', 4) => '\u{1EB4}', ('\u{0102}', 5) => '\u{1EB6}',
        // Họ e
        ('e', 1) => '\u{00E9}', ('e', 2) => '\u{00E8}', ('e', 3) => '\u{1EBB}', ('e', 4) => '\u{1EBD}', ('e', 5) => '\u{1EB9}',
        ('E', 1) => '\u{00C9}', ('E', 2) => '\u{00C8}', ('E', 3) => '\u{1EBA}', ('E', 4) => '\u{1EBC}', ('E', 5) => '\u{1EB8}',
        // Họ ê
        ('\u{00EA}', 1) => '\u{1EBF}', ('\u{00EA}', 2) => '\u{1EC1}', ('\u{00EA}', 3) => '\u{1EC3}', ('\u{00EA}', 4) => '\u{1EC5}', ('\u{00EA}', 5) => '\u{1EC7}',
        ('\u{00CA}', 1) => '\u{1EBE}', ('\u{00CA}', 2) => '\u{1EC0}', ('\u{00CA}', 3) => '\u{1EC2}', ('\u{00CA}', 4) => '\u{1EC4}', ('\u{00CA}', 5) => '\u{1EC6}',
        // Họ i
        ('i', 1) => '\u{00ED}', ('i', 2) => '\u{00EC}', ('i', 3) => '\u{1EC9}', ('i', 4) => '\u{0129}', ('i', 5) => '\u{1ECB}',
        ('I', 1) => '\u{00CD}', ('I', 2) => '\u{00CC}', ('I', 3) => '\u{1EC8}', ('I', 4) => '\u{0128}', ('I', 5) => '\u{1ECA}',
        // Họ o
        ('o', 1) => '\u{00F3}', ('o', 2) => '\u{00F2}', ('o', 3) => '\u{1ECF}', ('o', 4) => '\u{00F5}', ('o', 5) => '\u{1ECD}',
        ('O', 1) => '\u{00D3}', ('O', 2) => '\u{00D2}', ('O', 3) => '\u{1ECE}', ('O', 4) => '\u{00D5}', ('O', 5) => '\u{1ECC}',
        // Họ ô
        ('\u{00F4}', 1) => '\u{1ED1}', ('\u{00F4}', 2) => '\u{1ED3}', ('\u{00F4}', 3) => '\u{1ED5}', ('\u{00F4}', 4) => '\u{1ED7}', ('\u{00F4}', 5) => '\u{1ED9}',
        ('\u{00D4}', 1) => '\u{1ED0}', ('\u{00D4}', 2) => '\u{1ED2}', ('\u{00D4}', 3) => '\u{1ED4}', ('\u{00D4}', 4) => '\u{1ED6}', ('\u{00D4}', 5) => '\u{1ED8}',
        // Họ ơ
        ('\u{01A1}', 1) => '\u{1EDB}', ('\u{01A1}', 2) => '\u{1EDD}', ('\u{01A1}', 3) => '\u{1EDF}', ('\u{01A1}', 4) => '\u{1EE1}', ('\u{01A1}', 5) => '\u{1EE3}',
        ('\u{01A0}', 1) => '\u{1EDA}', ('\u{01A0}', 2) => '\u{1EDC}', ('\u{01A0}', 3) => '\u{1EDE}', ('\u{01A0}', 4) => '\u{1EE0}', ('\u{01A0}', 5) => '\u{1EE2}',
        // Họ u
        ('u', 1) => '\u{00FA}', ('u', 2) => '\u{00F9}', ('u', 3) => '\u{1EE7}', ('u', 4) => '\u{0169}', ('u', 5) => '\u{1EE5}',
        ('U', 1) => '\u{00DA}', ('U', 2) => '\u{00D9}', ('U', 3) => '\u{1EE6}', ('U', 4) => '\u{0168}', ('U', 5) => '\u{1EE4}',
        // Họ ư
        ('\u{01B0}', 1) => '\u{1EE9}', ('\u{01B0}', 2) => '\u{1EEB}', ('\u{01B0}', 3) => '\u{1EED}', ('\u{01B0}', 4) => '\u{1EEF}', ('\u{01B0}', 5) => '\u{1EF1}',
        ('\u{01AF}', 1) => '\u{1EE8}', ('\u{01AF}', 2) => '\u{1EEA}', ('\u{01AF}', 3) => '\u{1EEC}', ('\u{01AF}', 4) => '\u{1EEE}', ('\u{01AF}', 5) => '\u{1EF0}',
        // Họ y
        ('y', 1) => '\u{00FD}', ('y', 2) => '\u{1EF3}', ('y', 3) => '\u{1EF7}', ('y', 4) => '\u{1EF9}', ('y', 5) => '\u{1EF5}',
        ('Y', 1) => '\u{00DD}', ('Y', 2) => '\u{1EF2}', ('Y', 3) => '\u{1EF6}', ('Y', 4) => '\u{1EF8}', ('Y', 5) => '\u{1EF4}',

        _ => base,
    }
}
