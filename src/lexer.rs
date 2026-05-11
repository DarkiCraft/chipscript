#![allow(dead_code)]

use crate::error::{fail_at, LEX};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // -- LITERALS ------------------------------
    Int(i16),      // 10, 255
    Bool(bool),    // true, false
    Ident(String), // variable names, function names

    // -- KEYWORDS ------------------------------
    Vars,    // vars
    Sprites, // sprites
    Fn,      // fn
    Main,    // main
    If,      // if
    Elif,    // elif
    Else,    // else
    While,   // while
    Loop,    // loop

    // -- BUILTINS ------------------------------
    Draw,       // draw
    DrawDigit,  // drawdigit
    Clear,      // clear
    Delay,      // delay
    GetDelay,   // getdelay
    Beep,       // beep
    GetKey,     // getkey
    KeyPressed, // keypressed
    Rand,       // rand

    // -- OPERATORS ------------------------------
    Plus,    // +
    Minus,   // -
    Star,    // *
    Slash,   // /
    Percent, // %
    Equals,  // =
    EqEq,    // ==
    NotEq,   // !=
    Lt,      // <
    Gt,      // >
    LtEq,    // <=
    GtEq,    // >=
    And,     // and
    Or,      // or
    Not,     // not

    // -- DELIMITERS ------------------------------
    LParen,            // (
    RParen,            // )
    LBrace,            // {
    RBrace,            // }
    LBracket,          // [
    RBracket,          // ]
    Semicolon,         // ;
    Comma,             // ,
    Colon,             // :
    Arrow,             // ->
    Dot,               // .
    StringLit(String), // "file.spr"

    // -- SPECIAL ------------------------------
    EOF, // end of file
}

/// A token paired with the source line it started on (1-indexed).
pub type Spanned = (Token, u32);

pub fn lex(source: String) -> Vec<Spanned> {
    let mut tokens: Vec<Spanned> = Vec::new();
    let mut chars = source.chars().peekable();
    let mut line: u32 = 1;

    while let Some(c) = chars.next() {
        match c {
            // -- whitespace (track newlines) --------------------------
            '\n' => { line += 1; }
            ' ' | '\t' | '\r' => {}

            // -- single-character tokens ------------------------------
            '(' => tokens.push((Token::LParen,    line)),
            ')' => tokens.push((Token::RParen,    line)),
            '{' => tokens.push((Token::LBrace,    line)),
            '}' => tokens.push((Token::RBrace,    line)),
            '[' => tokens.push((Token::LBracket,  line)),
            ']' => tokens.push((Token::RBracket,  line)),
            ';' => tokens.push((Token::Semicolon, line)),
            ',' => tokens.push((Token::Comma,     line)),
            '+' => tokens.push((Token::Plus,      line)),
            '*' => tokens.push((Token::Star,      line)),
            '%' => tokens.push((Token::Percent,   line)),

            // -- two-character tokens ---------------------------------
            '-' => {
                if chars.peek() == Some(&'>') {
                    chars.next();
                    tokens.push((Token::Arrow, line));
                } else if chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    let start_line = line;
                    let mut num = String::new();
                    while let Some(&next) = chars.peek() {
                        if next.is_ascii_digit() {
                            num.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    let n: i16 = num.parse().unwrap_or_else(|_| {
                        fail_at(LEX, start_line, "negative number out of range")
                    });
                    tokens.push((Token::Int(-n), start_line));
                } else {
                    tokens.push((Token::Minus, line));
                }
            }
            '=' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push((Token::EqEq, line));
                } else {
                    tokens.push((Token::Equals, line));
                }
            }
            '!' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push((Token::NotEq, line));
                } else {
                    fail_at(LEX, line, "unexpected character '!' (did you mean '!='?)");
                }
            }
            '<' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push((Token::LtEq, line));
                } else {
                    tokens.push((Token::Lt, line));
                }
            }
            '>' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push((Token::GtEq, line));
                } else {
                    tokens.push((Token::Gt, line));
                }
            }
            '/' => {
                if chars.peek() == Some(&'/') {
                    // line comment — skip to end of line
                    for c in chars.by_ref() {
                        if c == '\n' {
                            line += 1;
                            break;
                        }
                    }
                } else {
                    tokens.push((Token::Slash, line));
                }
            }

            // -- string literals -------------------------------------
            '"' => {
                let start_line = line;
                let mut s = String::new();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\n') => {
                            line += 1;
                            s.push('\n');
                        }
                        Some(ch) => s.push(ch),
                        None => fail_at(LEX, start_line, "unterminated string literal"),
                    }
                }
                tokens.push((Token::StringLit(s), start_line));
            }

            // -- numbers ---------------------------------------------
            '0'..='9' => {
                let start_line = line;
                let mut num = String::new();
                num.push(c);
                if c == '0' && chars.peek() == Some(&'x') {
                    chars.next(); // consume 'x'
                    let mut hex = String::new();
                    while let Some(&next) = chars.peek() {
                        if next.is_ascii_hexdigit() {
                            hex.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    if hex.is_empty() {
                        fail_at(LEX, start_line, "expected hex digits after '0x'");
                    }
                    let value = i16::from_str_radix(&hex, 16).unwrap_or_else(|_| {
                        fail_at(LEX, start_line, format!("hex literal '0x{}' is too large", hex))
                    });
                    tokens.push((Token::Int(value), start_line));
                } else {
                    while let Some(&next) = chars.peek() {
                        if next.is_ascii_digit() {
                            num.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    let value: i16 = num.parse().unwrap_or_else(|_| {
                        fail_at(LEX, start_line, format!("integer literal '{}' is too large", num))
                    });
                    tokens.push((Token::Int(value), start_line));
                }
            }

            // -- identifiers and keywords ----------------------------
            'a'..='z' | 'A'..='Z' | '_' => {
                let start_line = line;
                let mut ident = String::new();
                ident.push(c);
                while let Some(&next) = chars.peek() {
                    if next.is_alphanumeric() || next == '_' {
                        ident.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                let token = match ident.as_str() {
                    "vars"       => Token::Vars,
                    "fn"         => Token::Fn,
                    "main"       => Token::Main,
                    "if"         => Token::If,
                    "elif"       => Token::Elif,
                    "else"       => Token::Else,
                    "loop"       => Token::Loop,
                    "while"      => Token::While,
                    "sprites"    => Token::Sprites,
                    "true"       => Token::Bool(true),
                    "false"      => Token::Bool(false),
                    "and"        => Token::And,
                    "or"         => Token::Or,
                    "not"        => Token::Not,
                    "draw"       => Token::Draw,
                    "clear"      => Token::Clear,
                    "delay"      => Token::Delay,
                    "getdelay"   => Token::GetDelay,
                    "beep"       => Token::Beep,
                    "getkey"     => Token::GetKey,
                    "keypressed" => Token::KeyPressed,
                    "rand"       => Token::Rand,
                    "drawdigit"  => Token::DrawDigit,
                    _            => Token::Ident(ident),
                };
                tokens.push((token, start_line));
            }

            other => fail_at(LEX, line, format!("unexpected character '{}'", other)),
        }
    }

    tokens.push((Token::EOF, line));
    tokens
}
