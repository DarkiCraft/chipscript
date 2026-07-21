#![allow(dead_code)]

use crate::ast::*;
use crate::error::{ANALYZE, fail};

use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq)]
enum Color {
    White,
    Gray,
    Black,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Bool,
}

pub struct Analyzer {
    vars: HashMap<String, Type>,
    functions: HashMap<String, (Vec<String>, String)>,
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

        // check name uniqueness across all three namespaces
        self.check_name_uniqueness(program);

        // build call graph for cycle + depth check
        self.check_call_graph(program);

        self.analyze_main(&program.main);
        for f in &program.functions {
            for arg in &f.args {
                self.vars.insert(arg.clone(), Type::Int);
            }
            self.vars.insert(f.ret.clone(), Type::Int);

            self.analyze_block(&f.body);

            // budget check: (globals + frame) + max temps ≤ 14
            let block_temps = self.block_temp_depth(&f.body);
            let total = self.vars.len() + block_temps;
            if total > 14 {
                fail(
                    ANALYZE,
                    format!(
                        "not enough registers for function '{}': {} global(s) + {} arg(s) + 1 return + {} temp(s) = {} (max 14)",
                        f.name,
                        program.vars.len(),
                        f.args.len(),
                        block_temps,
                        total,
                    ),
                );
            }

            for arg in &f.args {
                self.vars.remove(arg);
            }
            self.vars.remove(&f.ret);
        }

