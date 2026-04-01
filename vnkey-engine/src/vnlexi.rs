//! Từ điển tiếng Việt: tên ký tự, chuỗi nguyên âm, chuỗi phụ âm.

/// Tên ký tự tiếng Việt. Chẵn = hoa, lẻ = thường.
/// Dấu: base, 1(sắc), 2(huyền), 3(hỏi), 4(ngã), 5(nặng)
/// Hậu tố: r = mũ (â,ê,ô), b = trăng (ă), h = móc (ơ,ư)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i16)]
#[allow(non_camel_case_types)]
pub enum VnLexiName {
    NonVnChar = -1,

    // Họ A
    A = 0, a, A1, a1, A2, a2, A3, a3, A4, a4, A5, a5,
    // Họ Â (mũ)
    Ar, ar, Ar1, ar1, Ar2, ar2, Ar3, ar3, Ar4, ar4, Ar5, ar5,
    // Họ Ă (trăng)
    Ab, ab, Ab1, ab1, Ab2, ab2, Ab3, ab3, Ab4, ab4, Ab5, ab5,
    // Phụ âm
    B, b, C, c,
    D, d, DD, dd,
    // Họ E
    E, e, E1, e1, E2, e2, E3, e3, E4, e4, E5, e5,
    // Họ Ê (mũ)
    Er, er, Er1, er1, Er2, er2, Er3, er3, Er4, er4, Er5, er5,
    // Phụ âm
    F, f, G, g, H, h,
    // Họ I
    I, i, I1, i1, I2, i2, I3, i3, I4, i4, I5, i5,
    // Phụ âm
    J, j, K, k, L, l, M, m, N, n,
    // Họ O
    O, o, O1, o1, O2, o2, O3, o3, O4, o4, O5, o5,
    // Họ Ô (mũ)
    Or, or, Or1, or1, Or2, or2, Or3, or3, Or4, or4, Or5, or5,
    // Họ Ơ (móc)
    Oh, oh, Oh1, oh1, Oh2, oh2, Oh3, oh3, Oh4, oh4, Oh5, oh5,
    // Phụ âm
    P, p, Q, q, R, r, S, s, T, t,
    // Họ U
    U, u, U1, u1, U2, u2, U3, u3, U4, u4, U5, u5,
    // Họ Ư (móc)
    Uh, uh, Uh1, uh1, Uh2, uh2, Uh3, uh3, Uh4, uh4, Uh5, uh5,
    // Phụ âm
    V, v, W, w, X, x,
    // Họ Y
    Y, y, Y1, y1, Y2, y2, Y3, y3, Y4, y4, Y5, y5,
    // Phụ âm
    Z, z,

    LastChar,
}

use VnLexiName::*;

impl VnLexiName {
    /// An toàn: chuyển i16 sang VnLexiName, trả về NonVnChar nếu ngoài phạm vi
    pub fn from_i16(val: i16) -> Self {
        if val >= VnLexiName::NonVnChar as i16 && val < VnLexiName::LastChar as i16 {
            // SAFETY: repr(i16), variant liên tục -1..LastChar, đã kiểm tra biên
            unsafe { std::mem::transmute(val) }
        } else {
            VnLexiName::NonVnChar
        }
    }

    pub fn to_lower(self) -> Self {
        if self == NonVnChar { return self; }
        let val = self as i16;
        if val & 1 == 0 { // chẵn = chữ hoa
            VnLexiName::from_i16(val + 1)
        } else {
            self
        }
    }

    pub fn to_upper(self) -> Self {
        if self == NonVnChar { return self; }
        let val = self as i16;
        if val & 1 == 1 { // lẻ = chữ thường
            VnLexiName::from_i16(val - 1)
        } else {
            self
        }
    }

    pub fn change_case(self) -> Self {
        if self == NonVnChar { return self; }
        let val = self as i16;
        if val & 1 == 0 { // chẵn = chữ hoa → chữ thường
            VnLexiName::from_i16(val + 1)
        } else {
            VnLexiName::from_i16(val - 1)
        }
    }

    pub fn is_vowel(self) -> bool {
        if self == NonVnChar { return false; }
        IS_VN_VOWEL[self as usize + 1] // +1 vì NonVnChar = -1
    }
}

/// Bảng tra nguyên âm cho VnLexiName
/// Chỉ mục 0 = NonVnChar (-1 + 1), các mục tiếp theo
static IS_VN_VOWEL: once_cell::sync::Lazy<Vec<bool>> = once_cell::sync::Lazy::new(|| {
    let total = LastChar as usize;
    let mut table = vec![false; total + 2]; // +2 cho an toàn (chỉ mục 0 = NonVnChar)
    // Mặc định là nguyên âm
    for idx in 0..total {
        table[idx + 1] = true; // mặc định true
    }
    // Đặt phụ âm = false
    let consonants: &[VnLexiName] = &[
        B, VnLexiName::b, C, VnLexiName::c, D, VnLexiName::d, DD, dd,
        F, VnLexiName::f, G, VnLexiName::g, H, VnLexiName::h,
        J, VnLexiName::j, K, VnLexiName::k, L, VnLexiName::l,
        M, VnLexiName::m, N, VnLexiName::n,
        P, VnLexiName::p, Q, VnLexiName::q, R, VnLexiName::r,
        S, VnLexiName::s, T, VnLexiName::t,
        V, VnLexiName::v, W, VnLexiName::w, X, VnLexiName::x,
        Z, VnLexiName::z,
    ];
    for &ch in consonants.iter() {
        table[ch as usize + 1] = false;
    }
    table[0] = false; // NonVnChar
    table
});

