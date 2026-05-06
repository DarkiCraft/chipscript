#![allow(dead_code)]

use crate::ast::*;
use crate::error::*;
use std::collections::HashMap;

pub struct Codegen {
    rom: Vec<u8>,
    registers: HashMap<String, u8>,
    sprite_addrs: HashMap<String, u16>,
    sprite_heights: HashMap<String, u8>,
    fn_addrs: HashMap<String, u16>,
    next_reg: u8,
}

const ROM_START: u16 = 0x200;

impl Codegen {
    pub fn new() -> Self {
        Codegen {
            rom: Vec::new(),
            registers: HashMap::new(),
            sprite_addrs: HashMap::new(),
            sprite_heights: HashMap::new(),
            fn_addrs: HashMap::new(),
            next_reg: 0,
        }
    }

    fn current_addr(&self) -> u16 {
        ROM_START + self.rom.len() as u16
    }

    fn emit(&mut self, opcode: u16) {
        self.rom.push((opcode >> 8) as u8);
        self.rom.push((opcode & 0xFF) as u8);
    }

    fn patch(&mut self, offset: usize, opcode: u16) {
        self.rom[offset]     = (opcode >> 8) as u8;
        self.rom[offset + 1] = (opcode & 0xFF) as u8;
    }

    fn patch_jump(&mut self, offset: usize, addr: u16) {
        self.patch(offset, 0x1000 | addr);
    }

    fn reg(&self, name: &str) -> u8 {
        *self.registers.get(name)
            .unwrap_or_else(|| fail(format!("no register for variable: {}", name)))
    }

    fn alloc_reg(&mut self) -> u8 {
        let r = self.next_reg;
        self.next_reg += 1;
        if self.next_reg > 14 {
            fail(format!("out of registers!"));
        }
        r
    }

    fn free_regs(&mut self, count: u8) {
        self.next_reg -= count;
    }

    // -- ENTRY POINT ----------------------------------------------
    pub fn generate(&mut self, program: &Program) -> Vec<u8> {
        // allocate registers for variables
        for var in &program.vars {
            let r = self.alloc_reg();
            self.registers.insert(var.name.clone(), r);
        }

        // emit jump placeholder over sprites
        self.emit(0x1000);
        let jump_offset = 0usize;

        // embed sprites, record addresses and heights
        let mut pos = 2u16;
        for sprite in &program.sprites {
            let addr = ROM_START + pos;
            self.sprite_addrs.insert(sprite.name.clone(), addr);
            let bytes = self.load_sprite(sprite);
            self.sprite_heights.insert(sprite.name.clone(), bytes.len() as u8);
            pos += bytes.len() as u16;
            for b in bytes {
                self.rom.push(b);
            }
        }

        // patch jump to code start
        let code_start = self.current_addr();
        self.patch(jump_offset, 0x1000 | code_start);

        // initialize variables
        for var in &program.vars {
            let r = self.reg(&var.name);
            self.emit_load_expr(r, &var.value);
        }

        // collect function addresses first (forward declarations)
        // we need a two pass approach for functions:
        // pass 1: emit a JP over all functions, record addresses
        // pass 2: emit function bodies

        // emit jump over functions placeholder
        let fn_jump_offset = self.rom.len();
        self.emit(0x1000); // JP past all functions

        // emit each function, record its address
        for f in &program.functions {
            let addr = self.current_addr();
            self.fn_addrs.insert(f.name.clone(), addr);
            self.emit_fn(f);
        }

        // patch jump over functions
        let after_fns = self.current_addr();
        self.patch(fn_jump_offset, 0x1000 | after_fns);

        // emit main
        self.emit_block(&program.main);

        // infinite loop at end
        let end = self.current_addr();
        self.emit(0x1000 | end);

        self.rom.clone()
    }

    // -- SPRITE LOADING -------------------------------------------
    fn load_sprite(&self, sprite: &SpriteDecl) -> Vec<u8> {
        match &sprite.data {
            SpriteData::Inline(b) => b.clone(),
            SpriteData::File(path) => {
                std::fs::read(path)
                    .unwrap_or_else(|_| fail(format!("could not read sprite file: {}", path)))
            },
        }
    }

