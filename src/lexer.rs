#![allow(dead_code)]

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // -- LITERALS ------------------------------
    Int(i16),      // 10, 255
    Bool(bool),    // true, false
    Ident(String), // variable names, function names

    // -- KEYWORDS ------------------------------
    Vars,    // vars
    Sprites, // sprite
    Fn,      // fn
    Main,    // main
    If,      // if
    Elif,    // elif
    Else,    // else
    While,   // while
    Loop,    // loop

    // -- BUILTINS ------------------------------
    Draw,       // draw
    DrawDigit,  // draw builtin digit sprites
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

pub fn lex(source: String) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = source.chars().peekable();

    while let Some(c) = chars.next() {
        // grab next char, stop if none left
        match c {
            // skip whitespace
            ' ' | '\t' | '\n' | '\r' => {}

            // single character tokens
            '(' => tokens.push(Token::LParen),
            ')' => tokens.push(Token::RParen),
            '{' => tokens.push(Token::LBrace),
            '}' => tokens.push(Token::RBrace),
            '[' => tokens.push(Token::LBracket),
            ']' => tokens.push(Token::RBracket),
            ';' => tokens.push(Token::Semicolon),
            ',' => tokens.push(Token::Comma),
            '+' => tokens.push(Token::Plus),
            '*' => tokens.push(Token::Star),
            '%' => tokens.push(Token::Percent),

            // two character tokens (need to peek ahead)
            '-' => {
                if chars.peek() == Some(&'>') {
                    chars.next();
                    tokens.push(Token::Arrow);
                } else if chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    let mut num = String::new();
                    while let Some(&next) = chars.peek() {
                        if next.is_ascii_digit() {
                            num.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    let n: i16 = num.parse().expect("negative number out of range");
                    tokens.push(Token::Int(-n));
                } else {
                    tokens.push(Token::Minus);
                }
            }
            '=' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::EqEq);
                } else {
                    tokens.push(Token::Equals);
                }
            }
            '!' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::NotEq);
                } else {
                    panic!("unexpected character '!'"); // ! alone is invalid in ChipScript
                }
            }
            '<' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::LtEq);
                } else {
                    tokens.push(Token::Lt);
                }
            }
            '>' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::GtEq);
                } else {
                    tokens.push(Token::Gt);
                }
            }
            '/' => {
                if chars.peek() == Some(&'/') {
                    // it's a comment! skip until end of line
                    while let Some(c) = chars.next() {
                        if c == '\n' {
                            break;
                        }
                    }
                } else {
                    tokens.push(Token::Slash);
                }
            }

            // string literals "file.spr"
            '"' => {
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    if c == '"' {
                        break;
                    }
                    s.push(c);
                }
                tokens.push(Token::StringLit(s));
            }

            // numbers
            '0'..='9' => {
                let mut num = String::new();
                num.push(c);
                if c == '0' && chars.peek() == Some(&'x') {
                    chars.next();
                    let mut hex = String::new();
                    while let Some(&next) = chars.peek() {
                        if next.is_ascii_hexdigit() {
                            hex.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    let value = i16::from_str_radix(&hex, 16).expect("hex literal too large!");
                    tokens.push(Token::Int(value));
                } else {
                    while let Some(&next) = chars.peek() {
                        if next.is_ascii_digit() {
                            num.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    let value: i16 = num.parse().expect("number too large!");
                    tokens.push(Token::Int(value));
                }
            }

            // hex literals 0xFF
            // (already handled by number case for '0', but we peek for 'x')
            // handled inside the number arm above — we'll fix this in a sec

            // identifiers and keywords
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut ident = String::new();
                ident.push(c);
                while let Some(&next) = chars.peek() {
                    if next.is_alphanumeric() || next == '_' {
                        ident.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                // now check if it's a keyword
                let token = match ident.as_str() {
                    "vars" => Token::Vars,
                    "fn" => Token::Fn,
                    "main" => Token::Main,
                    "if" => Token::If,
                    "elif" => Token::Elif,
                    "else" => Token::Else,
                    "loop" => Token::Loop,
                    "while" => Token::While,
                    "sprites" => Token::Sprites,
                    "true" => Token::Bool(true),
                    "false" => Token::Bool(false),
                    "and" => Token::And,
                    "or" => Token::Or,
                    "not" => Token::Not,
                    "draw" => Token::Draw,
                    "clear" => Token::Clear,
                    "delay" => Token::Delay,
                    "getdelay" => Token::GetDelay,
                    "beep" => Token::Beep,
                    "getkey" => Token::GetKey,
                    "keypressed" => Token::KeyPressed,
                    "rand" => Token::Rand,
                    "drawdigit" => Token::DrawDigit,
                    _ => Token::Ident(ident), // not a keyword, it's a name
                };
                tokens.push(token);
            }

            other => panic!("unexpected character: '{}'", other),
        }
    }

    tokens.push(Token::EOF);
    tokens
}
