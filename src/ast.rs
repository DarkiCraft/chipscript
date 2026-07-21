#![allow(dead_code)]

// a program is the entire file
#[derive(Debug, Clone)]
pub struct Program {
    pub sprites: Vec<SpriteDecl>,
    pub vars: Vec<VarDecl>,
    pub functions: Vec<FnDecl>,
    pub main: Vec<Stmt>,
}

// sprite digit = [0xF0, 0x90];
// sprite ship = "ship.spr";
#[derive(Debug, Clone)]
pub struct SpriteDecl {
    pub name: String,
    pub data: SpriteData,
}

#[derive(Debug, Clone)]
pub enum SpriteData {
    Inline(Vec<u8>), // [0xF0, 0x90, ...]
    File(String),    // "ship.spr"
}

// one line inside vars {}
#[derive(Debug, Clone)]
pub struct VarDecl {
    pub name: String,
    pub value: Expr, // the initial value
}

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: String,
    pub args: Vec<String>,
    pub ret: String,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Assign(String, Expr),                                // a = expr
    If(Expr, Vec<Stmt>, Vec<ElseIf>, Option<Vec<Stmt>>), // if / elif / else
    Loop(Vec<Stmt>),                                     // loop {}
    While(Expr, Vec<Stmt>),                              // while (cond) { }
    Call(String, Vec<Expr>), // standalone function call (return value discarded)

    Clear,       // clear()
    Delay(Expr), // delay(n)
    Beep(Expr),  // beep(n)
}

// the elif branches
#[derive(Debug, Clone)]
pub struct ElseIf {
    pub condition: Expr,
    pub body: Vec<Stmt>,
}

// an expression produces a value
#[derive(Debug, Clone)]
pub enum Expr {
    Int(u8),                         // 10 (unsigned byte 0–255)
    Bool(bool),                      // true / false
    Var(String),                     // a
    BinOp(Box<Expr>, Op, Box<Expr>), // a + b, a == b
    Not(Box<Expr>),                  // not x

    Call(String, Vec<Expr>),                    // add(a, b)
    Draw(Box<Expr>, Box<Expr>, String),         // draw(x, y, sprite) -> bool
    DrawDigit(Box<Expr>, Box<Expr>, Box<Expr>), // drawdigit(x, y, n) -> bool
    GetKey,                                     // getkey() -> int
    GetDelay,                                   // getdelay() -> int
    KeyPressed(Box<Expr>),                      // keypressed(5) -> bool
    Rand(Box<Expr>),                            // rand(0xFF) -> int
}

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Mod, // arithmetic
    EqEq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq, // comparison
    And,
    Or, // logic
}