        // budget check for main
        let main_temps = self.block_temp_depth(&program.main);
        let total_main = self.vars.len() + main_temps;
        if total_main > 14 {
            fail(
                ANALYZE,
                format!(
                    "not enough registers: {} global(s) + {} temp(s) = {} (max 14)",
                    program.vars.len(),
                    main_temps,
                    total_main,
                ),
            );
        }
    }

    // -- SPRITES --------------------------------------------------
    fn collect_sprites(&mut self, program: &Program) {
        for sprite in &program.sprites {
            if let SpriteData::File(path) = &sprite.data {
                if !std::path::Path::new(path).exists() {
                    fail(ANALYZE, format!("sprite file not found: '{}'", path));
                }
            }
            let height = match &sprite.data {
                SpriteData::Inline(bytes) => bytes.len(),
                SpriteData::File(path) => std::fs::metadata(path)
                    .map(|m| m.len() as usize)
                    .unwrap_or(0),
            };
            if height == 0 {
                fail(
                    ANALYZE,
                    format!("sprite '{}' is empty (zero bytes)", sprite.name),
                );
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
        if program.vars.len() > 14 {
            fail(
                ANALYZE,
                format!(
                    "too many variables: declared {}, maximum is 14 (V0 reserved as scratch, VF is flag)",
                    program.vars.len()
                ),
            );
        }
        for var in &program.vars {
            // initializer must be a constant expression (literals + operators only)
            if !self.is_constant_expr(&var.value) {
                fail(
                    ANALYZE,
                    format!(
                        "variable '{}' initializer must be a constant expression (literals and operators only)",
                        var.name
                    ),
                );
            }
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

    // -- NAME UNIQUENESS ------------------------------------------
    fn check_name_uniqueness(&self, program: &Program) {
        let mut names: HashSet<&str> = HashSet::new();

        for var in &program.vars {
            if !names.insert(&var.name) {
                fail(
                    ANALYZE,
                    format!("duplicate name '{}' (multiple declarations)", var.name),
                );
            }
        }

        for f in &program.functions {
            if !names.insert(&f.name) {
                fail(
                    ANALYZE,
                    format!(
                        "name collision: '{}' is both a variable and a function",
                        f.name
                    ),
                );
            }
        }

        for s in &program.sprites {
            if !names.insert(&s.name) {
                fail(
                    ANALYZE,
                    format!(
                        "name collision: '{}' is both a variable/function and a sprite",
                        s.name
                    ),
                );
            }
        }
    }

    // -- CALL GRAPH -----------------------------------------------
    fn check_call_graph(&self, program: &Program) {
        let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
        for f in &program.functions {
            let callees = self.collect_calls(&f.body);
            graph.insert(&f.name, callees);
        }
        // main also calls functions
        let main_callees = self.collect_calls(&program.main);
        graph.insert("main", main_callees);

        // detect cycles (no recursion)
        if let Some(cycle) = self.find_cycle(&graph) {
            let path = cycle.join(" → ");
            fail(
                ANALYZE,
                format!(
                    "recursive call detected: {} (CHIP-8 does not support recursion)",
                    path
                ),
            );
        }

        // compute max call depth from main (≤ 16 hardware limit)
        let max_depth = self.max_call_depth(&graph, "main", &mut HashMap::new());
        if max_depth > 16 {
            fail(
                ANALYZE,
                format!(
                    "call chain too deep: {} nested calls (CHIP-8 stack limit is 16)",
                    max_depth
                ),
            );
        }
    }

    fn collect_calls<'a>(&self, stmts: &'a [Stmt]) -> Vec<&'a str> {
        let mut calls = Vec::new();
        self.collect_calls_stmts(stmts, &mut calls);
        calls
    }

    fn collect_calls_stmts<'a>(&self, stmts: &'a [Stmt], acc: &mut Vec<&'a str>) {
        for stmt in stmts {
            self.collect_calls_stmt(stmt, acc);
        }
    }

    fn collect_calls_stmt<'a>(&self, stmt: &'a Stmt, acc: &mut Vec<&'a str>) {
        match stmt {
            Stmt::Assign(_, expr) => self.collect_calls_expr(expr, acc),
            Stmt::If(cond, body, elseifs, else_body) => {
                self.collect_calls_expr(cond, acc);
                self.collect_calls_stmts(body, acc);
                for elif in elseifs {
                    self.collect_calls_expr(&elif.condition, acc);
                    self.collect_calls_stmts(&elif.body, acc);
                }
                if let Some(b) = else_body {
                    self.collect_calls_stmts(b, acc);
                }
            }
            Stmt::Loop(body) | Stmt::While(_, body) => self.collect_calls_stmts(body, acc),
            Stmt::Call(name, args) => {
                acc.push(name.as_str());
                for a in args {
                    self.collect_calls_expr(a, acc);
                }
            }
            Stmt::Clear => {}
            Stmt::Delay(e) | Stmt::Beep(e) => self.collect_calls_expr(e, acc),
        }
    }

    fn collect_calls_expr<'a>(&self, expr: &'a Expr, acc: &mut Vec<&'a str>) {
        match expr {
            Expr::Int(_) | Expr::Bool(_) | Expr::Var(_) => {}
            Expr::Not(e) => self.collect_calls_expr(e, acc),
            Expr::BinOp(l, _, r) => {
                self.collect_calls_expr(l, acc);
                self.collect_calls_expr(r, acc);
            }
            Expr::Call(name, args) => {
                acc.push(name.as_str());
                for a in args {
                    self.collect_calls_expr(a, acc);
                }
            }
            Expr::Draw(x, y, _) => {
                self.collect_calls_expr(x, acc);
                self.collect_calls_expr(y, acc);
            }
            Expr::DrawDigit(x, y, n) => {
                self.collect_calls_expr(x, acc);
                self.collect_calls_expr(y, acc);
                self.collect_calls_expr(n, acc);
            }
            Expr::KeyPressed(k) => self.collect_calls_expr(k, acc),
            Expr::GetKey | Expr::GetDelay => {}
            Expr::Rand(m) => self.collect_calls_expr(m, acc),
        }
    }

    fn find_cycle<'a>(&self, graph: &HashMap<&'a str, Vec<&'a str>>) -> Option<Vec<String>> {
        let mut color: HashMap<&str, Color> = HashMap::new();
        let mut path: Vec<String> = Vec::new();

        for node in graph.keys() {
            color.entry(*node).or_insert(Color::White);
        }

        for node in graph.keys() {
            let c = color.get(node).copied().unwrap_or(Color::White);
            if c == Color::White {
                if let Some(cycle) = self.dfs_cycle(node, graph, &mut color, &mut path) {
                    let start = path.iter().position(|n| *n == cycle).unwrap();
                    let cycle_path: Vec<String> = path[start..].to_vec();
                    let mut extended = cycle_path;
                    extended.push(cycle);
                    return Some(extended);
                }
            }
        }
        None
    }

    fn dfs_cycle<'a>(
        &self,
        node: &'a str,
        graph: &HashMap<&'a str, Vec<&'a str>>,
        color: &mut HashMap<&'a str, Color>,
        path: &mut Vec<String>,
    ) -> Option<String> {
        color.insert(node, Color::Gray);
        path.push(node.to_string());

        if let Some(callees) = graph.get(node) {
            for callee in callees {
                match color.get(*callee).copied() {
                    Some(Color::Gray) => {
                        return Some(callee.to_string());
                    }
                    Some(Color::White) => {
                        if let Some(cycle) = self.dfs_cycle(callee, graph, color, path) {
                            return Some(cycle);
                        }
                    }
                    _ => {}
                }
            }
        }

        color.insert(node, Color::Black);
        path.pop();
        None
    }

    fn max_call_depth<'a>(
        &self,
        graph: &HashMap<&'a str, Vec<&'a str>>,
        node: &'a str,
        memo: &mut HashMap<&'a str, usize>,
    ) -> usize {
        if let Some(&d) = memo.get(node) {
            return d;
        }
        let mut max_d = 0;
        if let Some(callees) = graph.get(node) {
            for callee in callees {
                let d = self.max_call_depth(graph, callee, memo);
                if d >= max_d {
                    max_d = d;
                }
            }
        }
        let depth = if node == "main" { max_d } else { 1 + max_d };
        memo.insert(node, depth);
        depth
    }

    // -- MAIN -----------------------------------------------------
    fn analyze_main(&mut self, stmts: &[Stmt]) {
        self.analyze_block(stmts);
    }

    // -- BLOCK ----------------------------------------------------
    fn analyze_block(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.analyze_stmt(stmt);
        }
    }

    // -- STATEMENT ------------------------------------------------
    fn analyze_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Assign(name, expr) => {
                let var_type =
                    self.vars.get(name).cloned().unwrap_or_else(|| {
                        fail(ANALYZE, format!("undeclared variable: '{}'", name))
                    });
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
                        fail(
                            ANALYZE,
                            format!("'elif' condition must be bool, got {:?}", t),
                        );
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
                    fail(
                        ANALYZE,
                        format!("'while' condition must be bool, got {:?}", t),
                    );
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
                    fail(
                        ANALYZE,
                        format!("delay() argument must be int, got {:?}", t),
                    );
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
    fn check_fn_call(&self, name: &str, args: &[Expr]) {
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
        for (i, arg) in args.iter().enumerate() {
            let t = self.type_of_expr(arg);
            if t != Type::Int {
                fail(
                    ANALYZE,
                    format!("argument {} of '{}' must be int, got {:?}", i + 1, name, t),
                );
            }
        }
    }

    // -- SYMBOL TABLE DUMP ----------------------------------------
    pub fn dump_symtable(&self) {
        println!("{:<20} {:<10} {}", "NAME", "TYPE", "SCOPE");
        println!("{}", "-".repeat(40));

        for (name, ty) in &self.vars {
            let ty_str = match ty {
                Type::Int => "int",
                Type::Bool => "bool",
            };
            println!("{:<20} {:<10} global", name, ty_str);
        }

        for (name, (args, ret)) in &self.functions {
            println!(
                "{:<20} {:<10} function  args={} ret={}",
                name,
                "fn",
                args.len(),
                ret
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
            Expr::Int(_) => Type::Int,
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
                    Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod => {
                        if lt != Type::Int {
                            fail(
                                ANALYZE,
                                "arithmetic operator requires int operands (left side is bool)",
                            );
                        }
                        if rt != Type::Int {
                            fail(
                                ANALYZE,
                                "arithmetic operator requires int operands (right side is bool)",
                            );
                        }
                        Type::Int
                    }
                    Op::EqEq | Op::NotEq => {
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
                    Op::Lt | Op::Gt | Op::LtEq | Op::GtEq => {
                        if lt != Type::Int || rt != Type::Int {
                            fail(
                                ANALYZE,
                                "ordered comparison (< > <= >=) requires int operands",
                            );
                        }
                        Type::Bool
                    }
                    Op::And | Op::Or => {
                        if lt != Type::Bool {
                            fail(
                                ANALYZE,
                                "'and'/'or' requires bool operands (left side is int)",
                            );
                        }
                        if rt != Type::Bool {
                            fail(
                                ANALYZE,
                                "'and'/'or' requires bool operands (right side is int)",
                            );
                        }
                        Type::Bool
                    }
                }
            }
            Expr::Not(e) => {
                let t = self.type_of_expr(e);
                if t != Type::Bool {
                    fail(
                        ANALYZE,
                        format!("'not' requires a bool operand, got {:?}", t),
                    );
                }
                Type::Bool
            }
            Expr::Call(name, args) => {
                self.check_fn_call(name, args);
                Type::Int
            }
            Expr::Draw(x, _, sprite) => {
                let tx = self.type_of_expr(x);
                if tx != Type::Int {
                    fail(ANALYZE, "draw() x coordinate must be int");
                }
                if !self.sprites.contains(sprite) {
                    fail(
                        ANALYZE,
                        format!("draw() references undeclared sprite: '{}'", sprite),
                    );
                }
                Type::Bool
            }
            Expr::DrawDigit(x, y, n) => {
                let tx = self.type_of_expr(x);
                let ty = self.type_of_expr(y);
                let tn = self.type_of_expr(n);
                if tx != Type::Int {
                    fail(ANALYZE, "drawdigit() x coordinate must be int");
                }
                if ty != Type::Int {
                    fail(ANALYZE, "drawdigit() y coordinate must be int");
                }
                if tn != Type::Int {
                    fail(
                        ANALYZE,
                        format!("drawdigit() third argument must be int, got {:?}", tn),
                    );
                }
                Type::Bool
            }
            Expr::GetKey => Type::Int,
            Expr::GetDelay => Type::Int,
            Expr::KeyPressed(k) => {
                let t = self.type_of_expr(k);
                if t != Type::Int {
                    fail(
                        ANALYZE,
                        format!("keypressed() argument must be int, got {:?}", t),
                    );
                }
                Type::Bool
            }
            Expr::Rand(mask) => {
                if let Expr::Int(n) = mask.as_ref() {
                    if *n == 0 {
                        // rand(0) is degenerate but allowed — mask AND 0 = always 0
                    }
                } else {
                    fail(
                        ANALYZE,
                        "rand() mask must be an integer literal (e.g. rand(0xFF)), not a variable",
                    );
                }
                Type::Int
            }
        }
    }

    // -- CONSTANT EXPRESSION CHECK --------------------------------
    fn is_constant_expr(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Int(_) | Expr::Bool(_) => true,
            Expr::Not(e) => self.is_constant_expr(e),
            Expr::BinOp(l, _, r) => self.is_constant_expr(l) && self.is_constant_expr(r),
            _ => false, // Var, Call, builtins — not allowed
        }
    }

    // -- TEMP DEPTH COMPUTATION -----------------------------------
    fn expr_temp_depth(&self, expr: &Expr) -> usize {
        match expr {
            Expr::Int(_) | Expr::Bool(_) | Expr::Var(_) => 0,
            Expr::Not(e) => self.expr_temp_depth(e), // uses V0 scratch
            Expr::BinOp(l, _, r) => {
                // alloc lr, rr: max(1+d(left), 2+d(right))
                let ld = self.expr_temp_depth(l);
                let rd = self.expr_temp_depth(r);
                std::cmp::max(1 + ld, 2 + rd)
            }
            Expr::Call(_, args) => {
                let argc = args.len();
                let ad = args
                    .iter()
                    .map(|a| self.expr_temp_depth(a))
                    .max()
                    .unwrap_or(0);
                argc + 1 + ad
            }
            Expr::Draw(x, y, _) => {
                std::cmp::max(1 + self.expr_temp_depth(x), 2 + self.expr_temp_depth(y))
            }
            Expr::DrawDigit(x, y, n) => {
                let d = [
                    1 + self.expr_temp_depth(x),
                    2 + self.expr_temp_depth(y),
                    3 + self.expr_temp_depth(n),
                ];
                *d.iter().max().unwrap()
            }
            Expr::GetKey | Expr::GetDelay => 0,
            Expr::KeyPressed(k) => 1 + self.expr_temp_depth(k),
            Expr::Rand(m) => self.expr_temp_depth(m),
        }
    }

    fn stmt_temp_depth(&self, stmt: &Stmt) -> usize {
        match stmt {
            Stmt::Assign(_, expr) => self.expr_temp_depth(expr),
            Stmt::If(cond, body, elseifs, else_body) => {
                let mut m = self.expr_temp_depth(cond);
                m = m.max(self.block_temp_depth(body));
                for elif in elseifs {
                    m = m.max(self.expr_temp_depth(&elif.condition));
                    m = m.max(self.block_temp_depth(&elif.body));
                }
                if let Some(b) = else_body {
                    m = m.max(self.block_temp_depth(b));
                }
                m
            }
            Stmt::Loop(body) => self.block_temp_depth(body),
            Stmt::While(cond, body) => {
                std::cmp::max(self.expr_temp_depth(cond), self.block_temp_depth(body))
            }
            Stmt::Call(_, args) => {
                let argc = args.len();
                let ad = args
                    .iter()
                    .map(|a| self.expr_temp_depth(a))
                    .max()
                    .unwrap_or(0);
                argc + 1 + ad
            }
            Stmt::Clear => 0,
            Stmt::Delay(e) | Stmt::Beep(e) => self.expr_temp_depth(e),
        }
    }

    fn block_temp_depth(&self, stmts: &[Stmt]) -> usize {
        stmts
            .iter()
            .map(|s| self.stmt_temp_depth(s))
            .max()
            .unwrap_or(0)
    }
}
