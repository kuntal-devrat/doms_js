//! Character classification helpers for the JavaScript lexical grammar.

#[inline]
pub fn is_line_terminator(c: char) -> bool {
    c == '\n' || c == '\r' || c == '\u{2028}' || c == '\u{2029}'
}

#[inline]
pub fn is_ecma_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'
            | '\u{000B}'
            | '\u{000C}'
            | '\u{0020}'
            | '\u{00A0}'
            | '\u{FEFF}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

#[inline]
pub fn is_identifier_start(c: char) -> bool {
    c == '$'
        || c == '_'
        || unicode_ident::is_xid_start(c)
        || matches!(
            c,
            '\u{1885}'..='\u{1886}'
                | '\u{2118}'
                | '\u{212E}'
                | '\u{309B}'..='\u{309C}'
                | '\u{0E33}'
                | '\u{0EB3}'
        )
}

#[inline]
pub fn is_identifier_part(c: char) -> bool {
    c == '$'
        || c == '_'
        || c == '\u{200C}'
        || c == '\u{200D}'
        || unicode_ident::is_xid_continue(c)
        || matches!(
            c,
            '\u{1885}'..='\u{1886}'
                | '\u{2118}'
                | '\u{212E}'
                | '\u{309B}'..='\u{309C}'
                | '\u{0E33}'
                | '\u{0EB3}'
                | '\u{FF9E}'..='\u{FF9F}'
                | '\u{00B7}'
                | '\u{0387}'
                | '\u{1369}'..='\u{1371}'
                | '\u{19DA}'
        )
}

pub fn is_reserved_word(s: &str) -> bool {
    matches!(
        s,
        "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "export"
            | "extends"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "new"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "enum"
            | "null"
            | "true"
            | "false"
    )
}
