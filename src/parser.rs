#![allow(dead_code)]

use crate::ast::*;
use crate::error::*;
use crate::lexer::Token;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    // look at current token without consuming
    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    // consume and return current token
    fn advance(&mut self) -> &Token {
        let t = &self.tokens[self.pos];
        self.pos += 1;
        t
    }

    // consume current token ONLY if it matches, else panic
    fn expect(&mut self, expected: &Token) -> &Token {
        let t = &self.tokens[self.pos];
        if t != expected {
            fail(format!("expected {:?} but got {:?}", expected, t));
        }
        self.pos += 1;
        &self.tokens[self.pos - 1]
    }

    // check current token without consuming
    fn check(&self, expected: &Token) -> bool {
        self.peek() == expected
    }

    // -- ENTRY POINT ------------------------------------------
    pub fn parse(&mut self) -> Program {
        let mut sprites = Vec::new();
        let mut vars = Vec::new();
        let mut functions = Vec::new();
        let mut main = Vec::new();

        while !self.check(&Token::EOF) {
            match self.peek().clone() {
                Token::Sprites => sprites = self.parse_sprites_block(),
                Token::Vars => vars = self.parse_vars(),
                Token::Fn => functions.push(self.parse_fn()),
                Token::Main => main = self.parse_main(),
                other => fail(format!("unexpected token at top level: {:?}", other)),
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
        self.advance(); // consume 'sprite'
        self.expect(&Token::LBrace);
        let mut sprites = Vec::new();
        while !self.check(&Token::RBrace) {
            let name = match self.advance().clone() {
                Token::Ident(n) => n,
                other => panic!("expected sprite name, got {:?}", other),
            };
            self.expect(&Token::Equals);
            let data = match self.peek().clone() {
                Token::LBracket => {
                    self.advance();
                    let mut bytes = Vec::new();
                    while !self.check(&Token::RBracket) {
                        match self.advance().clone() {
                            Token::Int(b) => bytes.push(b as u8),
                            other => panic!("expected byte in sprite, got {:?}", other),
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
                other => panic!("expected sprite data, got {:?}", other),
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
            let name = match self.advance().clone() {
                Token::Ident(n) => n,
                other => fail(format!("expected variable name, got {:?}", other)),
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
        let name = match self.advance().clone() {
            Token::Ident(n) => n,
            other => fail(format!("expected function name, got {:?}", other)),
        };
        self.expect(&Token::LParen);
        let mut args = Vec::new();
        while !self.check(&Token::RParen) {
            match self.advance().clone() {
                Token::Ident(n) => args.push(n),
                other => fail(format!("expected arg name, got {:?}", other)),
            }
            if self.check(&Token::Comma) {
                self.advance();
            }
        }
        self.expect(&Token::RParen);
        self.expect(&Token::Arrow);
        let ret = match self.advance().clone() {
            Token::Ident(n) => n,
            other => fail(format!("expected return name, got {:?}", other)),
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
            other => fail(format!("unexpected token in statement: {:?}", other)),
        }
    }

    // -- ASSIGN or CALL ---------------------------------------
    fn parse_assign_or_call(&mut self) -> Stmt {
        let name = match self.advance().clone() {
            Token::Ident(n) => n,
            other => fail(format!("expected ident, got {:?}", other)),
        };
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
            other => fail(format!("expected = or ( after ident, got {:?}", other)),
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

    // -- EXPRESSION -------------------------------------------
    fn parse_expr(&mut self) -> Expr {
        self.parse_binop()
    }

    fn parse_binop(&mut self) -> Expr {
        let mut left = self.parse_unary();
        loop {
            let op = match self.peek() {
                Token::Plus => Op::Add,
                Token::Minus => Op::Sub,
                Token::Star => Op::Mul,
                Token::Slash => Op::Div,
                Token::Percent => Op::Mod,
                Token::EqEq => Op::EqEq,
                Token::NotEq => Op::NotEq,
                Token::Lt => Op::Lt,
                Token::Gt => Op::Gt,
                Token::LtEq => Op::LtEq,
                Token::GtEq => Op::GtEq,
                Token::And => Op::And,
                Token::Or => Op::Or,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary();
            left = Expr::BinOp(Box::new(left), op, Box::new(right));
        }
        left
    }

    fn parse_unary(&mut self) -> Expr {
        if self.check(&Token::Not) {
            self.advance();
            return Expr::Not(Box::new(self.parse_unary()));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Expr {
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
            Token::Not => {
                self.advance();
                Expr::Not(Box::new(self.parse_primary()))
            }
            Token::Draw => {
                self.advance();
                self.expect(&Token::LParen);
                let x = self.parse_expr();
                self.expect(&Token::Comma);
                let y = self.parse_expr();
                self.expect(&Token::Comma);
                let sprite = match self.advance().clone() {
                    Token::Ident(n) => n,
                    other => fail(format!("expected sprite name in draw(), got {:?}", other)),
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
            other => fail(format!("unexpected token in expression: {:?}", other)),
        }
    }
}
