#![allow(dead_code)]

use crate::ast::*;

// ---------------------------------------------------------------------------
// Optimizer
//
// Three passes over the AST, run repeatedly until the program stops changing
// (fixed-point iteration), so that each pass can expose new opportunities for
// the others.
//
// Pass 1 – Constant Folding
//   Evaluates expressions whose operands are all compile-time literals.
//   Examples:
//     3 + 4          → 7
//     10 * 0         → 0
//     true and false → false
//     not true       → false
//     5 == 5         → true
//
// Pass 2 – Dead Code Elimination
//   Removes code that can never execute, determined by constant conditions.
//   Examples:
//     if (true)  { A } else { B }  → A
//     if (false) { A } else { B }  → B
//     if (false) { A }             → (nothing)
//     while (false) { ... }        → (nothing)
//   elif chains are pruned the same way: a branch whose condition is
//   a constant false is removed; a branch whose condition is a constant
//   true becomes the final else and all subsequent branches are dropped.
//
// Pass 3 – Strength Reduction
//   Replaces expensive operations with cheaper equivalents when one operand
//   is a known constant.  On CHIP-8, multiply and divide are software loops,
//   so reducing them matters.
//   Examples:
//     x + 0   → x           x - 0  → x
//     x * 0   → 0           x * 1  → x
//     x / 1   → x           x % 1  → 0
//     x * 2   → x + x       (avoids the multiply loop entirely)
//     x and true  → x       x and false → false
//     x or  false → x       x or  true  → true
//     x == true   → x       x == false  → not x
//     x != true   → not x   x != false  → x
//
// What is deliberately NOT attempted:
//   Copy Propagation – all variables are mutable globals; a copy `a = b` is
//   invalidated by any subsequent statement (function calls, draw, keypressed
//   all have side effects that may touch any register).  Safe propagation
//   windows would be so small as to be useless.
//
//   Common Subexpression Elimination – same reason: no expression involving a
//   variable or a builtin can be safely hoisted or deduplicated because any
//   intervening statement may change the value.  Literal-only CSE is already
//   handled by constant folding.
// ---------------------------------------------------------------------------

pub struct Optimizer {
    pub folds:      usize,  // constant folds performed
    pub dce:        usize,  // dead branches / statements eliminated
    pub reductions: usize,  // strength reductions performed
}

impl Optimizer {
    pub fn new() -> Self {
        Optimizer { folds: 0, dce: 0, reductions: 0 }
    }