/// Offset chuẩn StdVnChar để chuyển vnSym sang StdVnChar
pub const VN_STD_CHAR_OFFSET: u32 = 0x100; // Giá trị StdVnChar bắt đầu từ 256

/// Ánh xạ VnLexiName sang dạng chuẩn không dấu
/// Ví dụ: a1 -> a, ar3 -> ar, v.v.
pub fn std_vn_no_tone(sym: VnLexiName) -> VnLexiName {
    let val = sym as i16;
    if val < 0 { return sym; }

    let groups_with_tones: &[(VnLexiName, VnLexiName)] = &[
        (A, a), (Ar, ar), (Ab, ab),
        (E, e), (Er, er),
        (I, VnLexiName::i),
        (O, o), (Or, or), (Oh, oh),
        (U, u), (Uh, uh),
        (Y, y),
    ];

    for &(upper, lower) in groups_with_tones {
        let base = upper as i16;
        if val >= base && val < base + 12 {
            let offset = val - base;
            if offset & 1 == 0 {
                return upper;
            } else {
                return lower;
            }
        }
    }

    sym
}

/// Lấy giá trị dấu (0-5) từ VnLexiName
pub fn get_tone(sym: VnLexiName) -> i32 {
    let val = sym as i16;
    if val < 0 { return 0; }

    let groups: &[VnLexiName] = &[A, Ar, Ab, E, Er, I, O, Or, Oh, U, Uh, Y];
    for &group_start in groups {
        let base = group_start as i16;
        if val >= base && val < base + 12 {
            let offset = val - base;
            return (offset / 2) as i32;
        }
    }
    0
}

// ============= Chuỗi nguyên âm =============

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i16)]
#[allow(non_camel_case_types)]
pub enum VowelSeq {
    Nil = -1,
    // Nguyên âm đơn
    VS_A = 0, VS_AR, VS_AB,
    VS_E, VS_ER,
    VS_I,
    VS_O, VS_OR, VS_OH,
    VS_U, VS_UH,
    VS_Y,
    // Chuỗi 2 nguyên âm
    VS_AI, VS_AO, VS_AU, VS_AY,
    VS_ARU, VS_ARY,
    VS_EO, VS_EU, VS_ERU,
    VS_IA, VS_IE, VS_IER, VS_IU,
    VS_OA, VS_OAB, VS_OE, VS_OI,
    VS_ORI, VS_OHI,
    VS_UA, VS_UAR,
    VS_UE, VS_UER,
    VS_UI, VS_UO, VS_UOR, VS_UOH,
    VS_UU, VS_UY,
    VS_UHA, VS_UHI, VS_UHO, VS_UHOH, VS_UHU,
    VS_YE, VS_YER,
    // Chuỗi 3 nguyên âm
    VS_IEU, VS_IERU,
    VS_OAI, VS_OAY, VS_OEO,
    VS_UAY, VS_UARY,
    VS_UOI, VS_UOU,
    VS_UORI, VS_UOHI, VS_UOHU,
    VS_UYA, VS_UYE, VS_UYER, VS_UYU,
    VS_UHOI, VS_UHOU, VS_UHOHI, VS_UHOHU,
    VS_YEU, VS_YERU,
}

impl VowelSeq {
    pub fn from_i16(val: i16) -> Self {
        if val >= VowelSeq::Nil as i16 && val <= VowelSeq::VS_YERU as i16 {
            unsafe { std::mem::transmute(val) }
        } else {
            VowelSeq::Nil
        }
    }
}

use VowelSeq::*;

// ============= Chuỗi phụ âm =============

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i16)]
#[allow(non_camel_case_types)]
pub enum ConSeq {
    Nil = -1,
    CS_B = 0, CS_C, CS_CH,
    CS_D, CS_DD, CS_DZ,
    CS_G, CS_GH, CS_GI, CS_GIN,
    CS_K, CS_KH,
    CS_L, CS_M, CS_N, CS_NG, CS_NGH, CS_NH,
    CS_P, CS_PH,
    CS_Q, CS_QU,
    CS_R, CS_S, CS_T, CS_TH, CS_TR,
    CS_V, CS_X,
}

impl ConSeq {
    pub fn from_i16(val: i16) -> Self {
        if val >= ConSeq::Nil as i16 && val <= ConSeq::CS_X as i16 {
            unsafe { std::mem::transmute(val) }
        } else {
            ConSeq::Nil
        }
    }
}

use ConSeq::*;

