#![allow(dead_code)]

use crate::ast::*;
use crate::error::{PARSE, fail, fail_at};
use crate::lexer::{Spanned, Token};

pub struct Parser {
    tokens: Vec<Spanned>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Spanned>) -> Self {
        Parser { tokens, pos: 0 }
    }

    // -- token access helpers ------------------------------------

    /// Current token (without consuming).
    fn peek(&self) -> &Token {
        &self.tokens[self.pos].0
    }

    /// Line number of the current token.
    fn line(&self) -> u32 {
        self.tokens[self.pos].1
    }

    /// Consume and return the current token.
    fn advance(&mut self) -> &Token {
        let t = &self.tokens[self.pos].0;
        self.pos += 1;
        t
    }

    /// Consume the current token only if it matches `expected`, else abort.
    fn expect(&mut self, expected: &Token) -> &Token {
        let (t, ln) = &self.tokens[self.pos];
        if t != expected {
            fail_at(
                PARSE,
                *ln,
                format!("expected {:?} but got {:?}", expected, t),
            );
        }
        self.pos += 1;
        &self.tokens[self.pos - 1].0
    }

    /// True if the current token matches `expected` (does not consume).
    fn check(&self, expected: &Token) -> bool {
        self.peek() == expected
    }

    // -- ENTRY POINT ------------------------------------------
    pub fn parse(&mut self) -> Program {
        let mut sprites = Vec::new();
        let mut vars = Vec::new();
        let mut functions = Vec::new();
        let mut main = Vec::new();
        let mut seen_spr = false;
        let mut seen_vars = false;
        let mut seen_main = false;

        while !self.check(&Token::EOF) {
            let ln = self.line();
            match self.peek().clone() {
                Token::Sprites => {
                    if seen_spr {
                        fail_at(PARSE, ln, "duplicate 'sprites' section");
                    }
                    seen_spr = true;
                    sprites = self.parse_sprites_block();
                }
                Token::Vars => {
                    if seen_vars {
                        fail_at(PARSE, ln, "duplicate 'vars' section");
                    }
                    seen_vars = true;
                    vars = self.parse_vars();
                }
                Token::Fn => functions.push(self.parse_fn()),
                Token::Main => {
                    if seen_main {
                        fail_at(PARSE, ln, "duplicate 'main' section");
                    }
                    seen_main = true;
                    main = self.parse_main();
                }
                other => fail_at(
                    PARSE,
                    ln,
                    format!("unexpected token at top level: {:?}", other),
                ),
            }
        }

        Program {
            sprites,
            vars,
            functions,
            main,
        }
    }

    // -- SPRITE -----------------------------------------------
    fn parse_sprites_block(&mut self) -> Vec<SpriteDecl> {
        self.advance(); // consume 'sprites'
        self.expect(&Token::LBrace);
        let mut sprites = Vec::new();

        while !self.check(&Token::RBrace) {
            let ln = self.line();
            let name = match self.advance().clone() {
                Token::Ident(n) => n,
                other => fail_at(PARSE, ln, format!("expected sprite name, got {:?}", other)),
            };
            self.expect(&Token::Equals);

            let data_ln = self.line();
            let data = match self.peek().clone() {
                Token::LBracket => {
                    self.advance();
                    let mut bytes = Vec::new();
                    while !self.check(&Token::RBracket) {
                        let byte_ln = self.line();
                        match self.advance().clone() {
                            Token::Int(b) => bytes.push(b as u8),
                            other => fail_at(
                                PARSE,
                                byte_ln,
                                format!("expected byte in sprite data, got {:?}", other),
                            ),
                        }
                        if self.check(&Token::Comma) {
                            self.advance();
                        }
                    }
                    self.expect(&Token::RBracket);
                    SpriteData::Inline(bytes)
                }
                Token::StringLit(s) => {
                    let s = s.clone();
                    self.advance();
                    SpriteData::File(s)
                }
                other => fail_at(
                    PARSE,
                    data_ln,
                    format!(
                        "expected '[' or string path for sprite data, got {:?}",
                        other
                    ),
                ),
            };
            self.expect(&Token::Semicolon);
            sprites.push(SpriteDecl { name, data });
        }
        self.expect(&Token::RBrace);
        sprites
    }

    // -- VARS -------------------------------------------------
    fn parse_vars(&mut self) -> Vec<VarDecl> {
        self.advance(); // consume 'vars'
        self.expect(&Token::LBrace);
        let mut vars = Vec::new();

        while !self.check(&Token::RBrace) {
            let ln = self.line();
            let name = match self.advance().clone() {
                Token::Ident(n) => n,
                other => fail_at(
                    PARSE,
                    ln,
                    format!("expected variable name, got {:?}", other),
                ),
            };
            self.expect(&Token::Equals);
            let value = self.parse_expr();
            self.expect(&Token::Semicolon);
            vars.push(VarDecl { name, value });
        }
        self.expect(&Token::RBrace);
        vars
    }

    // -- FN ---------------------------------------------------
    fn parse_fn(&mut self) -> FnDecl {
        self.advance(); // consume 'fn'
        let name_ln = self.line();
        let name = match self.advance().clone() {
            Token::Ident(n) => n,
            other => fail_at(
                PARSE,
                name_ln,
                format!("expected function name, got {:?}", other),
            ),
        };
        self.expect(&Token::LParen);
        let mut args = Vec::new();
        while !self.check(&Token::RParen) {
            let arg_ln = self.line();
            match self.advance().clone() {
                Token::Ident(n) => args.push(n),
                other => fail_at(
                    PARSE,
                    arg_ln,
                    format!("expected argument name, got {:?}", other),
                ),
            }
            if self.check(&Token::Comma) {
                self.advance();
            }
        }
        self.expect(&Token::RParen);
        self.expect(&Token::Arrow);
        let ret_ln = self.line();
        let ret = match self.advance().clone() {
            Token::Ident(n) => n,
            other => fail_at(
                PARSE,
                ret_ln,
                format!("expected return variable name, got {:?}", other),
            ),
        };
        let body = self.parse_block();
        FnDecl {
            name,
            args,
            ret,
            body,
        }
    }

    // -- MAIN -------------------------------------------------
    fn parse_main(&mut self) -> Vec<Stmt> {
        self.advance(); // consume 'main'
        self.parse_block()
    }

    // -- BLOCK { stmts } --------------------------------------
    fn parse_block(&mut self) -> Vec<Stmt> {
        self.expect(&Token::LBrace);
        let mut stmts = Vec::new();
        while !self.check(&Token::RBrace) {
            stmts.push(self.parse_stmt());
        }
        self.expect(&Token::RBrace);
        stmts
    }

    // -- STATEMENT --------------------------------------------
    fn parse_stmt(&mut self) -> Stmt {
        let ln = self.line();
        match self.peek().clone() {
            Token::If => self.parse_if(),
            Token::Loop => {
                self.advance();
                let body = self.parse_block();
                Stmt::Loop(body)
            }
            Token::While => self.parse_while(),
            Token::Clear => {
                self.advance();
                self.expect(&Token::LParen);
                self.expect(&Token::RParen);
                self.expect(&Token::Semicolon);
                Stmt::Clear
            }
            Token::Delay => {
                self.advance();
                self.expect(&Token::LParen);
                let e = self.parse_expr();
                self.expect(&Token::RParen);
                self.expect(&Token::Semicolon);
                Stmt::Delay(e)
            }
            Token::Beep => {
                self.advance();
                self.expect(&Token::LParen);
                let e = self.parse_expr();
                self.expect(&Token::RParen);
                self.expect(&Token::Semicolon);
                Stmt::Beep(e)
            }
            Token::Ident(_) => self.parse_assign_or_call(),
            other => fail_at(
                PARSE,
                ln,
                format!("unexpected token at start of statement: {:?}", other),
            ),
        }
    }

    // -- ASSIGN or CALL ---------------------------------------
    fn parse_assign_or_call(&mut self) -> Stmt {
        let ln = self.line();
        let name = match self.advance().clone() {
            Token::Ident(n) => n,
            other => fail_at(PARSE, ln, format!("expected identifier, got {:?}", other)),
        };
        let after_ln = self.line();
        match self.peek().clone() {
            Token::Equals => {
                self.advance();
                let expr = self.parse_expr();
                self.expect(&Token::Semicolon);
                Stmt::Assign(name, expr)
            }
            Token::LParen => {
                self.advance();
                let mut args = Vec::new();
                while !self.check(&Token::RParen) {
                    args.push(self.parse_expr());
                    if self.check(&Token::Comma) {
                        self.advance();
                    }
                }
                self.expect(&Token::RParen);
                self.expect(&Token::Semicolon);
                Stmt::Call(name, args)
            }
            other => fail_at(
                PARSE,
                after_ln,
                format!("expected '=' or '(' after '{}', got {:?}", name, other),
            ),
        }
    }

    // -- IF ---------------------------------------------------
    fn parse_if(&mut self) -> Stmt {
        self.advance(); // consume 'if'
        self.expect(&Token::LParen);
        let cond = self.parse_expr();
        self.expect(&Token::RParen);
        let body = self.parse_block();

        let mut elseifs = Vec::new();
        let mut else_body = None;

        while self.check(&Token::Elif) {
            self.advance();
            self.expect(&Token::LParen);
            let c = self.parse_expr();
            self.expect(&Token::RParen);
            let b = self.parse_block();
            elseifs.push(ElseIf {
                condition: c,
                body: b,
            });
        }
        if self.check(&Token::Else) {
            self.advance();
            else_body = Some(self.parse_block());
        }
        Stmt::If(cond, body, elseifs, else_body)
    }

    // -- WHILE ------------------------------------------------
    fn parse_while(&mut self) -> Stmt {
        self.advance(); // consume 'while'
        self.expect(&Token::LParen);
        let cond = self.parse_expr();
        self.expect(&Token::RParen);
        let body = self.parse_block();
        Stmt::While(cond, body)
    }

    // -- EXPRESSION (precedence tiers) ------------------------
    fn parse_expr(&mut self) -> Expr {
        self.parse_or()
    }
    // lowest precedence: or
    fn parse_or(&mut self) -> Expr {
        let mut left = self.parse_and();
        while self.check(&Token::Or) {
            self.advance();
            let right = self.parse_and();
            left = Expr::BinOp(Box::new(left), Op::Or, Box::new(right));
        }
        left
    }
    // and
    fn parse_and(&mut self) -> Expr {
        let mut left = self.parse_cmp();
        while self.check(&Token::And) {
            self.advance();
            let right = self.parse_cmp();
            left = Expr::BinOp(Box::new(left), Op::And, Box::new(right));
        }
        left
    }
    // comparison (== != < > <= >=)
    fn parse_cmp(&mut self) -> Expr {
        let mut left = self.parse_add();
        loop {
            let op = match self.peek() {
                Token::EqEq => Op::EqEq,
                Token::NotEq => Op::NotEq,
                Token::Lt => Op::Lt,
                Token::Gt => Op::Gt,
                Token::LtEq => Op::LtEq,
                Token::GtEq => Op::GtEq,
                _ => break,
            };
            self.advance();
            let right = self.parse_add();
            left = Expr::BinOp(Box::new(left), op, Box::new(right));
        }
        left
    }
    // additive (+ -)
    fn parse_add(&mut self) -> Expr {
        let mut left = self.parse_mul();
        loop {
            let op = match self.peek() {
                Token::Plus => Op::Add,
                Token::Minus => Op::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_mul();
            left = Expr::BinOp(Box::new(left), op, Box::new(right));
        }
        left
    }
    // multiplicative (* / %)
    fn parse_mul(&mut self) -> Expr {
        let mut left = self.parse_unary();
        loop {
            let op = match self.peek() {
                Token::Star => Op::Mul,
                Token::Slash => Op::Div,
                Token::Percent => Op::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary();
            left = Expr::BinOp(Box::new(left), op, Box::new(right));
        }
        left
    }
    // unary (not)
    fn parse_unary(&mut self) -> Expr {
        if self.check(&Token::Not) {
            self.advance();
            return Expr::Not(Box::new(self.parse_unary()));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Expr {
        let ln = self.line();
        match self.peek().clone() {
            Token::Int(n) => {
                self.advance();
                Expr::Int(n)
            }
            Token::Bool(b) => {
                self.advance();
                Expr::Bool(b)
            }
            Token::LParen => {
                self.advance();
                let e = self.parse_expr();
                self.expect(&Token::RParen);
                e
            }
            Token::Draw => {
                self.advance();
                self.expect(&Token::LParen);
                let x = self.parse_expr();
                self.expect(&Token::Comma);
                let y = self.parse_expr();
                self.expect(&Token::Comma);
                let sprite_ln = self.line();
                let sprite = match self.advance().clone() {
                    Token::Ident(n) => n,
                    other => fail_at(
                        PARSE,
                        sprite_ln,
                        format!("expected sprite name in draw(), got {:?}", other),
                    ),
                };
                self.expect(&Token::RParen);
                Expr::Draw(Box::new(x), Box::new(y), sprite)
            }
            Token::DrawDigit => {
                self.advance();
                self.expect(&Token::LParen);
                let x = self.parse_expr();
                self.expect(&Token::Comma);
                let y = self.parse_expr();
                self.expect(&Token::Comma);
                let n = self.parse_expr();
                self.expect(&Token::RParen);
                Expr::DrawDigit(Box::new(x), Box::new(y), Box::new(n))
            }
            Token::GetKey => {
                self.advance();
                self.expect(&Token::LParen);
                self.expect(&Token::RParen);
                Expr::GetKey
            }
            Token::GetDelay => {
                self.advance();
                self.expect(&Token::LParen);
                self.expect(&Token::RParen);
                Expr::GetDelay
            }
            Token::KeyPressed => {
                self.advance();
                self.expect(&Token::LParen);
                let e = self.parse_expr();
                self.expect(&Token::RParen);
                Expr::KeyPressed(Box::new(e))
            }
            Token::Rand => {
                self.advance();
                self.expect(&Token::LParen);
                let e = self.parse_expr();
                self.expect(&Token::RParen);
                Expr::Rand(Box::new(e))
            }
            Token::Ident(n) => {
                let n = n.clone();
                self.advance();
                if self.check(&Token::LParen) {
                    self.advance();
                    let mut args = Vec::new();
                    while !self.check(&Token::RParen) {
                        args.push(self.parse_expr());
                        if self.check(&Token::Comma) {
                            self.advance();
                        }
                    }
                    self.expect(&Token::RParen);
                    Expr::Call(n, args)
                } else {
                    Expr::Var(n)
                }
            }
            other => fail_at(
                PARSE,
                ln,
                format!("unexpected token in expression: {:?}", other),
            ),
        }
    }
}

// -- standalone helper used from main.rs ---------------------------------
/// Re-export so callers can use `parser::fail` without importing error directly.
pub fn fail_phase(msg: impl AsRef<str>) -> ! {
    fail(PARSE, msg)
}