    // Run all passes to a fixed point (until nothing changes).
    pub fn optimize(&mut self, program: &mut Program) {
        loop {
            let before = (self.folds, self.dce, self.reductions);
            self.optimize_program(program);
            if (self.folds, self.dce, self.reductions) == before {
                break;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Program
    // -----------------------------------------------------------------------
    fn optimize_program(&mut self, program: &mut Program) {
        // Var initialisers
        for var in &mut program.vars {
            var.value = self.fold_expr(var.value.clone());
        }
        // Functions
        for f in &mut program.functions {
            self.optimize_block(&mut f.body);
        }
        // Main
        self.optimize_block(&mut program.main);
    }

    // -----------------------------------------------------------------------
    // Block
    // -----------------------------------------------------------------------
    fn optimize_block(&mut self, stmts: &mut Vec<Stmt>) {
        // First pass: optimize each statement in place.
        for stmt in stmts.iter_mut() {
            self.optimize_stmt(stmt);
        }

        // Second pass: DCE — remove statements that are provably no-ops.
        // We collect the surviving statements into a new vec.
        let old = std::mem::take(stmts);
        for stmt in old {
            match self.dce_stmt(stmt) {
                Some(s) => stmts.push(s),
                None    => { self.dce += 1; },
            }
        }
    }

    // -----------------------------------------------------------------------
    // Statement — in-place optimisation (expressions + sub-blocks)
    // -----------------------------------------------------------------------
    fn optimize_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Assign(_, expr) => {
                *expr = self.fold_expr(expr.clone());
            },
            Stmt::If(cond, body, elseifs, else_body) => {
                *cond = self.fold_expr(cond.clone());
                self.optimize_block(body);
                for elif in elseifs.iter_mut() {
                    elif.condition = self.fold_expr(elif.condition.clone());
                    self.optimize_block(&mut elif.body);
                }
                if let Some(b) = else_body {
                    self.optimize_block(b);
                }
            },
            Stmt::Loop(body) => {
                self.optimize_block(body);
            },
            Stmt::While(cond, body) => {
                *cond = self.fold_expr(cond.clone());
                self.optimize_block(body);
            },
            Stmt::Call(_, args) => {
                for arg in args.iter_mut() {
                    *arg = self.fold_expr(arg.clone());
                }
            },
            Stmt::Delay(e) | Stmt::Beep(e) => {
                *e = self.fold_expr(e.clone());
            },
            Stmt::Clear => {},
        }
    }

    // -----------------------------------------------------------------------
    // Dead Code Elimination — returns None if the statement should be dropped
    // -----------------------------------------------------------------------
    fn dce_stmt(&mut self, stmt: Stmt) -> Option<Stmt> {
        match stmt {
            // if (false) { ... }  → drop entirely (even with elif/else)
            // if (true)  { A } … → keep only A, discard elif/else
            Stmt::If(cond, body, elseifs, else_body) => {
                // Prune elif branches that can never or always execute.
                let (new_elseifs, promoted_else) =
                    self.dce_elif_chain(elseifs, else_body);

                match &cond {
                    Expr::Bool(true) => {
                        // The if-body always runs; elif/else are unreachable.
                        self.dce += new_elseifs.len()
                            + if promoted_else.is_some() { 1 } else { 0 };
                        // Flatten the body into a synthetic block via Loop trick —
                        // actually just return a rebuilt If with no branches.
                        // Since body is a Vec<Stmt> we can't return it directly as
                        // a Stmt, so we keep a trimmed If.
                        Some(Stmt::If(cond, body, vec![], None))
                    },
                    Expr::Bool(false) => {
                        // The if-body never runs.
                        self.dce += 1;
                        match promoted_else {
                            // Something in the elif/else chain survived → keep it
                            // as a new top-level If with condition=true (always runs).
                            Some((new_cond, new_body, rest_elseifs, rest_else)) => {
                                Some(Stmt::If(new_cond, new_body, rest_elseifs, rest_else))
                            },
                            None => None, // nothing left
                        }
                    },
                    _ => Some(Stmt::If(cond, body, new_elseifs, promoted_else.map(
                        |(_, b, ei, eb)| {
                            // rebuild: the promoted_else contains the surviving else chain
                            // but we only need the else_body here; elseifs are already in new_elseifs
                            let _ = (ei, eb); // already folded into new_elseifs
                            b
                        }
                    ))),
                }
            },

            // while (false) { ... } → drop
            // while (true)  { ... } → loop { ... }  (minor cleanup, saves a cond-check per iter)
            Stmt::While(cond, body) => {
                match &cond {
                    Expr::Bool(false) => {
                        self.dce += 1;
                        None
                    },
                    Expr::Bool(true) => {
                        self.dce += 1; // we count the redundant condition as eliminated
                        Some(Stmt::Loop(body))
                    },
                    _ => Some(Stmt::While(cond, body)),
                }
            },

            other => Some(other),
        }
    }

    // Process an elif chain + else_body for DCE.
    // Returns (surviving_elseifs, promoted_else_info).
    // promoted_else_info is Some((cond, body, remaining_elseifs, remaining_else))
    // when a constant-true elif (or the original else) becomes the new else branch.
    fn dce_elif_chain(
        &mut self,
        elseifs: Vec<ElseIf>,
        else_body: Option<Vec<Stmt>>,
    ) -> (Vec<ElseIf>, Option<(Expr, Vec<Stmt>, Vec<ElseIf>, Option<Vec<Stmt>>)>) {
        let mut surviving = Vec::new();
        let mut remaining_iter = elseifs.into_iter();

        while let Some(elif) = remaining_iter.next() {
            match &elif.condition {
                Expr::Bool(false) => {
                    // This branch can never run; drop it.
                    self.dce += 1;
                },
                Expr::Bool(true) => {
                    // This branch always runs; it becomes the else, everything after is dead.
                    let rest: Vec<ElseIf> = remaining_iter.collect();
                    self.dce += rest.len() + if else_body.is_some() { 1 } else { 0 };
                    return (
                        surviving,
                        Some((elif.condition, elif.body, vec![], None)),
                    );
                },
                _ => surviving.push(elif),
            }
        }

        // Reconstruct else_body as promotion target (None if no else)
        let promoted = else_body.map(|b| (Expr::Bool(true), b, vec![], None));
        (surviving, promoted)
    }

    // -----------------------------------------------------------------------
    // Expression — constant folding + strength reduction
    // -----------------------------------------------------------------------
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        match expr {
            // Literals are already fully reduced.
            Expr::Int(_) | Expr::Bool(_) => expr,

            // Variable reference — nothing to fold.
            Expr::Var(_) => expr,

            // not expr
            Expr::Not(inner) => {
                let inner = self.fold_expr(*inner);
                match inner {
                    Expr::Bool(b) => {
                        self.folds += 1;
                        Expr::Bool(!b)
                    },
                    _ => Expr::Not(Box::new(inner)),
                }
            },

            // Binary operation — the main workhorse.
            Expr::BinOp(left, op, right) => {
                let left  = self.fold_expr(*left);
                let right = self.fold_expr(*right);
                self.fold_binop(left, op, right)
            },

            // Builtins with sub-expressions — recurse into arguments.
            Expr::Draw(x, y, s) => Expr::Draw(
                Box::new(self.fold_expr(*x)),
                Box::new(self.fold_expr(*y)),
                s,
            ),
            Expr::DrawDigit(x, y, n) => Expr::DrawDigit(
                Box::new(self.fold_expr(*x)),
                Box::new(self.fold_expr(*y)),
                Box::new(self.fold_expr(*n)),
            ),
            Expr::KeyPressed(k) => Expr::KeyPressed(Box::new(self.fold_expr(*k))),
            Expr::Rand(m)       => Expr::Rand(Box::new(self.fold_expr(*m))),
            Expr::Call(name, args) => {
                let args = args.into_iter().map(|a| self.fold_expr(a)).collect();
                Expr::Call(name, args)
            },

            // Builtins with no sub-expressions.
            Expr::GetKey | Expr::GetDelay => expr,
        }
    }

