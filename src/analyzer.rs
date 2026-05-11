#![allow(dead_code)]

use crate::ast::*;
use crate::error::{fail, ANALYZE};

use std::collections::HashMap;

// the type of a variable, bool or int
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Bool,
}

pub struct Analyzer {
    // variable name -> type
    vars: HashMap<String, Type>,
    // function name -> (args, return var name)
    functions: HashMap<String, (Vec<String>, String)>,
    // sprite names
    sprites: Vec<String>,
}

impl Analyzer {
    pub fn new() -> Self {
        Analyzer {
            vars: HashMap::new(),
            functions: HashMap::new(),
            sprites: Vec::new(),
        }
    }

    pub fn analyze(&mut self, program: &Program) {
        self.collect_sprites(program);
        self.collect_vars(program);
        self.collect_functions(program);
        self.analyze_main(&program.main);
        for f in &program.functions {
            // temporarily insert args and return var so the body can reference them
            for arg in &f.args {
                self.vars.insert(arg.clone(), Type::Int);
            }
            self.vars.insert(f.ret.clone(), Type::Int);

            self.analyze_block(&f.body);

            // remove them again so they don't leak into other functions or main
            for arg in &f.args {
                self.vars.remove(arg);
            }
            self.vars.remove(&f.ret);
        }
    }

    // -- SPRITES --------------------------------------------------
    fn collect_sprites(&mut self, program: &Program) {
        for sprite in &program.sprites {
            // check file exists on disk
            if let SpriteData::File(path) = &sprite.data {
                if !std::path::Path::new(path).exists() {
                    fail(ANALYZE, format!("sprite file not found: '{}'", path));
                }
            }
            // validate height (CHIP-8 limit: max 15 rows)
            let height = match &sprite.data {
                SpriteData::Inline(bytes) => bytes.len(),
                SpriteData::File(path) => std::fs::metadata(path)
                    .map(|m| m.len() as usize)
                    .unwrap_or(0),
            };
            if height == 0 {
                fail(ANALYZE, format!("sprite '{}' is empty (zero bytes)", sprite.name));
            }
            if height > 15 {
                fail(
                    ANALYZE,
                    format!(
                        "sprite '{}' is {} rows tall — CHIP-8 maximum is 15",
                        sprite.name, height
                    ),
                );
            }
            self.sprites.push(sprite.name.clone());
        }
    }

    // -- VARS -----------------------------------------------------
    fn collect_vars(&mut self, program: &Program) {
        if program.vars.len() > 15 {
            fail(
                ANALYZE,
                format!(
                    "too many variables: declared {}, maximum is 15 (one CHIP-8 register each)",
                    program.vars.len()
                ),
            );
        }
        for var in &program.vars {
            let t = self.type_of_expr(&var.value);
            self.vars.insert(var.name.clone(), t);
        }
    }

    // -- FUNCTIONS ------------------------------------------------
    fn collect_functions(&mut self, program: &Program) {
        for f in &program.functions {
            self.functions
                .insert(f.name.clone(), (f.args.clone(), f.ret.clone()));
        }
    }

    // -- MAIN -----------------------------------------------------
    fn analyze_main(&mut self, stmts: &Vec<Stmt>) {
        self.analyze_block(stmts);
    }

    // -- BLOCK ----------------------------------------------------
    fn analyze_block(&mut self, stmts: &Vec<Stmt>) {
        for stmt in stmts {
            self.analyze_stmt(stmt);
        }
    }

