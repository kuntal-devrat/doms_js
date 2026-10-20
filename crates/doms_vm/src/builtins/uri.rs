//! Legacy URI encoding/decoding helpers (`encodeURI`, ...).

use super::*;

fn is_uri_mark(c: char) -> bool {
    matches!(c, '-' | '_' | '.' | '!' | '~' | '*' | '\'' | '(' | ')')
}

fn is_uri_reserved(c: char) -> bool {
    matches!(c, ';' | '/' | '?' | ':' | '@' | '&' | '=' | '+' | '$' | ',' | '#')
}

fn is_uri_alpha_num(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

pub(super) fn uri_encode(s: &str, unescape_reserved: bool) -> Result<String, VmError> {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '\u{FFFD}' {
            return Err(VmError::UriError("URI malformed".into()));
        }
        if is_uri_alpha_num(c) || is_uri_mark(c) || (unescape_reserved && is_uri_reserved(c)) {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            let encoded = c.encode_utf8(&mut buf);
            for b in encoded.bytes() {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    Ok(out)
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub(super) fn uri_decode(s: &str, preserve_reserved: bool) -> Result<String, VmError> {
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut decoded_bytes = Vec::with_capacity(len);
    let mut out = String::with_capacity(len);

    while i < len {
        let b = bytes[i];
        if b != b'%' {
            if !decoded_bytes.is_empty() {
                let s_part = std::str::from_utf8(&decoded_bytes)
                    .map_err(|_| VmError::UriError("URI malformed".into()))?;
                out.push_str(s_part);
                decoded_bytes.clear();
            }
            out.push(b as char);
            i += 1;
        } else {
            if i + 2 >= len {
                return Err(VmError::UriError("URI malformed".into()));
            }
            let h1 =
                hex_val(bytes[i + 1]).ok_or_else(|| VmError::UriError("URI malformed".into()))?;
            let h2 =
                hex_val(bytes[i + 2]).ok_or_else(|| VmError::UriError("URI malformed".into()))?;
            let byte_val = (h1 << 4) | h2;
            i += 3;

            if preserve_reserved && is_uri_reserved(byte_val as char) {
                if !decoded_bytes.is_empty() {
                    let s_part = std::str::from_utf8(&decoded_bytes)
                        .map_err(|_| VmError::UriError("URI malformed".into()))?;
                    out.push_str(s_part);
                    decoded_bytes.clear();
                }
                out.push('%');
                out.push(bytes[i - 2] as char);
                out.push(bytes[i - 1] as char);
            } else {
                decoded_bytes.push(byte_val);
            }
        }
    }
    if !decoded_bytes.is_empty() {
        let s_part = std::str::from_utf8(&decoded_bytes)
            .map_err(|_| VmError::UriError("URI malformed".into()))?;
        out.push_str(s_part);
    }
    Ok(out)
}
