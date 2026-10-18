//! Strict recursive-descent JSON parser.

use super::*;


pub(super) struct JsonParser<'a> {
    pub(super) chars: Vec<char>,
    pub(super) pos: usize,
    pub(super) _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> JsonParser<'a> {
    pub(super) fn new(text: &'a str) -> Self {
        Self {
            chars: text.chars().collect(),
            pos: 0,
            _phantom: std::marker::PhantomData,
        }
    }

    pub(super) fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() {
            match self.chars[self.pos] {
                ' ' | '\t' | '\r' | '\n' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        if self.pos < self.chars.len() {
            let c = self.chars[self.pos];
            self.pos += 1;
            Some(c)
        } else {
            None
        }
    }

    pub(super) fn parse_value(&mut self, vm: &mut Vm) -> Result<JsValue, VmError> {
        self.skip_whitespace();
        let c = self.peek().ok_or_else(|| VmError::SyntaxError("Unexpected end of JSON input".into()))?;
        match c {
            'n' => self.parse_null(),
            't' => self.parse_true(),
            'f' => self.parse_false(),
            '"' => self.parse_string(vm),
            '[' => self.parse_array(vm),
            '{' => self.parse_object(vm),
            '-' | '0'..='9' => self.parse_number(),
            _ => Err(VmError::SyntaxError(format!("Unexpected character '{}' in JSON", c))),
        }
    }

    fn parse_null(&mut self) -> Result<JsValue, VmError> {
        if self.pos + 4 <= self.chars.len() && &self.chars[self.pos..self.pos + 4] == &['n', 'u', 'l', 'l'] {
            self.pos += 4;
            Ok(JsValue::null())
        } else {
            Err(VmError::SyntaxError("Invalid JSON: expected null".into()))
        }
    }

    fn parse_true(&mut self) -> Result<JsValue, VmError> {
        if self.pos + 4 <= self.chars.len() && &self.chars[self.pos..self.pos + 4] == &['t', 'r', 'u', 'e'] {
            self.pos += 4;
            Ok(JsValue::from_boolean(true))
        } else {
            Err(VmError::SyntaxError("Invalid JSON: expected true".into()))
        }
    }

    fn parse_false(&mut self) -> Result<JsValue, VmError> {
        if self.pos + 5 <= self.chars.len() && &self.chars[self.pos..self.pos + 5] == &['f', 'a', 'l', 's', 'e'] {
            self.pos += 5;
            Ok(JsValue::from_boolean(false))
        } else {
            Err(VmError::SyntaxError("Invalid JSON: expected false".into()))
        }
    }

    fn parse_string_raw(&mut self) -> Result<String, VmError> {
        self.skip_whitespace();
        if self.advance() != Some('"') {
            return Err(VmError::SyntaxError("Expected string in JSON".into()));
        }
        let mut s = String::new();
        while let Some(c) = self.advance() {
            match c {
                '"' => return Ok(s),
                '\\' => {
                    let esc = self.advance().ok_or_else(|| VmError::SyntaxError("Unterminated escape sequence".into()))?;
                    match esc {
                        '"' => s.push('"'),
                        '\\' => s.push('\\'),
                        '/' => s.push('/'),
                        'b' => s.push('\u{08}'),
                        'f' => s.push('\u{0C}'),
                        'n' => s.push('\n'),
                        'r' => s.push('\r'),
                        't' => s.push('\t'),
                        'u' => {
                            let mut hex = String::new();
                            for _ in 0..4 {
                                let hc = self.advance().ok_or_else(|| VmError::SyntaxError("Invalid unicode escape".into()))?;
                                if !hc.is_ascii_hexdigit() {
                                    return Err(VmError::SyntaxError("Invalid hex digit in unicode escape".into()));
                                }
                                hex.push(hc);
                            }
                            let code = u32::from_str_radix(&hex, 16)
                                .map_err(|_| VmError::SyntaxError("Invalid unicode escape value".into()))?;
                            if (0xD800..=0xDBFF).contains(&code) {
                                // High surrogate: check for low surrogate
                                if self.pos + 6 <= self.chars.len() && self.chars[self.pos] == '\\' && self.chars[self.pos + 1] == 'u' {
                                    let mut low_hex = String::new();
                                    for idx in 0..4 {
                                        let hc = self.chars[self.pos + 2 + idx];
                                        if !hc.is_ascii_hexdigit() { break; }
                                        low_hex.push(hc);
                                    }
                                    if low_hex.len() == 4 {
                                        if let Ok(low_code) = u32::from_str_radix(&low_hex, 16) {
                                            if (0xDC00..=0xDFFF).contains(&low_code) {
                                                self.pos += 6;
                                                let full_code = 0x10000 + ((code - 0xD800) << 10) + (low_code - 0xDC00);
                                                if let Some(ch) = char::from_u32(full_code) {
                                                    s.push(ch);
                                                    continue;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            if let Some(ch) = char::from_u32(code) {
                                s.push(ch);
                            } else {
                                s.push('\u{FFFD}');
                            }
                        }
                        _ => return Err(VmError::SyntaxError(format!("Invalid escape character '{}'", esc))),
                    }
                }
                c if (c as u32) < 0x20 => {
                    return Err(VmError::SyntaxError("Unescaped control character in JSON string".into()));
                }
                other => s.push(other),
            }
        }
        Err(VmError::SyntaxError("Unterminated string in JSON".into()))
    }

    fn parse_string(&mut self, vm: &mut Vm) -> Result<JsValue, VmError> {
        let s = self.parse_string_raw()?;
        Ok(vm.alloc_string(s))
    }

    fn parse_number(&mut self) -> Result<JsValue, VmError> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.advance();
        }

        let first = self.advance().ok_or_else(|| VmError::SyntaxError("Expected digits for number".into()))?;
        if !first.is_ascii_digit() {
            return Err(VmError::SyntaxError("Expected digit".into()));
        }

        if first == '0' {
            if let Some(next) = self.peek() {
                if next.is_ascii_digit() {
                    return Err(VmError::SyntaxError("Leading zeros are not allowed in JSON numbers".into()));
                }
            }
        } else {
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        // Fractional part
        if self.peek() == Some('.') {
            self.advance();
            let mut has_digit = false;
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    has_digit = true;
                    self.advance();
                } else {
                    break;
                }
            }
            if !has_digit {
                return Err(VmError::SyntaxError("Decimal point must be followed by digits".into()));
            }
        }

        // Exponent part
        if self.peek() == Some('e') || self.peek() == Some('E') {
            self.advance();
            if self.peek() == Some('+') || self.peek() == Some('-') {
                self.advance();
            }
            let mut has_exp_digit = false;
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    has_exp_digit = true;
                    self.advance();
                } else {
                    break;
                }
            }
            if !has_exp_digit {
                return Err(VmError::SyntaxError("Exponent must be followed by digits".into()));
            }
        }

        let num_str: String = self.chars[start..self.pos].iter().collect();
        let val = num_str.parse::<f64>().map_err(|_| VmError::SyntaxError("Failed to parse number".into()))?;
        Ok(JsValue::from_double(val))
    }

    fn parse_array(&mut self, vm: &mut Vm) -> Result<JsValue, VmError> {
        self.advance(); // consume '['
        self.skip_whitespace();
        let mut elements = Vec::new();
        if self.peek() == Some(']') {
            self.advance();
            return Ok(vm.alloc_array(elements));
        }

        loop {
            let elem = self.parse_value(vm)?;
            elements.push(elem);
            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.advance();
                    self.skip_whitespace();
                    if self.peek() == Some(']') {
                        return Err(VmError::SyntaxError("Trailing comma in JSON array".into()));
                    }
                }
                Some(']') => {
                    self.advance();
                    break;
                }
                _ => return Err(VmError::SyntaxError("Expected ',' or ']' in JSON array".into())),
            }
        }

        Ok(vm.alloc_array(elements))
    }

    fn parse_object(&mut self, vm: &mut Vm) -> Result<JsValue, VmError> {
        self.advance(); // consume '{'
        self.skip_whitespace();
        let obj = vm.alloc_object();
        if self.peek() == Some('}') {
            self.advance();
            return Ok(obj);
        }

        loop {
            self.skip_whitespace();
            if self.peek() != Some('"') {
                return Err(VmError::SyntaxError("Property name must be double-quoted in JSON object".into()));
            }
            let key = self.parse_string_raw()?;
            self.skip_whitespace();
            if self.advance() != Some(':') {
                return Err(VmError::SyntaxError("Expected ':' after property name in JSON object".into()));
            }
            let val = self.parse_value(vm)?;
            vm.set_property(obj, &key, val);

            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.advance();
                    self.skip_whitespace();
                    if self.peek() == Some('}') {
                        return Err(VmError::SyntaxError("Trailing comma in JSON object".into()));
                    }
                }
                Some('}') => {
                    self.advance();
                    break;
                }
                _ => return Err(VmError::SyntaxError("Expected ',' or '}' in JSON object".into())),
            }
        }

        Ok(obj)
    }
}