// ============= Thông tin chuỗi nguyên âm =============

#[derive(Debug, Clone)]
pub struct VowelSeqInfo {
    pub len: usize,
    pub complete: bool,
    pub con_suffix: bool,
    pub v: [VnLexiName; 3],
    pub sub: [VowelSeq; 3],
    pub roof_pos: i32, // -1 nếu không áp dụng
    pub with_roof: VowelSeq,
    pub hook_pos: i32, // -1 nếu không áp dụng
    pub with_hook: VowelSeq,
}

// ============= Thông tin chuỗi phụ âm =============

#[derive(Debug, Clone)]
pub struct ConSeqInfo {
    pub len: usize,
    pub c: [VnLexiName; 3],
    pub suffix: bool,
}

/// N = NonVnChar viết tắt cho dễ đọc bảng
const NV: VnLexiName = NonVnChar;
const VN: VowelSeq = VowelSeq::Nil;

/// Danh sách VSeq đầy đủ — 71 định nghĩa chuỗi nguyên âm
pub static VSEQ_LIST: &[VowelSeqInfo] = &[
    // Nguyên âm đơn (0-11)
    VowelSeqInfo { len: 1, complete: true,  con_suffix: true,  v: [a, NV, NV],  sub: [VS_A, VN, VN],   roof_pos: -1, with_roof: VS_AR, hook_pos: -1, with_hook: VS_AB },
    VowelSeqInfo { len: 1, complete: true,  con_suffix: true,  v: [ar, NV, NV], sub: [VS_AR, VN, VN],  roof_pos: 0,  with_roof: VN,    hook_pos: -1, with_hook: VS_AB },
    VowelSeqInfo { len: 1, complete: true,  con_suffix: true,  v: [ab, NV, NV], sub: [VS_AB, VN, VN],  roof_pos: -1, with_roof: VS_AR, hook_pos: 0,  with_hook: VN },
    VowelSeqInfo { len: 1, complete: true,  con_suffix: true,  v: [e, NV, NV],  sub: [VS_E, VN, VN],   roof_pos: -1, with_roof: VS_ER, hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 1, complete: true,  con_suffix: true,  v: [er, NV, NV], sub: [VS_ER, VN, VN],  roof_pos: 0,  with_roof: VN,    hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 1, complete: true,  con_suffix: true,  v: [VnLexiName::i, NV, NV], sub: [VS_I, VN, VN],   roof_pos: -1, with_roof: VN,    hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 1, complete: true,  con_suffix: true,  v: [o, NV, NV],  sub: [VS_O, VN, VN],   roof_pos: -1, with_roof: VS_OR, hook_pos: -1, with_hook: VS_OH },
    VowelSeqInfo { len: 1, complete: true,  con_suffix: true,  v: [or, NV, NV], sub: [VS_OR, VN, VN],  roof_pos: 0,  with_roof: VN,    hook_pos: -1, with_hook: VS_OH },
    VowelSeqInfo { len: 1, complete: true,  con_suffix: true,  v: [oh, NV, NV], sub: [VS_OH, VN, VN],  roof_pos: -1, with_roof: VS_OR, hook_pos: 0,  with_hook: VN },
    VowelSeqInfo { len: 1, complete: true,  con_suffix: true,  v: [u, NV, NV],  sub: [VS_U, VN, VN],   roof_pos: -1, with_roof: VN,    hook_pos: -1, with_hook: VS_UH },
    VowelSeqInfo { len: 1, complete: true,  con_suffix: true,  v: [uh, NV, NV], sub: [VS_UH, VN, VN],  roof_pos: -1, with_roof: VN,    hook_pos: 0,  with_hook: VN },
    VowelSeqInfo { len: 1, complete: true,  con_suffix: true,  v: [y, NV, NV],  sub: [VS_Y, VN, VN],   roof_pos: -1, with_roof: VN,    hook_pos: -1, with_hook: VN },

    // Chuỗi 2 nguyên âm (12-48)
    VowelSeqInfo { len: 2, complete: true,  con_suffix: false, v: [a, VnLexiName::i, NV],  sub: [VS_A, VS_AI, VN],   roof_pos: -1, with_roof: VN,     hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: false, v: [a, o, NV],  sub: [VS_A, VS_AO, VN],   roof_pos: -1, with_roof: VN,     hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: false, v: [a, u, NV],  sub: [VS_A, VS_AU, VN],   roof_pos: -1, with_roof: VS_ARU, hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: false, v: [a, y, NV],  sub: [VS_A, VS_AY, VN],   roof_pos: -1, with_roof: VS_ARY, hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: false, v: [ar, u, NV], sub: [VS_AR, VS_ARU, VN], roof_pos: 0,  with_roof: VN,     hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: false, v: [ar, y, NV], sub: [VS_AR, VS_ARY, VN], roof_pos: 0,  with_roof: VN,     hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: false, v: [e, o, NV],  sub: [VS_E, VS_EO, VN],   roof_pos: -1, with_roof: VN,     hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: false, con_suffix: false, v: [e, u, NV],  sub: [VS_E, VS_EU, VN],   roof_pos: -1, with_roof: VS_ERU, hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: false, v: [er, u, NV], sub: [VS_ER, VS_ERU, VN], roof_pos: 0,  with_roof: VN,     hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: false, v: [VnLexiName::i, a, NV],  sub: [VS_I, VS_IA, VN],   roof_pos: -1, with_roof: VN,     hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: false, con_suffix: true,  v: [VnLexiName::i, e, NV],  sub: [VS_I, VS_IE, VN],   roof_pos: -1, with_roof: VS_IER, hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: true,  v: [VnLexiName::i, er, NV], sub: [VS_I, VS_IER, VN],  roof_pos: 1,  with_roof: VN,     hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: false, v: [VnLexiName::i, u, NV],  sub: [VS_I, VS_IU, VN],   roof_pos: -1, with_roof: VN,     hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: true,  v: [o, a, NV],  sub: [VS_O, VS_OA, VN],   roof_pos: -1, with_roof: VN,     hook_pos: -1, with_hook: VS_OAB },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: true,  v: [o, ab, NV], sub: [VS_O, VS_OAB, VN],  roof_pos: -1, with_roof: VN,     hook_pos: 1,  with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: true,  v: [o, e, NV],  sub: [VS_O, VS_OE, VN],   roof_pos: -1, with_roof: VN,     hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: false, v: [o, VnLexiName::i, NV],  sub: [VS_O, VS_OI, VN],   roof_pos: -1, with_roof: VS_ORI, hook_pos: -1, with_hook: VS_OHI },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: false, v: [or, VnLexiName::i, NV], sub: [VS_OR, VS_ORI, VN], roof_pos: 0,  with_roof: VN,     hook_pos: -1, with_hook: VS_OHI },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: false, v: [oh, VnLexiName::i, NV], sub: [VS_OH, VS_OHI, VN], roof_pos: -1, with_roof: VS_ORI, hook_pos: 0,  with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: true,  v: [u, a, NV],  sub: [VS_U, VS_UA, VN],   roof_pos: -1, with_roof: VS_UAR, hook_pos: -1, with_hook: VS_UHA },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: true,  v: [u, ar, NV], sub: [VS_U, VS_UAR, VN],  roof_pos: 1,  with_roof: VN,     hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: false, con_suffix: true,  v: [u, e, NV],  sub: [VS_U, VS_UE, VN],   roof_pos: -1, with_roof: VS_UER, hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: true,  v: [u, er, NV], sub: [VS_U, VS_UER, VN],  roof_pos: 1,  with_roof: VN,     hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: false, v: [u, VnLexiName::i, NV],  sub: [VS_U, VS_UI, VN],   roof_pos: -1, with_roof: VN,     hook_pos: -1, with_hook: VS_UHI },
    VowelSeqInfo { len: 2, complete: false, con_suffix: true,  v: [u, o, NV],  sub: [VS_U, VS_UO, VN],   roof_pos: -1, with_roof: VS_UOR, hook_pos: -1, with_hook: VS_UHO },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: true,  v: [u, or, NV], sub: [VS_U, VS_UOR, VN],  roof_pos: 1,  with_roof: VN,     hook_pos: -1, with_hook: VS_UOH },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: true,  v: [u, oh, NV], sub: [VS_U, VS_UOH, VN],  roof_pos: -1, with_roof: VS_UOR, hook_pos: 1,  with_hook: VS_UHOH },
    VowelSeqInfo { len: 2, complete: false, con_suffix: false, v: [u, u, NV],  sub: [VS_U, VS_UU, VN],   roof_pos: -1, with_roof: VN,     hook_pos: -1, with_hook: VS_UHU },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: true,  v: [u, y, NV],  sub: [VS_U, VS_UY, VN],   roof_pos: -1, with_roof: VN,     hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: false, v: [uh, a, NV], sub: [VS_UH, VS_UHA, VN], roof_pos: -1, with_roof: VN,     hook_pos: 0,  with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: false, v: [uh, VnLexiName::i, NV], sub: [VS_UH, VS_UHI, VN], roof_pos: -1, with_roof: VN,     hook_pos: 0,  with_hook: VN },
    VowelSeqInfo { len: 2, complete: false, con_suffix: true,  v: [uh, o, NV], sub: [VS_UH, VS_UHO, VN], roof_pos: -1, with_roof: VN,     hook_pos: 0,  with_hook: VS_UHOH },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: true,  v: [uh, oh, NV], sub: [VS_UH, VS_UHOH, VN], roof_pos: -1, with_roof: VN,  hook_pos: 0,  with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: false, v: [uh, u, NV], sub: [VS_UH, VS_UHU, VN], roof_pos: -1, with_roof: VN,     hook_pos: 0,  with_hook: VN },
    VowelSeqInfo { len: 2, complete: false, con_suffix: true,  v: [y, e, NV],  sub: [VS_Y, VS_YE, VN],   roof_pos: -1, with_roof: VS_YER, hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 2, complete: true,  con_suffix: true,  v: [y, er, NV], sub: [VS_Y, VS_YER, VN],  roof_pos: 1,  with_roof: VN,     hook_pos: -1, with_hook: VN },

    // Chuỗi 3 nguyên âm (49-70)
    VowelSeqInfo { len: 3, complete: false, con_suffix: false, v: [VnLexiName::i, e, u],  sub: [VS_I, VS_IE, VS_IEU],   roof_pos: -1, with_roof: VS_IERU, hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 3, complete: true,  con_suffix: false, v: [VnLexiName::i, er, u], sub: [VS_I, VS_IER, VS_IERU], roof_pos: 1,  with_roof: VN,      hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 3, complete: true,  con_suffix: false, v: [o, a, VnLexiName::i],  sub: [VS_O, VS_OA, VS_OAI],   roof_pos: -1, with_roof: VN,      hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 3, complete: true,  con_suffix: false, v: [o, a, y],  sub: [VS_O, VS_OA, VS_OAY],   roof_pos: -1, with_roof: VN,      hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 3, complete: true,  con_suffix: false, v: [o, e, o],  sub: [VS_O, VS_OE, VS_OEO],   roof_pos: -1, with_roof: VN,      hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 3, complete: false, con_suffix: false, v: [u, a, y],  sub: [VS_U, VS_UA, VS_UAY],   roof_pos: -1, with_roof: VS_UARY, hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 3, complete: true,  con_suffix: false, v: [u, ar, y], sub: [VS_U, VS_UAR, VS_UARY], roof_pos: 1,  with_roof: VN,      hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 3, complete: false, con_suffix: false, v: [u, o, VnLexiName::i],  sub: [VS_U, VS_UO, VS_UOI],   roof_pos: -1, with_roof: VS_UORI, hook_pos: -1, with_hook: VS_UHOI },
    VowelSeqInfo { len: 3, complete: false, con_suffix: false, v: [u, o, u],  sub: [VS_U, VS_UO, VS_UOU],   roof_pos: -1, with_roof: VN,      hook_pos: -1, with_hook: VS_UHOU },
    VowelSeqInfo { len: 3, complete: true,  con_suffix: false, v: [u, or, VnLexiName::i], sub: [VS_U, VS_UOR, VS_UORI], roof_pos: 1,  with_roof: VN,      hook_pos: -1, with_hook: VS_UOHI },
    VowelSeqInfo { len: 3, complete: false, con_suffix: false, v: [u, oh, VnLexiName::i], sub: [VS_U, VS_UOH, VS_UOHI], roof_pos: -1, with_roof: VS_UORI, hook_pos: 1,  with_hook: VS_UHOHI },
    VowelSeqInfo { len: 3, complete: false, con_suffix: false, v: [u, oh, u], sub: [VS_U, VS_UOH, VS_UOHU], roof_pos: -1, with_roof: VN,      hook_pos: 1,  with_hook: VS_UHOHU },
    VowelSeqInfo { len: 3, complete: true,  con_suffix: false, v: [u, y, a],  sub: [VS_U, VS_UY, VS_UYA],   roof_pos: -1, with_roof: VN,      hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 3, complete: false, con_suffix: true,  v: [u, y, e],  sub: [VS_U, VS_UY, VS_UYE],   roof_pos: -1, with_roof: VS_UYER, hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 3, complete: true,  con_suffix: true,  v: [u, y, er], sub: [VS_U, VS_UY, VS_UYER],  roof_pos: 2,  with_roof: VN,      hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 3, complete: true,  con_suffix: false, v: [u, y, u],  sub: [VS_U, VS_UY, VS_UYU],   roof_pos: -1, with_roof: VN,      hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 3, complete: false, con_suffix: false, v: [uh, o, VnLexiName::i], sub: [VS_UH, VS_UHO, VS_UHOI], roof_pos: -1, with_roof: VN, hook_pos: 0, with_hook: VS_UHOHI },
    VowelSeqInfo { len: 3, complete: false, con_suffix: false, v: [uh, o, u], sub: [VS_UH, VS_UHO, VS_UHOU], roof_pos: -1, with_roof: VN, hook_pos: 0, with_hook: VS_UHOHU },
    VowelSeqInfo { len: 3, complete: true,  con_suffix: false, v: [uh, oh, VnLexiName::i], sub: [VS_UH, VS_UHOH, VS_UHOHI], roof_pos: -1, with_roof: VN, hook_pos: 0, with_hook: VN },
    VowelSeqInfo { len: 3, complete: true,  con_suffix: false, v: [uh, oh, u], sub: [VS_UH, VS_UHOH, VS_UHOHU], roof_pos: -1, with_roof: VN, hook_pos: 0, with_hook: VN },
    VowelSeqInfo { len: 3, complete: false, con_suffix: false, v: [y, e, u],  sub: [VS_Y, VS_YE, VS_YEU],   roof_pos: -1, with_roof: VS_YERU, hook_pos: -1, with_hook: VN },
    VowelSeqInfo { len: 3, complete: true,  con_suffix: false, v: [y, er, u], sub: [VS_Y, VS_YER, VS_YERU], roof_pos: 1,  with_roof: VN,      hook_pos: -1, with_hook: VN },
];

