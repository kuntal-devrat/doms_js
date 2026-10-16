use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn scan_declared_private_names(&self) -> HashSet<String> {
        let mut names = HashSet::new();
        let mut lex = self.lexer.clone();
        let mut prev_kind = Some(self.current.kind.clone());
        let mut brace_depth = 1usize;

        match &self.current.kind {
            TokenKind::LBrace => brace_depth += 1,
            TokenKind::RBrace => {
                brace_depth = brace_depth.saturating_sub(1);
            }
            TokenKind::PrivateIdent(s) => {
                if brace_depth == 1 {
                    names.insert(format!("#{}", s));
                }
            }
            _ => {}
        }
        if brace_depth == 0 {
            return names;
        }

        let mut cur_tok = self.peeked.clone();
        loop {
            match &cur_tok.kind {
                TokenKind::LBrace => {
                    brace_depth += 1;
                }
                TokenKind::RBrace => {
                    brace_depth = brace_depth.saturating_sub(1);
                    if brace_depth == 0 {
                        break;
                    }
                }
                TokenKind::Eof => {
                    break;
                }
                TokenKind::PrivateIdent(s) => {
                    if brace_depth == 1 && prev_kind != Some(TokenKind::Dot) {
                        names.insert(format!("#{}", s));
                    }
                }
                _ => {}
            }
            prev_kind = Some(cur_tok.kind);
            match lex.next_token() {
                Ok(t) => cur_tok = t,
                Err(_) => break,
            }
        }
        names
    }
}
