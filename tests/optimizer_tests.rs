#![cfg(test)]

mod common;

use common::parse;

use chipscript::ast::*;
use chipscript::optimizer::Optimizer;

mod optimizer_tests {
    use super::*;

    fn optimize_expr(expr: Expr) -> Expr {
        let mut opt = Optimizer::new();
        opt.fold_expr(expr)
    }

    fn opt_program(src: &str) -> Program {
        let mut p = parse(src);
        Optimizer::new().optimize(&mut p);
        return p;
    }

    // --- Constant Folding: arithmetic --------------------------------------

    #[test]
    fn fold_add() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Int(3)),
                Op::Add,
                Box::new(Expr::Int(4))
            )),
            Expr::Int(7)
        ));
    }

    #[test]
    fn fold_sub() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Int(10)),
                Op::Sub,
                Box::new(Expr::Int(3))
            )),
            Expr::Int(7)
        ));
    }

    #[test]
    fn fold_mul() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Int(6)),
                Op::Mul,
                Box::new(Expr::Int(7))
            )),
            Expr::Int(42)
        ));
    }

    #[test]
    fn fold_div() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Int(10)),
                Op::Div,
                Box::new(Expr::Int(2))
            )),
            Expr::Int(5)
        ));
    }

    #[test]
    fn fold_mod() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Int(10)),
                Op::Mod,
                Box::new(Expr::Int(3))
            )),
            Expr::Int(1)
        ));
    }

    #[test]
    fn fold_div_by_zero_errors() {
        // Division by zero is a compile-time error (matches SPECS)
        let expr = Expr::BinOp(Box::new(Expr::Int(5)), Op::Div, Box::new(Expr::Int(0)));
        assert!(
            std::panic::catch_unwind(|| {
                optimize_expr(expr);
            })
            .is_err()
        );
    }

    #[test]
    fn fold_mod_by_zero_errors() {
        let expr = Expr::BinOp(Box::new(Expr::Int(5)), Op::Mod, Box::new(Expr::Int(0)));
        assert!(
            std::panic::catch_unwind(|| {
                optimize_expr(expr);
            })
            .is_err()
        );
    }

    #[test]
    fn fold_wraps_on_overflow() {
        // i8 max + 1 wraps: 127 + 1 = 128 cast to i16 then truncated per codegen
        let result = optimize_expr(Expr::BinOp(
            Box::new(Expr::Int(127)),
            Op::Add,
            Box::new(Expr::Int(1)),
        ));
        assert!(matches!(result, Expr::Int(128))); // stays as i16 128; codegen casts to u8
    }

    // --- Constant Folding: comparisons ------------------------------------

    #[test]
    fn fold_eqeq_true() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Int(5)),
                Op::EqEq,
                Box::new(Expr::Int(5))
            )),
            Expr::Bool(true)
        ));
    }

    #[test]
    fn fold_eqeq_false() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Int(5)),
                Op::EqEq,
                Box::new(Expr::Int(6))
            )),
            Expr::Bool(false)
        ));
    }

    #[test]
    fn fold_noteq() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Int(5)),
                Op::NotEq,
                Box::new(Expr::Int(6))
            )),
            Expr::Bool(true)
        ));
    }

    #[test]
    fn fold_lt() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Int(3)),
                Op::Lt,
                Box::new(Expr::Int(5))
            )),
            Expr::Bool(true)
        ));
    }

    #[test]
    fn fold_gt() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Int(5)),
                Op::Gt,
                Box::new(Expr::Int(3))
            )),
            Expr::Bool(true)
        ));
    }

    #[test]
    fn fold_lteq_equal() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Int(5)),
                Op::LtEq,
                Box::new(Expr::Int(5))
            )),
            Expr::Bool(true)
        ));
    }

    #[test]
    fn fold_gteq() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Int(5)),
                Op::GtEq,
                Box::new(Expr::Int(3))
            )),
            Expr::Bool(true)
        ));
    }

    // --- Constant Folding: logic ------------------------------------------

    #[test]
    fn fold_and_tt() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Bool(true)),
                Op::And,
                Box::new(Expr::Bool(true))
            )),
            Expr::Bool(true)
        ));
    }

    #[test]
    fn fold_and_tf() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Bool(true)),
                Op::And,
                Box::new(Expr::Bool(false))
            )),
            Expr::Bool(false)
        ));
    }

    #[test]
    fn fold_or_ft() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Bool(false)),
                Op::Or,
                Box::new(Expr::Bool(true))
            )),
            Expr::Bool(true)
        ));
    }

    #[test]
    fn fold_or_ff() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Bool(false)),
                Op::Or,
                Box::new(Expr::Bool(false))
            )),
            Expr::Bool(false)
        ));
    }

    #[test]
    fn fold_not_true() {
        assert!(matches!(
            optimize_expr(Expr::Not(Box::new(Expr::Bool(true)))),
            Expr::Bool(false)
        ));
    }

    #[test]
    fn fold_not_false() {
        assert!(matches!(
            optimize_expr(Expr::Not(Box::new(Expr::Bool(false)))),
            Expr::Bool(true)
        ));
    }

    // --- Constant Folding: bool comparisons --------------------------------

    #[test]
    fn fold_bool_eqeq() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Bool(true)),
                Op::EqEq,
                Box::new(Expr::Bool(true))
            )),
            Expr::Bool(true)
        ));
    }

    #[test]
    fn fold_bool_noteq() {
        assert!(matches!(
            optimize_expr(Expr::BinOp(
                Box::new(Expr::Bool(true)),
                Op::NotEq,
                Box::new(Expr::Bool(false))
            )),
            Expr::Bool(true)
        ));
    }

    // --- Constant Folding: nested / fixed-point ----------------------------

    #[test]
    fn fold_nested_expr() {
        // (1 + 2) * (3 + 4)  →  3 * 7  →  21
        let inner_left = Expr::BinOp(Box::new(Expr::Int(1)), Op::Add, Box::new(Expr::Int(2)));
        let inner_right = Expr::BinOp(Box::new(Expr::Int(3)), Op::Add, Box::new(Expr::Int(4)));
        let outer = Expr::BinOp(Box::new(inner_left), Op::Mul, Box::new(inner_right));
        assert!(matches!(optimize_expr(outer), Expr::Int(21)));
    }

    #[test]
    fn fold_in_var_initializer() {
        let p = opt_program("vars { x = 3 + 4; } main { clear(); }");
        assert!(matches!(p.vars[0].value, Expr::Int(7)));
    }

    #[test]
    fn fold_in_assign() {
        let p = opt_program("vars { x = 0; } main { x = 2 + 3; }");
        if let Stmt::Assign(_, expr) = &p.main[0] {
            assert!(matches!(expr, Expr::Int(5)));
        } else {
            panic!("expected Assign");
        }
    }

    #[test]
    fn fold_in_delay() {
        let p = opt_program("main { delay(10 + 5); }");
        if let Stmt::Delay(expr) = &p.main[0] {
            assert!(matches!(expr, Expr::Int(15)));
        } else {
            panic!("expected Delay");
        }
    }

    // --- Dead Code Elimination ---------------------------------------------

    #[test]
    fn dce_if_false_removed() {
        let p = opt_program("vars { x = 0; } main { if (false) { x = 99; } }");
        assert!(p.main.is_empty(), "if(false) body should be removed");
    }

    #[test]
    fn dce_if_true_kept_else_removed() {
        let p = opt_program(
            "vars { x = 0; } main {
            if (true) { x = 1; } else { x = 99; }
        }",
        );
        assert_eq!(p.main.len(), 1);
        if let Stmt::If(_, body, elseifs, else_body) = &p.main[0] {
            assert_eq!(body.len(), 1);
            assert!(elseifs.is_empty());
            assert!(else_body.is_none());
        } else {
            panic!("expected If");
        }
    }

    #[test]
    fn dce_while_false_removed() {
        let p = opt_program("main { while (false) { clear(); } }");
        assert!(p.main.is_empty(), "while(false) should be removed");
    }

    #[test]
    fn dce_while_true_becomes_loop() {
        let p = opt_program("main { while (true) { clear(); } }");
        assert!(matches!(&p.main[0], Stmt::Loop(_)));
    }

    #[test]
    fn dce_elif_false_pruned() {
        let p = opt_program(
            "vars { x = 0; b = false; } main {
            if (b) { x = 1; }
            elif (false) { x = 2; }
            else { x = 3; }
        }",
        );
        if let Stmt::If(_, _, elseifs, _) = &p.main[0] {
            assert!(elseifs.is_empty(), "elif(false) should be pruned");
        } else {
            panic!("expected If");
        }
    }

    #[test]
    fn dce_constant_folded_condition() {
        // The condition 1 == 1 folds to true, then DCE fires
        let p = opt_program("vars { x = 0; } main { if (1 == 1) { x = 1; } else { x = 99; } }");
        if let Stmt::If(cond, _, _, else_body) = &p.main[0] {
            assert!(matches!(cond, Expr::Bool(true)));
            assert!(else_body.is_none());
        } else {
            panic!("expected If");
        }
    }

    // --- Strength Reduction ------------------------------------------------

    #[test]
    fn strength_add_zero_right() {
        let expr = Expr::BinOp(
            Box::new(Expr::Var("x".into())),
            Op::Add,
            Box::new(Expr::Int(0)),
        );
        assert!(matches!(optimize_expr(expr), Expr::Var(n) if n == "x"));
    }

    #[test]
    fn strength_add_zero_left() {
        let expr = Expr::BinOp(
            Box::new(Expr::Int(0)),
            Op::Add,
            Box::new(Expr::Var("x".into())),
        );
        assert!(matches!(optimize_expr(expr), Expr::Var(n) if n == "x"));
    }

    #[test]
    fn strength_sub_zero() {
        let expr = Expr::BinOp(
            Box::new(Expr::Var("x".into())),
            Op::Sub,
            Box::new(Expr::Int(0)),
        );
        assert!(matches!(optimize_expr(expr), Expr::Var(n) if n == "x"));
    }

    #[test]
    fn strength_mul_zero_right() {
        let expr = Expr::BinOp(
            Box::new(Expr::Var("x".into())),
            Op::Mul,
            Box::new(Expr::Int(0)),
        );
        assert!(matches!(optimize_expr(expr), Expr::Int(0)));
    }

    #[test]
    fn strength_mul_zero_left() {
        let expr = Expr::BinOp(
            Box::new(Expr::Int(0)),
            Op::Mul,
            Box::new(Expr::Var("x".into())),
        );
        assert!(matches!(optimize_expr(expr), Expr::Int(0)));
    }

    #[test]
    fn strength_mul_one_right() {
        let expr = Expr::BinOp(
            Box::new(Expr::Var("x".into())),
            Op::Mul,
            Box::new(Expr::Int(1)),
        );
        assert!(matches!(optimize_expr(expr), Expr::Var(n) if n == "x"));
    }

    #[test]
    fn strength_mul_one_left() {
        let expr = Expr::BinOp(
            Box::new(Expr::Int(1)),
            Op::Mul,
            Box::new(Expr::Var("x".into())),
        );
        assert!(matches!(optimize_expr(expr), Expr::Var(n) if n == "x"));
    }

    #[test]
    fn strength_div_one() {
        let expr = Expr::BinOp(
            Box::new(Expr::Var("x".into())),
            Op::Div,
            Box::new(Expr::Int(1)),
        );
        assert!(matches!(optimize_expr(expr), Expr::Var(n) if n == "x"));
    }

    #[test]
    fn strength_mod_one() {
        let expr = Expr::BinOp(
            Box::new(Expr::Var("x".into())),
            Op::Mod,
            Box::new(Expr::Int(1)),
        );
        assert!(matches!(optimize_expr(expr), Expr::Int(0)));
    }

    #[test]
    fn strength_mul_two_becomes_add() {
        let expr = Expr::BinOp(
            Box::new(Expr::Var("x".into())),
            Op::Mul,
            Box::new(Expr::Int(2)),
        );
        assert!(matches!(optimize_expr(expr), Expr::BinOp(_, Op::Add, _)));
    }

    #[test]
    fn strength_and_true_right() {
        let expr = Expr::BinOp(
            Box::new(Expr::Var("b".into())),
            Op::And,
            Box::new(Expr::Bool(true)),
        );
        assert!(matches!(optimize_expr(expr), Expr::Var(n) if n == "b"));
    }

    #[test]
    fn strength_and_false_right() {
        let expr = Expr::BinOp(
            Box::new(Expr::Var("b".into())),
            Op::And,
            Box::new(Expr::Bool(false)),
        );
        assert!(matches!(optimize_expr(expr), Expr::Bool(false)));
    }

    #[test]
    fn strength_or_false_right() {
        let expr = Expr::BinOp(
            Box::new(Expr::Var("b".into())),
            Op::Or,
            Box::new(Expr::Bool(false)),
        );
        assert!(matches!(optimize_expr(expr), Expr::Var(n) if n == "b"));
    }

    #[test]
    fn strength_or_true_right() {
        let expr = Expr::BinOp(
            Box::new(Expr::Var("b".into())),
            Op::Or,
            Box::new(Expr::Bool(true)),
        );
        assert!(matches!(optimize_expr(expr), Expr::Bool(true)));
    }

    #[test]
    fn strength_eqeq_true() {
        // x == true  →  x
        let expr = Expr::BinOp(
            Box::new(Expr::Var("b".into())),
            Op::EqEq,
            Box::new(Expr::Bool(true)),
        );
        assert!(matches!(optimize_expr(expr), Expr::Var(n) if n == "b"));
    }

    #[test]
    fn strength_eqeq_false() {
        // x == false  →  not x
        let expr = Expr::BinOp(
            Box::new(Expr::Var("b".into())),
            Op::EqEq,
            Box::new(Expr::Bool(false)),
        );
        assert!(matches!(optimize_expr(expr), Expr::Not(_)));
    }

    #[test]
    fn strength_noteq_true() {
        // x != true  →  not x
        let expr = Expr::BinOp(
            Box::new(Expr::Var("b".into())),
            Op::NotEq,
            Box::new(Expr::Bool(true)),
        );
        assert!(matches!(optimize_expr(expr), Expr::Not(_)));
    }

    #[test]
    fn strength_noteq_false() {
        // x != false  →  x
        let expr = Expr::BinOp(
            Box::new(Expr::Var("b".into())),
            Op::NotEq,
            Box::new(Expr::Bool(false)),
        );
        assert!(matches!(optimize_expr(expr), Expr::Var(n) if n == "b"));
    }

    // --- Counters ----------------------------------------------------------

    #[test]
    fn fold_counter_increments() {
        let mut opt = Optimizer::new();
        let expr = Expr::BinOp(Box::new(Expr::Int(1)), Op::Add, Box::new(Expr::Int(2)));
        opt.fold_expr(expr);
        assert!(opt.folds > 0);
    }

    #[test]
    fn dce_counter_increments() {
        let mut p = parse("main { while (false) { clear(); } }");
        let mut opt = Optimizer::new();
        opt.optimize(&mut p);
        assert!(opt.dce > 0);
    }

    #[test]
    fn reduction_counter_increments() {
        let mut opt = Optimizer::new();
        let expr = Expr::BinOp(
            Box::new(Expr::Var("x".into())),
            Op::Add,
            Box::new(Expr::Int(0)),
        );
        opt.fold_expr(expr);
        assert!(opt.reductions > 0);
    }
}