/// Danh sách CSeq đầy đủ — 29 định nghĩa chuỗi phụ âm
pub static CSEQ_LIST: &[ConSeqInfo] = &[
    ConSeqInfo { len: 1, c: [VnLexiName::b, NV, NV], suffix: false },
    ConSeqInfo { len: 1, c: [VnLexiName::c, NV, NV], suffix: true },
    ConSeqInfo { len: 2, c: [VnLexiName::c, VnLexiName::h, NV], suffix: true },
    ConSeqInfo { len: 1, c: [VnLexiName::d, NV, NV], suffix: false },
    ConSeqInfo { len: 1, c: [dd, NV, NV], suffix: false },
    ConSeqInfo { len: 2, c: [VnLexiName::d, VnLexiName::z, NV], suffix: false },
    ConSeqInfo { len: 1, c: [VnLexiName::g, NV, NV], suffix: false },
    ConSeqInfo { len: 2, c: [VnLexiName::g, VnLexiName::h, NV], suffix: false },
    ConSeqInfo { len: 2, c: [VnLexiName::g, VnLexiName::i, NV], suffix: false },
    ConSeqInfo { len: 3, c: [VnLexiName::g, VnLexiName::i, VnLexiName::n], suffix: false },
    ConSeqInfo { len: 1, c: [VnLexiName::k, NV, NV], suffix: false },
    ConSeqInfo { len: 2, c: [VnLexiName::k, VnLexiName::h, NV], suffix: false },
    ConSeqInfo { len: 1, c: [VnLexiName::l, NV, NV], suffix: false },
    ConSeqInfo { len: 1, c: [VnLexiName::m, NV, NV], suffix: true },
    ConSeqInfo { len: 1, c: [VnLexiName::n, NV, NV], suffix: true },
    ConSeqInfo { len: 2, c: [VnLexiName::n, VnLexiName::g, NV], suffix: true },
    ConSeqInfo { len: 3, c: [VnLexiName::n, VnLexiName::g, VnLexiName::h], suffix: false },
    ConSeqInfo { len: 2, c: [VnLexiName::n, VnLexiName::h, NV], suffix: true },
    ConSeqInfo { len: 1, c: [VnLexiName::p, NV, NV], suffix: true },
    ConSeqInfo { len: 2, c: [VnLexiName::p, VnLexiName::h, NV], suffix: false },
    ConSeqInfo { len: 1, c: [VnLexiName::q, NV, NV], suffix: false },
    ConSeqInfo { len: 2, c: [VnLexiName::q, u, NV], suffix: false },
    ConSeqInfo { len: 1, c: [VnLexiName::r, NV, NV], suffix: false },
    ConSeqInfo { len: 1, c: [VnLexiName::s, NV, NV], suffix: false },
    ConSeqInfo { len: 1, c: [VnLexiName::t, NV, NV], suffix: true },
    ConSeqInfo { len: 2, c: [VnLexiName::t, VnLexiName::h, NV], suffix: false },
    ConSeqInfo { len: 2, c: [VnLexiName::t, VnLexiName::r, NV], suffix: false },
    ConSeqInfo { len: 1, c: [VnLexiName::v, NV, NV], suffix: false },
    ConSeqInfo { len: 1, c: [VnLexiName::x, NV, NV], suffix: false },
];

