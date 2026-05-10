#![cfg(test)]

mod common;

use common::assert_panics;
use common::parse;

use chipscript::ast::*;

// --- Program structure --------------------------------------------------

#[test]
fn empty_main() {
    let p = parse("main {}");
    assert!(p.main.is_empty());
    assert!(p.vars.is_empty());
    assert!(p.sprites.is_empty());
    assert!(p.functions.is_empty());
}

#[test]
fn vars_block() {
    let p = parse("vars { x = 10; y = false; }");
    assert_eq!(p.vars.len(), 2);
    assert_eq!(p.vars[0].name, "x");
    assert!(matches!(p.vars[0].value, Expr::Int(10)));
    assert_eq!(p.vars[1].name, "y");
    assert!(matches!(p.vars[1].value, Expr::Bool(false)));
}

#[test]
fn sprites_inline() {
    let p = parse("sprites { ball = [0x3C, 0x7E]; }");
    assert_eq!(p.sprites.len(), 1);
    assert_eq!(p.sprites[0].name, "ball");
    assert!(matches!(&p.sprites[0].data, SpriteData::Inline(b) if b == &vec![0x3C, 0x7E]));
}

#[test]
fn sprites_file() {
    let p = parse(r#"sprites { ship = "ship.spr"; }"#);
    assert!(matches!(&p.sprites[0].data, SpriteData::File(s) if s == "ship.spr"));
}

#[test]
fn function_declaration() {
    let p = parse("fn add(a, b) -> result { result = a; }");
    assert_eq!(p.functions.len(), 1);
    let f = &p.functions[0];
    assert_eq!(f.name, "add");
    assert_eq!(f.args, vec!["a", "b"]);
    assert_eq!(f.ret, "result");
    assert_eq!(f.body.len(), 1);
}

#[test]
fn function_no_args() {
    let p = parse("fn noop() -> r { r = 0; }");
    assert!(p.functions[0].args.is_empty());
}

// --- Statements ---------------------------------------------------------

#[test]
fn assign_stmt() {
    let p = parse("vars { x = 0; } main { x = 5; }");
    assert!(matches!(&p.main[0], Stmt::Assign(n, Expr::Int(5)) if n == "x"));
}

#[test]
fn clear_stmt() {
    let p = parse("main { clear(); }");
    assert!(matches!(p.main[0], Stmt::Clear));
}

#[test]
fn delay_stmt() {
    let p = parse("main { delay(10); }");
    assert!(matches!(&p.main[0], Stmt::Delay(Expr::Int(10))));
}

#[test]
fn beep_stmt() {
    let p = parse("main { beep(5); }");
    assert!(matches!(&p.main[0], Stmt::Beep(Expr::Int(5))));
}

#[test]
fn loop_stmt() {
    let p = parse("main { loop { clear(); } }");
    assert!(matches!(&p.main[0], Stmt::Loop(_)));
    if let Stmt::Loop(body) = &p.main[0] {
        assert_eq!(body.len(), 1);
    }
}

#[test]
fn while_stmt() {
    let p = parse("vars { x = 0; } main { while (x == 0) { clear(); } }");
    assert!(matches!(&p.main[0], Stmt::While(_, _)));
}

#[test]
fn call_stmt() {
    let p = parse("fn f() -> r { r = 0; } main { f(); }");
    assert!(matches!(&p.main[0], Stmt::Call(n, _) if n == "f"));
}

// --- If / elif / else ---------------------------------------------------

#[test]
fn if_only() {
    let p = parse("vars { x = 0; } main { if (x == 0) { clear(); } }");
    assert!(matches!(&p.main[0],
            Stmt::If(_, body, elseifs, None) if body.len() == 1 && elseifs.is_empty()));
}

#[test]
fn if_else() {
    let p = parse("vars { x = 0; } main { if (x == 0) { clear(); } else { beep(1); } }");
    assert!(matches!(&p.main[0],
            Stmt::If(_, _, elseifs, Some(_)) if elseifs.is_empty()));
}

#[test]
fn if_elif_else() {
    let p = parse(
        "vars { x = 0; } main {
            if (x == 0) { clear(); }
            elif (x == 1) { beep(1); }
            else { beep(2); }
        }",
    );
    if let Stmt::If(_, _, elseifs, else_body) = &p.main[0] {
        assert_eq!(elseifs.len(), 1);
        assert!(else_body.is_some());
    } else {
        panic!("expected If");
    }
}

#[test]
fn multiple_elif() {
    let p = parse(
        "vars { x = 0; } main {
            if (x == 0) { clear(); }
            elif (x == 1) { clear(); }
            elif (x == 2) { clear(); }
        }",
    );
    if let Stmt::If(_, _, elseifs, _) = &p.main[0] {
        assert_eq!(elseifs.len(), 2);
    } else {
        panic!("expected If");
    }
}

// --- Expressions --------------------------------------------------------

#[test]
fn int_literal_expr() {
    let p = parse("vars { x = 0; } main { x = 42; }");
    assert!(matches!(&p.main[0], Stmt::Assign(_, Expr::Int(42))));
}

