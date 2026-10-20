//! UTF-16 code unit and surrogate-pair helpers used by string scanning.

pub(super) fn flush_surrogate(s: &mut String) -> bool {
    let bytes = s.as_bytes();
    let len = bytes.len();
    if len >= 3
        && bytes[len - 3] == 0xED
        && (0xA0..=0xAF).contains(&bytes[len - 2])
        && (0x80..=0xBF).contains(&bytes[len - 1])
    {
        s.truncate(len - 3);
        s.push('\u{FFFD}');
        true
    } else {
        false
    }
}

pub(super) fn push_unicode_code_unit(s: &mut String, code: u32) -> bool {
    if (0xD800..=0xDBFF).contains(&code) {
        let unp = flush_surrogate(s);
        unsafe {
            s.as_mut_vec().extend_from_slice(&[
                0xED,
                (0x80 | ((code >> 6) & 0x3F)) as u8,
                (0x80 | (code & 0x3F)) as u8,
            ]);
        }
        unp
    } else if (0xDC00..=0xDFFF).contains(&code) {
        let bytes = s.as_bytes();
        let len = bytes.len();
        if len >= 3
            && bytes[len - 3] == 0xED
            && (0xA0..=0xAF).contains(&bytes[len - 2])
            && (0x80..=0xBF).contains(&bytes[len - 1])
        {
            let b1 = bytes[len - 2] as u32;
            let b2 = bytes[len - 1] as u32;
            let high = 0xD800 | ((b1 & 0x0F) << 6) | (b2 & 0x3F);
            s.truncate(len - 3);
            let combined = 0x10000 + ((high - 0xD800) << 10) + (code - 0xDC00);
            if let Some(ch) = char::from_u32(combined) {
                s.push(ch);
                return false;
            }
        }
        s.push('\u{FFFD}');
        true
    } else {
        let unp = flush_surrogate(s);
        if let Some(ch) = char::from_u32(code) {
            s.push(ch);
            unp
        } else {
            s.push('\u{FFFD}');
            true
        }
    }
}