/// Bảng kiểm tra cặp VC (nguyên âm - phụ âm cuối)
pub static VC_PAIR_LIST: &[(VowelSeq, ConSeq)] = &[
    (VS_A, CS_C), (VS_A, CS_CH), (VS_A, CS_M), (VS_A, CS_N), (VS_A, CS_NG),
    (VS_A, CS_NH), (VS_A, CS_P), (VS_A, CS_T),
    (VS_AR, CS_C), (VS_AR, CS_M), (VS_AR, CS_N), (VS_AR, CS_NG), (VS_AR, CS_P), (VS_AR, CS_T),
    (VS_AB, CS_C), (VS_AB, CS_M), (VS_AB, CS_N), (VS_AB, CS_NG), (VS_AB, CS_P), (VS_AB, CS_T),
    (VS_E, CS_C), (VS_E, CS_CH), (VS_E, CS_M), (VS_E, CS_N), (VS_E, CS_NG),
    (VS_E, CS_NH), (VS_E, CS_P), (VS_E, CS_T),
    (VS_ER, CS_C), (VS_ER, CS_CH), (VS_ER, CS_M), (VS_ER, CS_N), (VS_ER, CS_NH),
    (VS_ER, CS_P), (VS_ER, CS_T),
    (VS_I, CS_C), (VS_I, CS_CH), (VS_I, CS_M), (VS_I, CS_N), (VS_I, CS_NH), (VS_I, CS_P), (VS_I, CS_T),
    (VS_O, CS_C), (VS_O, CS_M), (VS_O, CS_N), (VS_O, CS_NG), (VS_O, CS_P), (VS_O, CS_T),
    (VS_OR, CS_C), (VS_OR, CS_M), (VS_OR, CS_N), (VS_OR, CS_NG), (VS_OR, CS_P), (VS_OR, CS_T),
    (VS_OH, CS_M), (VS_OH, CS_N), (VS_OH, CS_P), (VS_OH, CS_T),
    (VS_U, CS_C), (VS_U, CS_M), (VS_U, CS_N), (VS_U, CS_NG), (VS_U, CS_P), (VS_U, CS_T),
    (VS_UH, CS_C), (VS_UH, CS_M), (VS_UH, CS_N), (VS_UH, CS_NG), (VS_UH, CS_T),
    (VS_Y, CS_T),
    (VS_IE, CS_C), (VS_IE, CS_M), (VS_IE, CS_N), (VS_IE, CS_NG), (VS_IE, CS_P), (VS_IE, CS_T),
    (VS_IER, CS_C), (VS_IER, CS_M), (VS_IER, CS_N), (VS_IER, CS_NG), (VS_IER, CS_P), (VS_IER, CS_T),
    (VS_OA, CS_C), (VS_OA, CS_CH), (VS_OA, CS_M), (VS_OA, CS_N), (VS_OA, CS_NG),
    (VS_OA, CS_NH), (VS_OA, CS_P), (VS_OA, CS_T),
    (VS_OAB, CS_C), (VS_OAB, CS_M), (VS_OAB, CS_N), (VS_OAB, CS_NG), (VS_OAB, CS_T),
    (VS_OE, CS_N), (VS_OE, CS_T),
    (VS_UA, CS_N), (VS_UA, CS_NG), (VS_UA, CS_T),
    (VS_UAR, CS_N), (VS_UAR, CS_NG), (VS_UAR, CS_T),
    (VS_UE, CS_C), (VS_UE, CS_CH), (VS_UE, CS_N), (VS_UE, CS_NH),
    (VS_UER, CS_C), (VS_UER, CS_CH), (VS_UER, CS_N), (VS_UER, CS_NH),
    (VS_UO, CS_C), (VS_UO, CS_M), (VS_UO, CS_N), (VS_UO, CS_NG), (VS_UO, CS_P), (VS_UO, CS_T),
    (VS_UOR, CS_C), (VS_UOR, CS_M), (VS_UOR, CS_N), (VS_UOR, CS_NG), (VS_UOR, CS_T),
    (VS_UY, CS_C), (VS_UY, CS_CH), (VS_UY, CS_N), (VS_UY, CS_NH), (VS_UY, CS_P), (VS_UY, CS_T),
    (VS_UHO, CS_C), (VS_UHO, CS_M), (VS_UHO, CS_N), (VS_UHO, CS_NG), (VS_UHO, CS_P), (VS_UHO, CS_T),
    (VS_UHOH, CS_C), (VS_UHOH, CS_M), (VS_UHOH, CS_N), (VS_UHOH, CS_NG), (VS_UHOH, CS_P), (VS_UHOH, CS_T),
    (VS_YE, CS_M), (VS_YE, CS_N), (VS_YE, CS_NG), (VS_YE, CS_P), (VS_YE, CS_T),
    (VS_YER, CS_M), (VS_YER, CS_N), (VS_YER, CS_NG), (VS_YER, CS_T),
    (VS_UYE, CS_N), (VS_UYE, CS_T),
    (VS_UYER, CS_N), (VS_UYER, CS_T),
];

