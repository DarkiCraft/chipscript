#![cfg(test)]

mod common;

use common::assert_panics;

use chipscript::lexer::{self, Token};

fn lex(src: &str) -> Vec<Token> {
    lexer::lex(src.to_string())
}

fn lex_no_eof(src: &str) -> Vec<Token> {
    let mut t = lex(src);
    assert_eq!(t.last(), Some(&Token::EOF));
    t.pop();
    t
}

// --- Literals -----------------------------------------------------------

#[test]
fn decimal_integer() {
    assert_eq!(lex_no_eof("42"), vec![Token::Int(42)]);
}

#[test]
fn zero() {
    assert_eq!(lex_no_eof("0"), vec![Token::Int(0)]);
}

#[test]
fn negative_integer() {
    assert_eq!(lex_no_eof("-10"), vec![Token::Int(-10)]);
}

#[test]
fn hex_integer() {
    assert_eq!(lex_no_eof("0xFF"), vec![Token::Int(0xFF)]);
    assert_eq!(lex_no_eof("0x0F"), vec![Token::Int(0x0F)]);
    assert_eq!(lex_no_eof("0x00"), vec![Token::Int(0x00)]);
}

#[test]
fn bool_true() {
    assert_eq!(lex_no_eof("true"), vec![Token::Bool(true)]);
}

#[test]
fn bool_false() {
    assert_eq!(lex_no_eof("false"), vec![Token::Bool(false)]);
}

#[test]
fn string_literal() {
    assert_eq!(
        lex_no_eof(r#""ship.spr""#),
        vec![Token::StringLit("ship.spr".into())]
    );
}

#[test]
fn string_literal_with_path() {
    assert_eq!(
        lex_no_eof(r#""assets/player.spr""#),
        vec![Token::StringLit("assets/player.spr".into())]
    );
}

// --- Keywords -----------------------------------------------------------

#[test]
fn all_keywords() {
    let src = "vars sprites fn main if elif else while loop";
    let tokens = lex_no_eof(src);
    assert_eq!(
        tokens,
        vec![
            Token::Vars,
            Token::Sprites,
            Token::Fn,
            Token::Main,
            Token::If,
            Token::Elif,
            Token::Else,
            Token::While,
            Token::Loop,
        ]
    );
}

#[test]
fn all_builtins() {
    let src = "draw drawdigit clear delay getdelay beep getkey keypressed rand";
    let tokens = lex_no_eof(src);
    assert_eq!(
        tokens,
        vec![
            Token::Draw,
            Token::DrawDigit,
            Token::Clear,
            Token::Delay,
            Token::GetDelay,
            Token::Beep,
            Token::GetKey,
            Token::KeyPressed,
            Token::Rand,
        ]
    );
}

#[test]
fn logical_keywords() {
    assert_eq!(
        lex_no_eof("and or not"),
        vec![Token::And, Token::Or, Token::Not]
    );
}

// --- Operators ----------------------------------------------------------

#[test]
fn arithmetic_operators() {
    assert_eq!(
        lex_no_eof("+ - * / %"),
        vec![
            Token::Plus,
            Token::Minus,
            Token::Star,
            Token::Slash,
            Token::Percent,
        ]
    );
}

#[test]
fn comparison_operators() {
    assert_eq!(
        lex_no_eof("== != < > <= >="),
        vec![
            Token::EqEq,
            Token::NotEq,
            Token::Lt,
            Token::Gt,
            Token::LtEq,
            Token::GtEq,
        ]
    );
}

#[test]
fn assign_vs_eqeq() {
    assert_eq!(lex_no_eof("= =="), vec![Token::Equals, Token::EqEq]);
}

#[test]
fn arrow() {
    assert_eq!(lex_no_eof("->"), vec![Token::Arrow]);
}

#[test]
fn minus_vs_arrow_vs_negative() {
    // '-' alone = Minus; '->' = Arrow; '-5' = Int(-5)
    assert_eq!(
        lex_no_eof("- -> -5"),
        vec![Token::Minus, Token::Arrow, Token::Int(-5),]
    );
}

// --- Delimiters ---------------------------------------------------------

#[test]
fn delimiters() {
    assert_eq!(
        lex_no_eof("( ) { } [ ] ; ,"),
        vec![
            Token::LParen,
            Token::RParen,
            Token::LBrace,
            Token::RBrace,
            Token::LBracket,
            Token::RBracket,
            Token::Semicolon,
            Token::Comma,
        ]
    );
}

// --- Identifiers --------------------------------------------------------

#[test]
fn plain_identifier() {
    assert_eq!(lex_no_eof("myVar"), vec![Token::Ident("myVar".into())]);
}

#[test]
fn underscore_identifier() {
    assert_eq!(lex_no_eof("_x"), vec![Token::Ident("_x".into())]);
}

#[test]
fn identifier_with_digits() {
    assert_eq!(lex_no_eof("x1"), vec![Token::Ident("x1".into())]);
}

// --- Whitespace & comments ---------------------------------------------

#[test]
fn whitespace_ignored() {
    assert_eq!(lex_no_eof("  42   "), vec![Token::Int(42)]);
}

#[test]
fn comment_skipped() {
    assert_eq!(lex_no_eof("42 // this is ignored\n"), vec![Token::Int(42)]);
}

#[test]
fn comment_at_end_no_newline() {
    assert_eq!(lex_no_eof("42 // no newline"), vec![Token::Int(42)]);
}

#[test]
fn multiline_with_comments() {
    let src = "// line 1\n42\n// line 2\n";
    assert_eq!(lex_no_eof(src), vec![Token::Int(42)]);
}

// --- EOF ----------------------------------------------------------------

#[test]
fn always_ends_with_eof() {
    let t = lex("42");
    assert_eq!(t.last(), Some(&Token::EOF));
}

#[test]
fn empty_source_is_just_eof() {
    assert_eq!(lex(""), vec![Token::EOF]);
}

// --- Error cases --------------------------------------------------------

#[test]
fn bare_exclamation_panics() {
    assert_panics(|| {
        lex("!");
    });
}

#[test]
fn unexpected_character_panics() {
    assert_panics(|| {
        lex("@");
    });
}