#[test]
fn bool_literal_expr() {
    let p = parse("vars { b = false; } main { b = true; }");
    assert!(matches!(&p.main[0], Stmt::Assign(_, Expr::Bool(true))));
}

#[test]
fn binop_add() {
    let p = parse("vars { x = 0; } main { x = x + 1; }");
    if let Stmt::Assign(_, Expr::BinOp(_, op, _)) = &p.main[0] {
        assert_eq!(*op, Op::Add);
    } else {
        panic!("expected BinOp");
    }
}

#[test]
fn binop_all_arithmetic() {
    for (sym, expected_op) in &[
        ("+", Op::Add),
        ("-", Op::Sub),
        ("*", Op::Mul),
        ("/", Op::Div),
        ("%", Op::Mod),
    ] {
        let src = format!("vars {{ x = 0; }} main {{ x = x {} x; }}", sym);
        let p = parse(&src);
        if let Stmt::Assign(_, Expr::BinOp(_, op, _)) = &p.main[0] {
            assert_eq!(op, expected_op, "op mismatch for {}", sym);
        } else {
            panic!("expected BinOp for {}", sym);
        }
    }
}

#[test]
fn binop_all_comparisons() {
    for (sym, expected_op) in &[
        ("==", Op::EqEq),
        ("!=", Op::NotEq),
        ("<", Op::Lt),
        (">", Op::Gt),
        ("<=", Op::LtEq),
        (">=", Op::GtEq),
    ] {
        let src = format!("vars {{ b = false; x = 0; }} main {{ b = x {} x; }}", sym);
        let p = parse(&src);
        if let Stmt::Assign(_, Expr::BinOp(_, op, _)) = &p.main[0] {
            assert_eq!(op, expected_op, "op mismatch for {}", sym);
        } else {
            panic!("expected BinOp for {}", sym);
        }
    }
}

#[test]
fn not_expr() {
    let p = parse("vars { b = false; } main { b = not b; }");
    assert!(matches!(&p.main[0], Stmt::Assign(_, Expr::Not(_))));
}

#[test]
fn parenthesised_expr() {
    let p = parse("vars { x = 0; } main { x = (x + 1); }");
    assert!(matches!(&p.main[0], Stmt::Assign(_, Expr::BinOp(..))));
}

#[test]
fn nested_binop() {
    // (a + b) + c — left-associative
    let p = parse("vars { x = 0; } main { x = x + x + x; }");
    // outer op should be Add, left should also be BinOp(Add)
    if let Stmt::Assign(_, Expr::BinOp(left, Op::Add, _)) = &p.main[0] {
        assert!(matches!(left.as_ref(), Expr::BinOp(_, Op::Add, _)));
    } else {
        panic!("expected nested BinOp");
    }
}

#[test]
fn call_expr() {
    let p = parse("fn f() -> r { r = 0; } vars { x = 0; } main { x = f(); }");
    assert!(matches!(&p.main[0], Stmt::Assign(_, Expr::Call(n, _)) if n == "f"));
}

#[test]
fn draw_expr() {
    let p = parse("sprites { b = [0xFF]; } vars { h = false; } main { h = draw(0, 0, b); }");
    assert!(matches!(&p.main[0], Stmt::Assign(_, Expr::Draw(..))));
}

#[test]
fn drawdigit_expr() {
    let p = parse("vars { h = false; } main { h = drawdigit(0, 0, 5); }");
    assert!(matches!(&p.main[0], Stmt::Assign(_, Expr::DrawDigit(..))));
}

#[test]
fn getkey_expr() {
    let p = parse("vars { k = 0; } main { k = getkey(); }");
    assert!(matches!(&p.main[0], Stmt::Assign(_, Expr::GetKey)));
}

#[test]
fn getdelay_expr() {
    let p = parse("vars { t = 0; } main { t = getdelay(); }");
    assert!(matches!(&p.main[0], Stmt::Assign(_, Expr::GetDelay)));
}

#[test]
fn keypressed_expr() {
    let p = parse("vars { b = false; } main { b = keypressed(5); }");
    assert!(matches!(&p.main[0], Stmt::Assign(_, Expr::KeyPressed(_))));
}

#[test]
fn rand_expr() {
    let p = parse("vars { r = 0; } main { r = rand(0xFF); }");
    assert!(matches!(&p.main[0], Stmt::Assign(_, Expr::Rand(_))));
}

// --- Sections in any order ---------------------------------------------

#[test]
fn sections_any_order() {
    // main before vars and sprites — all valid
    let p = parse("main { clear(); } vars { x = 0; } sprites { b = [0xFF]; }");
    assert!(!p.main.is_empty());
    assert!(!p.vars.is_empty());
    assert!(!p.sprites.is_empty());
}

// --- Error cases --------------------------------------------------------

#[test]
fn unexpected_top_level_token_panics() {
    assert_panics(|| {
        parse("42");
    });
}

#[test]
fn missing_semicolon_panics() {
    assert_panics(|| {
        parse("vars { x = 0 }");
    });
}

#[test]
fn unclosed_brace_panics() {
    assert_panics(|| {
        parse("main { clear();");
    });
}