/// Tra chuỗi nguyên âm từ 1-3 nguyên âm
pub fn lookup_vseq(v1: VnLexiName, v2: VnLexiName, v3: VnLexiName) -> VowelSeq {
    for (idx, info) in VSEQ_LIST.iter().enumerate() {
        if info.v[0] == v1 && info.v[1] == v2 && info.v[2] == v3 {
            return VowelSeq::from_i16(idx as i16);
        }
    }
    VowelSeq::Nil
}

/// Tra chuỗi nguyên âm từ 1 nguyên âm
pub fn lookup_vseq1(v1: VnLexiName) -> VowelSeq {
    lookup_vseq(v1, NonVnChar, NonVnChar)
}

/// Tra chuỗi nguyên âm từ 2 nguyên âm
pub fn lookup_vseq2(v1: VnLexiName, v2: VnLexiName) -> VowelSeq {
    lookup_vseq(v1, v2, NonVnChar)
}

/// Tra chuỗi phụ âm từ 1-3 phụ âm
pub fn lookup_cseq(c1: VnLexiName, c2: VnLexiName, c3: VnLexiName) -> ConSeq {
    for (idx, info) in CSEQ_LIST.iter().enumerate() {
        if info.c[0] == c1 && info.c[1] == c2 && info.c[2] == c3 {
            return ConSeq::from_i16(idx as i16);
        }
    }
    ConSeq::Nil
}

