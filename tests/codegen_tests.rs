#![cfg(test)]

mod common;

use common::compile;
use common::compile_no_opt;

const ROM_START: usize = 0x200;

// Helper: extract instruction at a byte offset from ROM start
fn instr_at(rom: &[u8], byte_offset: usize) -> u16 {
    let i = byte_offset;
    ((rom[i] as u16) << 8) | rom[i + 1] as u16
}

// --- ROM always starts at 0x200 ----------------------------------------

#[test]
fn rom_is_nonempty() {
    let rom = compile("main { clear(); }");
    assert!(!rom.is_empty());
}

#[test]
fn first_instruction_is_jump() {
    // ROM always starts with JP somewhere (over sprites/functions)
    let rom = compile("main { clear(); }");
    assert_eq!(rom[0] & 0xF0, 0x10, "first byte should be JP (0x1xxx)");
}

// --- Clear --------------------------------------------------------------

#[test]
fn clear_emits_00e0() {
    let rom = compile_no_opt("main { clear(); }");
    // Find 0x00E0 in the ROM
    let found = rom.windows(2).any(|w| w[0] == 0x00 && w[1] == 0xE0);
    assert!(found, "0x00E0 (CLS) not found in ROM");
}

// --- Load immediate -----------------------------------------------------

#[test]
fn load_immediate_zero() {
    // vars { x = 0; } → 6X00
    let rom = compile_no_opt("vars { x = 0; } main { clear(); }");
    let found = rom
        .windows(2)
        .any(|w| (w[0] & 0xF0) == 0x60 && w[1] == 0x00);
    assert!(found, "6X00 not found");
}

#[test]
fn load_immediate_nonzero() {
    // vars { x = 42; } → 6X2A
    let rom = compile_no_opt("vars { x = 42; } main { clear(); }");
    let found = rom.windows(2).any(|w| (w[0] & 0xF0) == 0x60 && w[1] == 42);
    assert!(found, "6X2A not found");
}

// --- Delay / Beep -------------------------------------------------------

#[test]
fn delay_emits_fx15() {
    let rom = compile_no_opt("main { delay(10); }");
    let found = rom
        .windows(2)
        .any(|w| (w[0] & 0x0F) == 0x00 && w[1] == 0x15);
    assert!(found, "FX15 not found");
}

#[test]
fn beep_emits_fx18() {
    let rom = compile_no_opt("main { beep(5); }");
    let found = rom
        .windows(2)
        .any(|w| (w[0] & 0x0F) == 0x00 && w[1] == 0x18);
    assert!(found, "FX18 not found");
}

// --- GetKey / GetDelay --------------------------------------------------

#[test]
fn getkey_emits_fx0a() {
    let rom = compile_no_opt("vars { k = 0; } main { k = getkey(); }");
    let found = rom
        .windows(2)
        .any(|w| (w[0] & 0xF0) == 0xF0 && w[1] == 0x0A);
    assert!(found, "Fx0A not found");
}

#[test]
fn getdelay_emits_fx07() {
    let rom = compile_no_opt("vars { t = 0; } main { t = getdelay(); }");
    let found = rom
        .windows(2)
        .any(|w| (w[0] & 0xF0) == 0xF0 && w[1] == 0x07);
    assert!(found, "Fx07 not found");
}

// --- KeyPressed ---------------------------------------------------------

#[test]
fn keypressed_emits_ex9e() {
    let rom = compile_no_opt("vars { b = false; } main { b = keypressed(5); }");
    let found = rom
        .windows(2)
        .any(|w| (w[0] & 0xF0) == 0xE0 && w[1] == 0x9E);
    assert!(found, "EX9E not found");
}

// --- Rand ---------------------------------------------------------------

#[test]
fn rand_emits_cxkk() {
    let rom = compile_no_opt("vars { r = 0; } main { r = rand(0xFF); }");
    let found = rom
        .windows(2)
        .any(|w| (w[0] & 0xF0) == 0xC0 && w[1] == 0xFF);
    assert!(found, "CX FF not found");
}

// --- Draw ---------------------------------------------------------------

#[test]
fn draw_emits_dxyn() {
    let rom =
        compile_no_opt("sprites { b = [0xFF]; } vars { h = false; } main { h = draw(0, 0, b); }");
    let found = rom.windows(2).any(|w| (w[0] & 0xF0) == 0xD0);
    assert!(found, "DXYN not found");
}

#[test]
fn draw_sprite_height_in_nibble() {
    // 3-row sprite → DXY3
    let rom = compile_no_opt(
        "sprites { s = [0xFF, 0xFF, 0xFF]; } vars { h = false; } main { h = draw(0, 0, s); }",
    );
    let found = rom
        .windows(2)
        .any(|w| (w[0] & 0xF0) == 0xD0 && (w[1] & 0x0F) == 3);
    assert!(found, "DXYN with n=3 not found");
}

// --- DrawDigit ----------------------------------------------------------

#[test]
fn drawdigit_emits_fx29() {
    let rom = compile_no_opt("vars { h = false; } main { h = drawdigit(0, 0, 5); }");
    // let found = rom
    //     .windows(2)
    //     .any(|w| (w[0] & 0x0F) == 0x00 && w[1] == 0x29);
    let found = rom.windows(2).any(|w| w[0] & 0xF0 == 0xF0 && w[1] == 0x29);
    assert!(found, "FX29 not found");
}

// --- Function call ------------------------------------------------------

#[test]
fn function_call_emits_2nnn() {
    let rom = compile_no_opt("fn f() -> r { r = 0; } main { f(); }");
    let found = rom.windows(2).any(|w| (w[0] & 0xF0) == 0x20);
    assert!(found, "2NNN (CALL) not found");
}

#[test]
fn function_return_emits_00ee() {
    let rom = compile_no_opt("fn f() -> r { r = 0; } main { f(); }");
    let found = rom.windows(2).any(|w| w[0] == 0x00 && w[1] == 0xEE);
    assert!(found, "00EE (RET) not found");
}

// --- Infinite loop at end ----------------------------------------------

#[test]
fn infinite_loop_at_end() {
    let rom = compile_no_opt("main { clear(); }");
    // Last 2 bytes should be JP to their own address (1NNN where NNN = addr of those bytes)
    let last_addr = (ROM_START + rom.len() - 2) as u16;
    let last_instr = instr_at(&rom, rom.len() - 2);
    assert_eq!(
        last_instr,
        0x1000 | last_addr,
        "last instruction should be JP to itself"
    );
}

// --- ROM size sanity ----------------------------------------------------

#[test]
fn sprite_bytes_in_rom() {
    // A 2-byte sprite should appear verbatim in the ROM
    let rom = compile_no_opt(
        "sprites { b = [0xAB, 0xCD]; } vars { h = false; } main { h = draw(0,0,b); }",
    );
    let found = rom.windows(2).any(|w| w[0] == 0xAB && w[1] == 0xCD);
    assert!(found, "sprite bytes not found in ROM");
}

#[test]
fn no_opt_larger_than_opt() {
    let src = "vars { x = 0; } main { x = 3 + 4; x = x * 1; }";
    let rom_opt = compile(src);
    let rom_no_opt = compile_no_opt(src);
    // Optimized ROM should be same size or smaller
    assert!(
        rom_opt.len() <= rom_no_opt.len(),
        "optimized ROM ({}) should not be larger than unoptimized ({})",
        rom_opt.len(),
        rom_no_opt.len()
    );
}