    // -----------------------------------------------------------------------
    // BinOp folding — called after both operands are already folded.
    // -----------------------------------------------------------------------
    fn fold_binop(&mut self, left: Expr, op: Op, right: Expr) -> Expr {
        use Op::*;

        // ── Pass 1: both operands are literals → evaluate fully ──────────
        match (&left, &op, &right) {
            // Arithmetic (Int op Int → Int)
            (Expr::Int(a), Add, Expr::Int(b)) => return self.folded_int((*a as i32 + *b as i32) as i16),
            (Expr::Int(a), Sub, Expr::Int(b)) => return self.folded_int((*a as i32 - *b as i32) as i16),
            (Expr::Int(a), Mul, Expr::Int(b)) => return self.folded_int((*a as i32 * *b as i32) as i16),
            (Expr::Int(a), Div, Expr::Int(b)) if *b != 0 => return self.folded_int(*a / *b),
            (Expr::Int(a), Mod, Expr::Int(b)) if *b != 0 => return self.folded_int(*a % *b),

            // Comparison (Int op Int → Bool)
            (Expr::Int(a), EqEq,  Expr::Int(b)) => return self.folded_bool(a == b),
            (Expr::Int(a), NotEq, Expr::Int(b)) => return self.folded_bool(a != b),
            (Expr::Int(a), Lt,    Expr::Int(b)) => return self.folded_bool(a < b),
            (Expr::Int(a), Gt,    Expr::Int(b)) => return self.folded_bool(a > b),
            (Expr::Int(a), LtEq,  Expr::Int(b)) => return self.folded_bool(a <= b),
            (Expr::Int(a), GtEq,  Expr::Int(b)) => return self.folded_bool(a >= b),

            // Comparison (Bool op Bool → Bool)
            (Expr::Bool(a), EqEq,  Expr::Bool(b)) => return self.folded_bool(a == b),
            (Expr::Bool(a), NotEq, Expr::Bool(b)) => return self.folded_bool(a != b),

            // Logic (Bool op Bool → Bool)
            (Expr::Bool(a), And, Expr::Bool(b)) => return self.folded_bool(*a && *b),
            (Expr::Bool(a), Or,  Expr::Bool(b)) => return self.folded_bool(*a || *b),

            _ => {}
        }

        // ── Pass 2: one operand is a literal → strength reduction ─────────

        // x + 0  →  x       x - 0  →  x
        if matches!(op, Add | Sub) {
            if let Expr::Int(0) = right { return self.reduced(left); }
        }
        if let (Op::Add, Expr::Int(0)) = (&op, &left) {
            return self.reduced(right);
        }

        // x * 0  →  0       0 * x  →  0
        if let Op::Mul = &op {
            if matches!(right, Expr::Int(0)) { return self.reduced(Expr::Int(0)); }
            if matches!(left,  Expr::Int(0)) { return self.reduced(Expr::Int(0)); }
        }

        // x * 1  →  x       1 * x  →  x
        if let Op::Mul = &op {
            if let Expr::Int(1) = right { return self.reduced(left); }
            if let Expr::Int(1) = left  { return self.reduced(right); }
        }

        // x / 1  →  x
        if let (Op::Div, Expr::Int(1)) = (&op, &right) {
            return self.reduced(left);
        }

        // x % 1  →  0
        if let (Op::Mod, Expr::Int(1)) = (&op, &right) {
            return self.reduced(Expr::Int(0));
        }

        // x * 2  →  x + x  (avoids the software multiply loop)
        // We only do this for the literal-2 case; larger powers of 2 would
        // need repeated addition which isn't necessarily cheaper past *4.
        if let (Op::Mul, Expr::Int(2)) = (&op, &right) {
            self.reductions += 1;
            let left_copy = left.clone();
            return Expr::BinOp(Box::new(left), Add, Box::new(left_copy));
        }
        if let (Op::Mul, Expr::Int(2)) = (&op, &left) {
            self.reductions += 1;
            let right_copy = right.clone();
            return Expr::BinOp(Box::new(right), Add, Box::new(right_copy));
        }

        // x and true  →  x       x and false  →  false
        if let Op::And = &op {
            if let Expr::Bool(true)  = right { return self.reduced(left); }
            if let Expr::Bool(false) = right { return self.reduced(Expr::Bool(false)); }
            if let Expr::Bool(true)  = left  { return self.reduced(right); }
            if let Expr::Bool(false) = left  { return self.reduced(Expr::Bool(false)); }
        }

        // x or false  →  x       x or true  →  true
        if let Op::Or = &op {
            if let Expr::Bool(false) = right { return self.reduced(left); }
            if let Expr::Bool(true)  = right { return self.reduced(Expr::Bool(true)); }
            if let Expr::Bool(false) = left  { return self.reduced(right); }
            if let Expr::Bool(true)  = left  { return self.reduced(Expr::Bool(true)); }
        }

        // x == true  →  x       x == false  →  not x
        if let Op::EqEq = &op {
            if let Expr::Bool(true)  = right { return self.reduced(left); }
            if let Expr::Bool(false) = right { return self.reduced(Expr::Not(Box::new(left))); }
            if let Expr::Bool(true)  = left  { return self.reduced(right); }
            if let Expr::Bool(false) = left  { return self.reduced(Expr::Not(Box::new(right))); }
        }

        // x != true  →  not x   x != false  →  x
        if let Op::NotEq = &op {
            if let Expr::Bool(true)  = right { return self.reduced(Expr::Not(Box::new(left))); }
            if let Expr::Bool(false) = right { return self.reduced(left); }
            if let Expr::Bool(true)  = left  { return self.reduced(Expr::Not(Box::new(right))); }
            if let Expr::Bool(false) = left  { return self.reduced(right); }
        }

        // Nothing matched — return unchanged.
        Expr::BinOp(Box::new(left), op, Box::new(right))
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn folded_int(&mut self, n: i16) -> Expr {
        self.folds += 1;
        Expr::Int(n)
    }

    fn folded_bool(&mut self, b: bool) -> Expr {
        self.folds += 1;
        Expr::Bool(b)
    }

    fn reduced(&mut self, e: Expr) -> Expr {
        self.reductions += 1;
        e
    }
}
