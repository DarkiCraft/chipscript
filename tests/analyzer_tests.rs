#![cfg(test)]

mod common;

use common::assert_panics;
use common::parse;

use chipscript::analyzer::Analyzer;

fn analyze(src: &str) {
    let p = parse(src);
    Analyzer::new().analyze(&p);
}

// --- Valid programs -----------------------------------------------------

#[test]
fn minimal_valid_program() {
    analyze("main { clear(); }");
}

#[test]
fn vars_and_assign() {
    analyze("vars { x = 0; } main { x = 5; }");
}

#[test]
fn bool_var_assign() {
    analyze("vars { b = false; } main { b = true; }");
}

#[test]
fn int_arithmetic() {
    analyze("vars { x = 0; } main { x = x + 1; }");
}

#[test]
fn comparison_produces_bool() {
    analyze("vars { b = false; x = 0; } main { b = (x == 0); }");
}

#[test]
fn logical_ops() {
    analyze("vars { a = false; b = false; } main { a = a and b; a = a or b; a = not b; }");
}

#[test]
fn if_with_bool_cond() {
    analyze("vars { x = 0; b = false; } main { if (b) { x = 1; } }");
}

#[test]
fn while_with_bool_cond() {
    analyze("vars { b = false; } main { while (b) { clear(); } }");
}

#[test]
fn function_call_valid() {
    analyze("fn add(a, b) -> r { r = a; } vars { x = 0; } main { x = add(1, 2); }");
}

#[test]
fn function_stmt_call() {
    analyze("fn f() -> r { r = 0; } main { f(); }");
}

#[test]
fn sprite_inline_valid() {
    analyze("sprites { b = [0xFF]; } vars { h = false; } main { h = draw(0, 0, b); }");
}

#[test]
fn drawdigit_valid() {
    analyze("vars { h = false; x = 0; } main { h = drawdigit(0, 0, x); }");
}

#[test]
fn getkey_is_int() {
    analyze("vars { k = 0; } main { k = getkey(); }");
}

#[test]
fn getdelay_is_int() {
    analyze("vars { t = 0; } main { t = getdelay(); }");
}

#[test]
fn keypressed_is_bool() {
    analyze("vars { b = false; } main { b = keypressed(5); }");
}

#[test]
fn rand_is_int() {
    analyze("vars { r = 0; } main { r = rand(0xFF); }");
}

#[test]
fn delay_accepts_int() {
    analyze("vars { t = 0; } main { delay(t); }");
}

#[test]
fn beep_accepts_int() {
    analyze("main { beep(10); }");
}

#[test]
fn exactly_15_vars_allowed() {
    let vars: String = (0..15)
        .map(|i| format!("v{} = 0;", i))
        .collect::<Vec<_>>()
        .join(" ");
    analyze(&format!("vars {{ {} }} main {{ clear(); }}", vars));
}

// --- Type errors --------------------------------------------------------

#[test]
fn assign_bool_to_int_errors() {
    assert_panics(|| {
        analyze("vars { x = 0; } main { x = true; }");
    });
}

#[test]
fn assign_int_to_bool_errors() {
    assert_panics(|| {
        analyze("vars { b = false; } main { b = 5; }");
    });
}

#[test]
fn arithmetic_on_bool_errors() {
    assert_panics(|| {
        analyze("vars { b = false; } main { b = b + b; }");
    });
}

#[test]
fn logic_on_int_errors() {
    assert_panics(|| {
        analyze("vars { x = 0; } main { x = x and x; }");
    });
}

#[test]
fn not_on_int_errors() {
    assert_panics(|| {
        analyze("vars { x = 0; b = false; } main { b = not x; }");
    });
}

#[test]
fn comparison_mixed_types_errors() {
    assert_panics(|| {
        analyze("vars { x = 0; b = false; r = false; } main { r = (x == b); }");
    });
}

#[test]
fn if_int_condition_errors() {
    assert_panics(|| {
        analyze("vars { x = 0; } main { if (x) { clear(); } }");
    });
}

#[test]
fn while_int_condition_errors() {
    assert_panics(|| {
        analyze("vars { x = 0; } main { while (x) { clear(); } }");
    });
}

#[test]
fn delay_with_bool_errors() {
    assert_panics(|| {
        analyze("vars { b = false; } main { delay(b); }");
    });
}

#[test]
fn beep_with_bool_errors() {
    assert_panics(|| {
        analyze("vars { b = false; } main { beep(b); }");
    });
}

// --- Undeclared names ---------------------------------------------------

#[test]
fn undeclared_variable_errors() {
    assert_panics(|| {
        analyze("main { x = 5; }");
    });
}

#[test]
fn undeclared_function_errors() {
    assert_panics(|| {
        analyze("main { nope(); }");
    });
}

#[test]
fn undeclared_sprite_errors() {
    assert_panics(|| {
        analyze("vars { h = false; } main { h = draw(0, 0, ghost); }");
    });
}

// --- Limits -------------------------------------------------------------

#[test]
fn too_many_vars_errors() {
    let vars: String = (0..16)
        .map(|i| format!("v{} = 0;", i))
        .collect::<Vec<_>>()
        .join(" ");
    assert_panics(|| {
        analyze(&format!("vars {{ {} }} main {{ clear(); }}", vars));
    });
}

#[test]
fn wrong_arg_count_errors() {
    assert_panics(|| {
        analyze("fn f(a) -> r { r = a; } vars { x = 0; } main { x = f(1, 2); }");
    });
}

#[test]
fn rand_with_variable_mask_errors() {
    assert_panics(|| {
        analyze("vars { r = 0; m = 0; } main { r = rand(m); }");
    });
}

#[test]
fn sprite_empty_errors() {
    assert_panics(|| {
        analyze("sprites { b = []; } vars { h = false; } main { h = draw(0, 0, b); }");
    });
}

#[test]
fn sprite_too_tall_errors() {
    let bytes = (0..16).map(|_| "0xFF").collect::<Vec<_>>().join(", ");
    assert_panics(|| {
        analyze(&format!(
            "sprites {{ b = [{}]; }} vars {{ h = false; }} main {{ h = draw(0, 0, b); }}",
            bytes
        ));
    });
}