    // -- FUNCTION -------------------------------------------------
    fn emit_fn(&mut self, f: &FnDecl) {
        // allocate temp registers for args and return value
        let mut temp_names: Vec<String> = Vec::new();

        for arg in &f.args {
            let r = self.alloc_reg();
            self.registers.insert(arg.clone(), r);
            temp_names.push(arg.clone());
        }

        let ret_reg = self.alloc_reg();
        self.registers.insert(f.ret.clone(), ret_reg);
        temp_names.push(f.ret.clone());

        self.emit_block(&f.body);

        // return
        self.emit(0x00EE);

        // free temp registers
        self.free_regs(temp_names.len() as u8);
        for name in &temp_names {
            self.registers.remove(name);
        }
    }

    // -- BLOCK ----------------------------------------------------
    fn emit_block(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.emit_stmt(stmt);
        }
    }

    // -- STATEMENT ------------------------------------------------
    fn emit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Assign(name, expr) => {
                let r = self.reg(name);
                self.emit_load_expr(r, expr);
            },

            Stmt::If(cond, body, elseifs, else_body) => {
                self.emit_if(cond, body, elseifs, else_body.as_deref());
            },

            Stmt::Loop(body) => {
                let loop_start = self.current_addr();
                self.emit_block(body);
                self.emit(0x1000 | loop_start); // JP loop_start forever
            },

            Stmt::While(cond, body) => {
                self.emit_while(cond, body);
            },

            Stmt::Call(name, args) => {
                self.emit_fn_call(name, args);
            },

            Stmt::Clear => {
                self.emit(0x00E0);
            },

            Stmt::Delay(e) => {
                let r = self.alloc_reg();
                self.emit_load_expr(r, e);
                self.emit(0xF015 | ((r as u16) << 8));
                self.free_regs(1);
            },

            Stmt::Beep(e) => {
                let r = self.alloc_reg();
                self.emit_load_expr(r, e);
                self.emit(0xF018 | ((r as u16) << 8));
                self.free_regs(1);
            },
        }
    }

    // -- IF -------------------------------------------------------
    fn emit_if(
        &mut self,
        cond: &Expr,
        body: &[Stmt],
        elseifs: &[ElseIf],
        else_body: Option<&[Stmt]>,
    ) {
        let mut end_jumps: Vec<usize> = Vec::new();

        // if branch
        let skip_offset = self.emit_cond_jump(cond);
        self.emit_block(body);
        end_jumps.push(self.rom.len());
        self.emit(0x1000); // JP end placeholder
        let after_body = self.current_addr();
        self.patch_jump(skip_offset, after_body);

        // elif branches
        for elif in elseifs {
            let skip = self.emit_cond_jump(&elif.condition);
            self.emit_block(&elif.body);
            end_jumps.push(self.rom.len());
            self.emit(0x1000);
            let after = self.current_addr();
            self.patch_jump(skip, after);
        }

        // else branch
        if let Some(body) = else_body {
            self.emit_block(body);
        }

        // patch all end jumps
        let end_addr = self.current_addr();
        for offset in end_jumps {
            self.patch(offset, 0x1000 | end_addr);
        }
    }

    // -- WHILE ----------------------------------------------------
    fn emit_while(&mut self, cond: &Expr, body: &[Stmt]) {
        let loop_start = self.current_addr();
        let skip_offset = self.emit_cond_jump(cond);
        self.emit_block(body);
        self.emit(0x1000 | loop_start);
        let loop_end = self.current_addr();
        self.patch_jump(skip_offset, loop_end);
    }

    // -- CONDITIONAL JUMP -----------------------------------------
    // emits a jump that skips the body if condition is FALSE
    // returns ROM offset of the placeholder jump to patch later
    fn emit_cond_jump(&mut self, cond: &Expr) -> usize {
        match cond {
            Expr::Var(name) => {
                let r = self.reg(name);
                // SE VX, 1 -- skip if true (execute body)
                self.emit(0x3000 | ((r as u16) << 8) | 0x01);
                let offset = self.rom.len();
                self.emit(0x1000); // JP skip body
                offset
            },
            Expr::BinOp(left, op, right) => {
                self.emit_comparison_jump(left, op, right)
            },
            _ => {
                // evaluate into temp reg, then check
                let r = self.alloc_reg();
                self.emit_load_expr(r, cond);
                self.emit(0x3000 | ((r as u16) << 8) | 0x01);
                let offset = self.rom.len();
                self.emit(0x1000);
                self.free_regs(1);
                offset
            }
        }
    }

    // -- COMPARISON JUMP ------------------------------------------
    fn emit_comparison_jump(&mut self, left: &Expr, op: &Op, right: &Expr) -> usize {
        let lr = self.alloc_reg();
        let rr = self.alloc_reg();
        self.emit_load_expr(lr, left);
        self.emit_load_expr(rr, right);

        let offset = match op {
            Op::EqEq => {
                // skip if VX == VY (true) -> execute body
                self.emit(0x5000 | ((lr as u16) << 8) | ((rr as u16) << 4));
                let o = self.rom.len();
                self.emit(0x1000);
                o
            },
            Op::NotEq => {
                // skip if VX != VY (true) -> execute body
                self.emit(0x9000 | ((lr as u16) << 8) | ((rr as u16) << 4));
                let o = self.rom.len();
                self.emit(0x1000);
                o
            },
            Op::Lt => {
                // VX < VY: VX - VY causes borrow, VF = 0
                self.emit(0x8000 | ((lr as u16) << 8) | ((rr as u16) << 4) | 0x5);
                // if VF == 0 (borrow happened, so lt is true), skip next
                self.emit(0x3F00); // SE VF, 0
                let o = self.rom.len();
                self.emit(0x1000);
                o
            },
            Op::Gt => {
                // VX > VY: VY - VX causes borrow, VF = 0
                self.emit(0x8000 | ((rr as u16) << 8) | ((lr as u16) << 4) | 0x5);
                self.emit(0x3F00);
                let o = self.rom.len();
                self.emit(0x1000);
                o
            },
            Op::LtEq => {
                // VX <= VY: VY - VX no borrow, VF = 1
                self.emit(0x8000 | ((rr as u16) << 8) | ((lr as u16) << 4) | 0x5);
                self.emit(0x3F00 | 0x01); // SE VF, 1
                let o = self.rom.len();
                self.emit(0x1000);
                o
            },
            Op::GtEq => {
                // VX >= VY: VX - VY no borrow, VF = 1
                self.emit(0x8000 | ((lr as u16) << 8) | ((rr as u16) << 4) | 0x5);
                self.emit(0x3F00 | 0x01); // SE VF, 1
                let o = self.rom.len();
                self.emit(0x1000);
                o
            },
            _ => fail(format!("non-comparison op in comparison jump")),
        };

        self.free_regs(2);
        offset
    }

    // -- FUNCTION CALL --------------------------------------------
    fn emit_fn_call(&mut self, name: &str, args: &[Expr]) {
        let (params, _ret) = {
            // look up function declaration info
            // args get copied into the function's arg registers
            // we need to know which registers those are
            // since emit_fn allocated them in order, we can re-derive them
            // but its cleaner to just copy args into temp regs before call
            let addr = *self.fn_addrs.get(name)
                .unwrap_or_else(|| fail(format!("unknown function: {}", name)));
            (addr, 0u8)
        };

        // copy arg expressions into registers starting at next_reg
        // these MUST match what emit_fn allocated for the function args
        let base = self.next_reg;
        for (i, arg) in args.iter().enumerate() {
            let r = base + i as u8;
            self.emit_load_expr(r, arg);
        }

        self.emit(0x2000 | params); // CALL addr
    }

    // -- LOAD EXPR INTO REGISTER ----------------------------------
    fn emit_load_expr(&mut self, dest: u8, expr: &Expr) {
        match expr {
            Expr::Int(n) => {
                let byte = (*n as i8) as u8; // preserve two's complement
                self.emit(0x6000 | ((dest as u16) << 8) | (byte as u16));
            },

            Expr::Bool(b) => {
                let n: u16 = if *b { 1 } else { 0 };
                self.emit(0x6000 | ((dest as u16) << 8) | n);
            },

            Expr::Var(name) => {
                let src = self.reg(name);
                if src != dest {
                    self.emit(0x8000 | ((dest as u16) << 8) | ((src as u16) << 4));
                }
            },

            Expr::BinOp(left, op, right) => {
                self.emit_binop(dest, left, op, right);
            },

            Expr::Not(e) => {
                self.emit_load_expr(dest, e);
                // flip bool: XOR with 1
                // need a temp reg holding 1
                let tmp = self.alloc_reg();
                self.emit(0x6000 | ((tmp as u16) << 8) | 0x01); // tmp = 1
                self.emit(0x8000 | ((dest as u16) << 8) | ((tmp as u16) << 4) | 0x3); // VX = VX XOR tmp
                self.free_regs(1);
            },

            Expr::Call(name, args) => {
                // emit the call
                self.emit_fn_call(name, args);
                // return value is in the function's return register
                // which is at base + args.len()
                let ret_reg = self.next_reg + args.len() as u8;
                if ret_reg != dest {
                    self.emit(0x8000 | ((dest as u16) << 8) | ((ret_reg as u16) << 4));
                }
            },

            Expr::Draw(x, y, sprite_name) => {
                let xr = self.alloc_reg();
                let yr = self.alloc_reg();
                self.emit_load_expr(xr, x);
                self.emit_load_expr(yr, y);
                let addr = *self.sprite_addrs.get(sprite_name.as_str())
                    .unwrap_or_else(|| fail(format!("sprite address not found: {}", sprite_name)));
                let height = *self.sprite_heights.get(sprite_name.as_str())
                    .unwrap_or_else(|| fail(format!("sprite height not found: {}", sprite_name))) as u16;
                self.emit(0xA000 | addr);
                self.emit(0xD000 | ((xr as u16) << 8) | ((yr as u16) << 4) | height);
                // VF = collision, copy to dest
                self.emit(0x8000 | ((dest as u16) << 8) | (0xF << 4));
                self.free_regs(2);
            },

            Expr::DrawDigit(x, y, n) => {
                let xr = self.alloc_reg();
                let yr = self.alloc_reg();
                let nr = self.alloc_reg();
                self.emit_load_expr(xr, x);
                self.emit_load_expr(yr, y);
                self.emit_load_expr(nr, n);
                // FX29: set I to digit sprite for VX
                self.emit(0xF029 | ((nr as u16) << 8));
                // draw 5 rows tall (all digit sprites are 5 bytes)
                self.emit(0xD000 | ((xr as u16) << 8) | ((yr as u16) << 4) | 0x5);
                // VF = collision
                self.emit(0x8000 | ((dest as u16) << 8) | (0xF << 4));
                self.free_regs(3);
            },

            Expr::GetKey => {
                self.emit(0xF00A | ((dest as u16) << 8));
            },

            Expr::GetDelay => {
                self.emit(0xF007 | ((dest as u16) << 8));
            },

            Expr::KeyPressed(key) => {
                let kr = self.alloc_reg();
                self.emit_load_expr(kr, key);
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01); // dest = 1
                self.emit(0xE09E | ((kr as u16) << 8));           // skip if key pressed
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00); // dest = 0
                self.free_regs(1);
            },

            Expr::Rand(mask) => {
                if let Expr::Int(n) = mask.as_ref() {
                    self.emit(0xC000 | ((dest as u16) << 8) | (*n as u16));
                } else {
                    fail(format!("rand() mask must be an integer literal"));
                }
            },
        }
    }

    // -- BINOP ----------------------------------------------------
    fn emit_binop(&mut self, dest: u8, left: &Expr, op: &Op, right: &Expr) {
        let lr = self.alloc_reg();
        let rr = self.alloc_reg();
        self.emit_load_expr(lr, left);
        self.emit_load_expr(rr, right);

        match op {
            Op::Add => {
                self.emit(0x8000 | ((lr as u16) << 8) | ((rr as u16) << 4) | 0x4);
                self.emit(0x8000 | ((dest as u16) << 8) | ((lr as u16) << 4));
            },
            Op::Sub => {
                self.emit(0x8000 | ((lr as u16) << 8) | ((rr as u16) << 4) | 0x5);
                self.emit(0x8000 | ((dest as u16) << 8) | ((lr as u16) << 4));
            },
            Op::Mul => {
                let counter = self.alloc_reg();
                self.emit(0x6000 | ((dest as u16) << 8));
                self.emit(0x8000 | ((counter as u16) << 8) | ((rr as u16) << 4));
                let loop_start = self.current_addr();
                self.emit(0x3000 | ((counter as u16) << 8) | 0x00); // SE counter, 0
                let skip = self.rom.len();
                self.emit(0x1000);
                self.emit(0x8000 | ((dest as u16) << 8) | ((lr as u16) << 4) | 0x4);
                let one = self.alloc_reg();
                self.emit(0x6000 | ((one as u16) << 8) | 0x01);
                self.emit(0x8000 | ((counter as u16) << 8) | ((one as u16) << 4) | 0x5);
                self.free_regs(1); // free one
                self.emit(0x1000 | loop_start);
                let end = self.current_addr();
                self.patch(skip, 0x1000 | end);
                self.free_regs(1); // free counter
            },
            Op::Div => {
                let counter = self.alloc_reg();
                self.emit(0x6000 | ((counter as u16) << 8));
                let loop_start = self.current_addr();
                let tmp = self.alloc_reg();
                self.emit(0x8000 | ((tmp as u16) << 8) | ((lr as u16) << 4));
                self.emit(0x8000 | ((tmp as u16) << 8) | ((rr as u16) << 4) | 0x5);
                self.free_regs(1); // free tmp
                self.emit(0x3F00); // SE VF, 0 (borrow = lr < rr, done)
                let skip = self.rom.len();
                self.emit(0x1000);
                self.emit(0x8000 | ((lr as u16) << 8) | ((rr as u16) << 4) | 0x5);
                let one = self.alloc_reg();
                self.emit(0x6000 | ((one as u16) << 8) | 0x01);
                self.emit(0x8000 | ((counter as u16) << 8) | ((one as u16) << 4) | 0x4);
                self.free_regs(1); // free one
                self.emit(0x1000 | loop_start);
                let end = self.current_addr();
                self.patch(skip, 0x1000 | end);
                self.emit(0x8000 | ((dest as u16) << 8) | ((counter as u16) << 4));
                self.free_regs(1); // free counter
            },
            Op::Mod => {
                let loop_start = self.current_addr();
                let tmp = self.alloc_reg();
                self.emit(0x8000 | ((tmp as u16) << 8) | ((lr as u16) << 4));
                self.emit(0x8000 | ((tmp as u16) << 8) | ((rr as u16) << 4) | 0x5);
                self.free_regs(1); // free tmp
                self.emit(0x3F00); // SE VF, 0 (done)
                let skip = self.rom.len();
                self.emit(0x1000);
                self.emit(0x8000 | ((lr as u16) << 8) | ((rr as u16) << 4) | 0x5);
                self.emit(0x1000 | loop_start);
                let end = self.current_addr();
                self.patch(skip, 0x1000 | end);
                self.emit(0x8000 | ((dest as u16) << 8) | ((lr as u16) << 4));
            },
            Op::EqEq => {
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
                self.emit(0x9000 | ((lr as u16) << 8) | ((rr as u16) << 4));
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01);
            },
            Op::NotEq => {
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
                self.emit(0x5000 | ((lr as u16) << 8) | ((rr as u16) << 4));
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01);
            },
            Op::Lt => {
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
                self.emit(0x8000 | ((lr as u16) << 8) | ((rr as u16) << 4) | 0x5);
                self.emit(0x4F00); // SNE VF, 0
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01);
            },
            Op::Gt => {
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
                self.emit(0x8000 | ((rr as u16) << 8) | ((lr as u16) << 4) | 0x5);
                self.emit(0x4F00);
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01);
            },
            Op::LtEq => {
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
                self.emit(0x8000 | ((rr as u16) << 8) | ((lr as u16) << 4) | 0x5);
                self.emit(0x3F00 | 0x01); // SE VF, 1
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01);
            },
            Op::GtEq => {
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
                self.emit(0x8000 | ((lr as u16) << 8) | ((rr as u16) << 4) | 0x5);
                self.emit(0x3F00 | 0x01);
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01);
            },
            Op::And => {
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
                self.emit(0x4000 | ((lr as u16) << 8) | 0x01); // SNE VL, 1
                self.emit(0x4000 | ((rr as u16) << 8) | 0x01); // SNE VR, 1
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01);
            },
            Op::Or => {
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
                self.emit(0x3000 | ((lr as u16) << 8) | 0x01); // SE VL, 1
                self.emit(0x3000 | ((rr as u16) << 8) | 0x01); // SE VR, 1
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01);
            },
        }

        self.free_regs(2);
    }
}