pub fn lookup_cseq1(c1: VnLexiName) -> ConSeq {
    lookup_cseq(c1, NonVnChar, NonVnChar)
}

pub fn lookup_cseq2(c1: VnLexiName, c2: VnLexiName) -> ConSeq {
    lookup_cseq(c1, c2, NonVnChar)
}

/// Lấy VowelSeqInfo cho VowelSeq đã cho
pub fn vseq_info(vs: VowelSeq) -> &'static VowelSeqInfo {
    let idx = vs as i16;
    if idx < 0 || idx as usize >= VSEQ_LIST.len() {
        return &VSEQ_LIST[0];
    }
    &VSEQ_LIST[idx as usize]
}

/// Lấy ConSeqInfo cho ConSeq đã cho
pub fn cseq_info(cs: ConSeq) -> &'static ConSeqInfo {
    let idx = cs as i16;
    if idx < 0 || idx as usize >= CSEQ_LIST.len() {
        return &CSEQ_LIST[0];
    }
    &CSEQ_LIST[idx as usize]
}

/// Kiểm tra cặp nguyên âm - phụ âm cuối có hợp lệ không
pub fn is_valid_vc(vs: VowelSeq, cs: ConSeq) -> bool {
    if vs == VowelSeq::Nil || cs == ConSeq::Nil {
        return true;
    }
    let v_info = vseq_info(vs);
    if !v_info.con_suffix {
        return false;
    }
    let c_info = cseq_info(cs);
    if !c_info.suffix {
        return false;
    }
    VC_PAIR_LIST.binary_search_by(|pair| {
        pair.0.cmp(&vs).then(pair.1.cmp(&cs))
    }).is_ok()
}

