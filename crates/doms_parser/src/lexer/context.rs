//! Token-context predicates used to disambiguate regexp literals and braces.

use super::token::TokenKind;

pub(super) fn is_expr_brace_context(prev: &Option<TokenKind>) -> bool {
    match prev {
        Some(TokenKind::LParen)
        | Some(TokenKind::LBracket)
        | Some(TokenKind::Comma)
        | Some(TokenKind::Colon)
        | Some(TokenKind::Question)
        | Some(TokenKind::Eq)
        | Some(TokenKind::PlusEq)
        | Some(TokenKind::MinusEq)
        | Some(TokenKind::StarEq)
        | Some(TokenKind::SlashEq)
        | Some(TokenKind::PercentEq)
        | Some(TokenKind::StarStarEq)
        | Some(TokenKind::LtLtEq)
        | Some(TokenKind::GtGtEq)
        | Some(TokenKind::GtGtGtEq)
        | Some(TokenKind::AmpEq)
        | Some(TokenKind::PipeEq)
        | Some(TokenKind::CaretEq)
        | Some(TokenKind::AmpAmpEq)
        | Some(TokenKind::PipePipeEq)
        | Some(TokenKind::QuestionQuestionEq)
        | Some(TokenKind::Plus)
        | Some(TokenKind::Minus)
        | Some(TokenKind::Star)
        | Some(TokenKind::Slash)
        | Some(TokenKind::Percent)
        | Some(TokenKind::StarStar)
        | Some(TokenKind::Lt)
        | Some(TokenKind::LtEq)
        | Some(TokenKind::Gt)
        | Some(TokenKind::GtEq)
        | Some(TokenKind::LtLt)
        | Some(TokenKind::GtGt)
        | Some(TokenKind::GtGtGt)
        | Some(TokenKind::EqEq)
        | Some(TokenKind::EqEqEq)
        | Some(TokenKind::NotEq)
        | Some(TokenKind::NotEqEq)
        | Some(TokenKind::Amp)
        | Some(TokenKind::Pipe)
        | Some(TokenKind::Caret)
        | Some(TokenKind::AmpAmp)
        | Some(TokenKind::PipePipe)
        | Some(TokenKind::QuestionQuestion)
        | Some(TokenKind::Bang)
        | Some(TokenKind::Tilde)
        | Some(TokenKind::DotDotDot)
        | Some(TokenKind::Return)
        | Some(TokenKind::Throw)
        | Some(TokenKind::Yield)
        | Some(TokenKind::Await)
        | Some(TokenKind::Case)
        | Some(TokenKind::In)
        | Some(TokenKind::Instanceof)
        | Some(TokenKind::Typeof)
        | Some(TokenKind::Void)
        | Some(TokenKind::Delete) => true,
        _ => false,
    }
}

pub(super) fn can_precede_slash_operator(prev: &Option<TokenKind>, last_rbrace_was_expr: bool) -> bool {
    match prev {
        Some(TokenKind::Ident(_))
        | Some(TokenKind::Number(_))
        | Some(TokenKind::BigInt(_))
        | Some(TokenKind::StringLit { .. })
        | Some(TokenKind::TemplateLiteral { .. })
        | Some(TokenKind::RParen)
        | Some(TokenKind::RBracket)
        | Some(TokenKind::This)
        | Some(TokenKind::Super)
        | Some(TokenKind::True)
        | Some(TokenKind::False)
        | Some(TokenKind::Null)
        | Some(TokenKind::Undefined)
        | Some(TokenKind::PlusPlus)
        | Some(TokenKind::MinusMinus)
        | Some(TokenKind::RegExp { .. })
        | Some(TokenKind::PrivateIdent(_)) => true,
        Some(TokenKind::RBrace) => last_rbrace_was_expr,
        _ => false,
    }
}