    // -- STATEMENT ------------------------------------------------
    fn analyze_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Assign(name, expr) => {
                let var_type = self
                    .vars
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| fail(ANALYZE, format!("undeclared variable: '{}'", name)));
                let expr_type = self.type_of_expr(expr);
                if var_type != expr_type {
                    fail(
                        ANALYZE,
                        format!(
                            "type mismatch: '{}' is {:?} but the assigned expression is {:?}",
                            name, var_type, expr_type
                        ),
                    );
                }
            }
            Stmt::If(cond, body, elseifs, else_body) => {
                let t = self.type_of_expr(cond);
                if t != Type::Bool {
                    fail(ANALYZE, format!("'if' condition must be bool, got {:?}", t));
                }
                self.analyze_block(body);
                for elif in elseifs {
                    let t = self.type_of_expr(&elif.condition);
                    if t != Type::Bool {
                        fail(ANALYZE, format!("'elif' condition must be bool, got {:?}", t));
                    }
                    self.analyze_block(&elif.body);
                }
                if let Some(b) = else_body {
                    self.analyze_block(b);
                }
            }
            Stmt::Loop(body) => {
                self.analyze_block(body);
            }
            Stmt::While(cond, body) => {
                let t = self.type_of_expr(cond);
                if t != Type::Bool {
                    fail(ANALYZE, format!("'while' condition must be bool, got {:?}", t));
                }
                self.analyze_block(body);
            }
            Stmt::Call(name, args) => {
                self.check_fn_call(name, args);
            }
            Stmt::Clear => {}
            Stmt::Delay(e) => {
                let t = self.type_of_expr(e);
                if t != Type::Int {
                    fail(ANALYZE, format!("delay() argument must be int, got {:?}", t));
                }
            }
            Stmt::Beep(e) => {
                let t = self.type_of_expr(e);
                if t != Type::Int {
                    fail(ANALYZE, format!("beep() argument must be int, got {:?}", t));
                }
            }
        }
    }

    // -- FUNCTION CALL CHECK --------------------------------------
    fn check_fn_call(&self, name: &str, args: &Vec<Expr>) {
        let (params, _) = self
            .functions
            .get(name)
            .unwrap_or_else(|| fail(ANALYZE, format!("call to undeclared function: '{}'", name)));
        if args.len() != params.len() {
            fail(
                ANALYZE,
                format!(
                    "function '{}' expects {} argument(s), got {}",
                    name,
                    params.len(),
                    args.len()
                ),
            );
        }
        // check register budget: vars + args + return slot must be <= 15
        let slots_needed = args.len() + 1; // args + return value
        let slots_used   = self.vars.len();
        if slots_used + slots_needed > 15 {
            fail(
                ANALYZE,
                format!(
                    "not enough registers to call '{}': {} global(s) + {} arg(s) + 1 return = {} > 15",
                    name,
                    slots_used,
                    args.len(),
                    slots_used + slots_needed
                ),
            );
        }
    }

    // -- SYMBOL TABLE DUMP ----------------------------------------
    pub fn dump_symtable(&self) {
        println!("{:<20} {:<10} {}", "NAME", "TYPE", "SCOPE");
        println!("{}", "-".repeat(40));

        for (name, ty) in &self.vars {
            let ty_str = match ty {
                Type::Int  => "int",
                Type::Bool => "bool",
            };
            println!("{:<20} {:<10} global", name, ty_str);
        }

        for (name, (args, ret)) in &self.functions {
            println!(
                "{:<20} {:<10} function  args={} ret={}",
                name, "fn", args.len(), ret
            );
        }

        for name in &self.sprites {
            println!("{:<20} {:<10} sprite", name, "sprite");
        }

        println!("{}", "-".repeat(40));
        println!(
            "{} var(s), {} function(s), {} sprite(s)",
            self.vars.len(),
            self.functions.len(),
            self.sprites.len()
        );
    }

    // -- TYPE INFERENCE -------------------------------------------
    fn type_of_expr(&self, expr: &Expr) -> Type {
        match expr {
            Expr::Int(_)  => Type::Int,
            Expr::Bool(_) => Type::Bool,
            Expr::Var(name) => self
                .vars
                .get(name)
                .cloned()
                .unwrap_or_else(|| fail(ANALYZE, format!("undeclared variable: '{}'", name))),
            Expr::BinOp(left, op, right) => {
                let lt = self.type_of_expr(left);
                let rt = self.type_of_expr(right);
                match op {
                    // arithmetic — both must be int, result is int
                    Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod => {
                        if lt != Type::Int {
                            fail(ANALYZE, "arithmetic operator requires int operands (left side is bool)");
                        }
                        if rt != Type::Int {
                            fail(ANALYZE, "arithmetic operator requires int operands (right side is bool)");
                        }
                        Type::Int
                    }
                    // comparison — both must be same type, result is bool
                    Op::EqEq | Op::NotEq | Op::Lt | Op::Gt | Op::LtEq | Op::GtEq => {
                        if lt != rt {
                            fail(
                                ANALYZE,
                                format!(
                                    "comparison between different types: {:?} and {:?}",
                                    lt, rt
                                ),
                            );
                        }
                        Type::Bool
                    }
                    // logic — both must be bool, result is bool
                    Op::And | Op::Or => {
                        if lt != Type::Bool {
                            fail(ANALYZE, "'and'/'or' requires bool operands (left side is int)");
                        }
                        if rt != Type::Bool {
                            fail(ANALYZE, "'and'/'or' requires bool operands (right side is int)");
                        }
                        Type::Bool
                    }
                }
            }
            Expr::Not(e) => {
                let t = self.type_of_expr(e);
                if t != Type::Bool {
                    fail(ANALYZE, format!("'not' requires a bool operand, got {:?}", t));
                }
                Type::Bool
            }
            Expr::Call(name, args) => {
                self.check_fn_call(name, args);
                Type::Int
            }
            Expr::Draw(_, _, sprite) => {
                if !self.sprites.contains(sprite) {
                    fail(ANALYZE, format!("draw() references undeclared sprite: '{}'", sprite));
                }
                Type::Bool
            }
            Expr::DrawDigit(_, _, n) => {
                let t = self.type_of_expr(n);
                if t != Type::Int {
                    fail(ANALYZE, format!("drawdigit() third argument must be int, got {:?}", t));
                }
                Type::Bool
            }
            Expr::GetKey    => Type::Int,
            Expr::GetDelay  => Type::Int,
            Expr::KeyPressed(_) => Type::Bool,
            Expr::Rand(mask) => {
                if !matches!(mask.as_ref(), Expr::Int(_)) {
                    fail(
                        ANALYZE,
                        "rand() mask must be an integer literal (e.g. rand(0xFF)), not a variable",
                    );
                }
                Type::Int
            }
        }
    }
}