/// Kiểm tra cặp phụ âm đầu - nguyên âm có hợp lệ không
pub fn is_valid_cv(cs: ConSeq, vs: VowelSeq) -> bool {
    if cs == ConSeq::Nil || vs == VowelSeq::Nil {
        return true;
    }
    let v_info = vseq_info(vs);

    // gi không đi với i, qu không đi với u
    if (cs == CS_GI && v_info.v[0] == VnLexiName::i) ||
       (cs == CS_QU && v_info.v[0] == u) {
        return false;
    }

    // k chỉ đi với một số nguyên âm nhất định
    if cs == CS_K {
        let k_vseqs = [VS_E, VS_I, VS_Y, VS_ER, VS_EO, VS_EU,
                       VS_ERU, VS_IA, VS_IE, VS_IER, VS_IEU, VS_IERU];
        return k_vseqs.contains(&vs);
    }

    true
}

/// Kiểm tra tổ hợp phụ âm-nguyên âm-phụ âm đầy đủ
pub fn is_valid_cvc(c1: ConSeq, vs: VowelSeq, c2: ConSeq) -> bool {
    if vs == VowelSeq::Nil {
        return c1 == ConSeq::Nil || c2 != ConSeq::Nil;
    }
    if c1 == ConSeq::Nil {
        return is_valid_vc(vs, c2);
    }
    if c2 == ConSeq::Nil {
        return is_valid_cv(c1, vs);
    }

    let ok_cv = is_valid_cv(c1, vs);
    let ok_vc = is_valid_vc(vs, c2);

    if ok_cv && ok_vc {
        return true;
    }

    if !ok_vc {
        // quyn, quynh
        if c1 == CS_QU && vs == VS_Y && (c2 == CS_N || c2 == CS_NH) {
            return true;
        }
        // gieng, giêng
        if c1 == CS_GI && (vs == VS_E || vs == VS_ER) && (c2 == CS_N || c2 == CS_NG) {
            return true;
        }
    }
    false
}

/// Ánh xạ A-Z sang VnLexiName (chữ hoa)
pub static AZ_LEXI_UPPER: [VnLexiName; 26] = [
    A, B, C, D, E, F, G, H, I, J,
    K, L, M, N, O, P, Q, R, S, T,
    U, V, W, X, Y, Z,
];

/// Ánh xạ a-z sang VnLexiName (chữ thường)
pub static AZ_LEXI_LOWER: [VnLexiName; 26] = [
    a, VnLexiName::b, VnLexiName::c, VnLexiName::d, e, VnLexiName::f,
    VnLexiName::g, VnLexiName::h, VnLexiName::i, VnLexiName::j,
    VnLexiName::k, VnLexiName::l, VnLexiName::m, VnLexiName::n,
    o, VnLexiName::p, VnLexiName::q, VnLexiName::r, VnLexiName::s, VnLexiName::t,
    u, VnLexiName::v, VnLexiName::w, VnLexiName::x, y, VnLexiName::z,
];

/// Chuyển mã phím ASCII sang VnLexiName
pub fn iso_to_vn_lexi(key_code: u32) -> VnLexiName {
    if key_code >= 256 {
        return NonVnChar;
    }
    let ch = key_code as u8;
    match ch {
        b'a'..=b'z' => AZ_LEXI_LOWER[(ch - b'a') as usize],
        b'A'..=b'Z' => AZ_LEXI_UPPER[(ch - b'A') as usize],
        _ => NonVnChar,
    }
}
