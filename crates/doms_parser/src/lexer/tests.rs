use super::*;

#[test]
fn test_basic_lexing() {
    let source = "let x = 42; const msg = \"hello\";";
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize_all().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Let);
    assert_eq!(tokens[1].kind, TokenKind::Ident("x".into()));
    assert_eq!(tokens[2].kind, TokenKind::Eq);
    assert_eq!(tokens[3].kind, TokenKind::Number(42.0));
    assert_eq!(tokens[4].kind, TokenKind::Semicolon);
}
