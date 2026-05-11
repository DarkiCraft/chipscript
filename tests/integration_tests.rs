#![cfg(test)]

mod common;

use common::compile;
use common::compile_no_opt;

// Every test here runs the full pipeline without panicking.

#[test]
fn minimal_program() {
    let rom = compile("main { clear(); }");
    assert!(!rom.is_empty());
}

#[test]
fn bouncing_ball() {
    let src = r#"
            sprites { ball = [0x3C, 0x7E, 0x7E, 0x3C]; }
            vars { bx = 32; by = 16; dx = 1; dy = 1; hit = false; }
            main {
                loop {
                    hit = draw(bx, by, ball);
                    bx = bx + dx;
                    by = by + dy;
                    if ((bx <= 0) or (bx >= 60)) { dx = dx * -1; }
                    if ((by <= 0) or (by >= 28)) { dy = dy * -1; }
                    hit = draw(bx, by, ball);
                    delay(1);
                }
            }
        "#;
    let rom = compile(src);
    assert!(!rom.is_empty());
}

#[test]
fn keyboard_counter() {
    let src = r#"
            vars { count = 0; k = 0; b = false; }
            main {
                clear();
                loop {
                    b = drawdigit(28, 12, count);
                    k = getkey();
                    b = drawdigit(28, 12, count);
                    count = count + 1;
                    if (count == 16) { count = 0; }
                }
            }
        "#;
    compile(src);
}

#[test]
fn dice_roller() {
    let src = r#"
            vars { roll = 0; k = 0; b = false; }
            fn roll_die(seed) -> result {
                result = rand(0x07);
                if (result > 5) { result = rand(0x05); }
                result = result + 1;
            }
            main {
                clear();
                loop {
                    k    = getkey();
                    roll = roll_die(k);
                    clear();
                    b = drawdigit(28, 12, roll);
                    if (roll == 6) { beep(20); }
                }
            }
        "#;
    compile(src);
}

#[test]
fn wandering_dot() {
    let src = r#"
            sprites { seg = [0xF0]; }
            vars { hx = 32; hy = 16; dx = 1; dy = 0; hit = false; r = 0; }
            main {
                loop {
                    r = rand(0x03);
                    if (r == 0) { dx = 1;  dy = 0; }
                    if (r == 1) { dx = 0;  dy = 1; }
                    if (r == 2) { dx = -1; dy = 0; }
                    if (r == 3) { dx = 0;  dy = -1; }
                    hx = hx + dx;
                    hy = hy + dy;
                    if (hx == 64)  { hx = 0;  }
                    if (hy == 32)  { hy = 0;  }
                    hit = draw(hx, hy, seg);
                    delay(6);
                    r = getdelay();
                    while (r != 0) { r = getdelay(); }
                }
            }
        "#;
    compile(src);
}

#[test]
fn multiple_functions() {
    let src = r#"
            fn double(x) -> r { r = x + x; }
            fn triple(x) -> r { r = x + x; r = r + x; }
            vars { a = 0; b = 0; }
            main {
                a = double(3);
                b = triple(3);
            }
        "#;
    compile(src);
}

#[test]
fn nested_if_elif_else() {
    let src = r#"
            vars { x = 0; k = 0; }
            main {
                k = getkey();
                if (k == 0) { x = 1; }
                elif (k == 1) { x = 2; }
                elif (k == 2) { x = 3; }
                else { x = 0; }
            }
        "#;
    compile(src);
}

#[test]
fn while_loop_program() {
    let src = r#"
            vars { t = 0; }
            main {
                delay(60);
                t = getdelay();
                while (t != 0) { t = getdelay(); }
                clear();
            }
        "#;
    compile(src);
}

#[test]
fn all_arithmetic_ops() {
    let src = r#"
            vars { a = 0; b = 0; c = 0; }
            main {
                a = a + 1;
                b = b - 1;
                c = a * 2;
                a = c / 2;
                b = c % 3;
            }
        "#;
    compile(src);
}

#[test]
fn all_comparison_ops() {
    let src = r#"
            vars { x = 0; b = false; }
            main {
                b = (x == 0);
                b = (x != 0);
                b = (x < 5);
                b = (x > 5);
                b = (x <= 5);
                b = (x >= 5);
            }
        "#;
    compile(src);
}

#[test]
fn optimizer_reduces_rom_size_for_constant_mul() {
    // x * 1 should be stripped; without opt it emits the full software loop
    let src = "vars { x = 0; } main { x = x * 1; }";
    let opt = compile(src);
    let no_opt = compile_no_opt(src);
    assert!(
        opt.len() < no_opt.len(),
        "optimizer should shrink ROM for x*1: opt={} no_opt={}",
        opt.len(),
        no_opt.len()
    );
}

#[test]
fn optimizer_eliminates_dead_if() {
    // if(false) removed entirely → smaller ROM
    let src = "vars { x = 0; } main { if (false) { x = 99; x = 88; x = 77; } }";
    let opt = compile(src);
    let no_opt = compile_no_opt(src);
    assert!(
        opt.len() < no_opt.len(),
        "optimizer should shrink ROM for if(false): opt={} no_opt={}",
        opt.len(),
        no_opt.len()
    );
}

#[test]
fn full_pipeline_produces_valid_rom_start() {
    // All CHIP-8 ROMs must start with a JP at 0x200
    let rom = compile("main { clear(); }");
    assert_eq!(rom[0] & 0xF0, 0x10);
}
