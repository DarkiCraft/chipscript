use crate::ast::*;
use crate::error::{CODEGEN, fail};
use crate::ir::{IrOp, Quad};
use std::collections::{HashMap, HashSet};

const ROM_START: u16 = 0x200;
const ROM_MAX: usize = 3584; // 0x200..0xFFF

pub struct Codegen {
    rom: Vec<u8>,
    ir: Vec<Quad>,
    ir_label: u32,
    registers: HashMap<String, u8>,
    sprite_addrs: HashMap<String, u16>,
    sprite_heights: HashMap<String, u8>,
    fn_addrs: HashMap<String, u16>,
    fn_depths: HashMap<String, u8>, // function name → call depth (main=0)
    next_reg: u8,
    call_fixups: Vec<(usize, String)>,
    save_area_addrs: Vec<u16>,
    current_depth: u8,
    ng: usize,
}

impl Codegen {
    pub fn new() -> Self {
        Codegen {
            rom: Vec::new(),
            ir: Vec::new(),
            ir_label: 0,
            registers: HashMap::new(),
            sprite_addrs: HashMap::new(),
            sprite_heights: HashMap::new(),
            fn_addrs: HashMap::new(),
            fn_depths: HashMap::new(),
            next_reg: 1,
            call_fixups: Vec::new(),
            save_area_addrs: Vec::new(),
            current_depth: 0,
            ng: 0,
        }
    }

    // -- IR HELPERS -----------------------------------------------
    pub fn get_ir(&self) -> &[Quad] {
        &self.ir
    }

    fn quad(&mut self, op: IrOp, arg1: Option<&str>, arg2: Option<&str>, result: Option<&str>) {
        self.ir.push(Quad::new(op, arg1, arg2, result));
    }

    fn fresh_label(&mut self) -> String {
        let l = format!("L{}", self.ir_label);
        self.ir_label += 1;
        l
    }

    fn current_addr(&self) -> u16 {
        ROM_START + self.rom.len() as u16
    }

    fn emit(&mut self, opcode: u16) {
        self.rom.push((opcode >> 8) as u8);
        self.rom.push((opcode & 0xFF) as u8);
    }

    fn patch(&mut self, offset: usize, opcode: u16) {
        self.rom[offset] = (opcode >> 8) as u8;
        self.rom[offset + 1] = (opcode & 0xFF) as u8;
    }

    fn patch_jump(&mut self, offset: usize, addr: u16) {
        self.patch(offset, 0x1000 | addr);
    }

    fn patch_call(&mut self, offset: usize, addr: u16) {
        self.patch(offset, 0x2000 | addr);
    }

    /// V0 scratch, V1..V{work_end} = work region, V{work_end+1}..V14 = globals, VF flag
    fn work_end(&self) -> u8 {
        (14 - self.ng) as u8
    }

    fn reg(&self, name: &str) -> u8 {
        *self
            .registers
            .get(name)
            .unwrap_or_else(|| fail(CODEGEN, format!("no register for variable '{}'", name)))
    }

    /// Allocate a register in the work region (V1..V{work_end}).
    fn alloc_reg(&mut self) -> u8 {
        let max = self.work_end();
        if self.next_reg > max {
            fail(
                CODEGEN,
                format!(
                    "out of registers (work region V1..V{}, max call depth may overflow)",
                    max
                ),
            );
        }
        let r = self.next_reg;
        self.next_reg += 1;
        r
    }

    fn free_regs(&mut self, count: u8) {
        self.next_reg -= count;
    }

    // -- CALL GRAPH & DEPTHS --------------------------------------
    fn compute_depths(&self, program: &Program) -> HashMap<String, u8> {
        let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
        for f in &program.functions {
            let callees = self.collect_calls(&f.body);
            graph.insert(f.name.as_str(), callees);
        }
        let main_callees = self.collect_calls(&program.main);
        graph.insert("main", main_callees);

        let mut depths: HashMap<String, u8> = HashMap::new();
        depths.insert("main".to_string(), 0);
        for f in &program.functions {
            self.depth_dfs(f.name.as_str(), &graph, &mut depths, &mut HashSet::new());
        }
        depths
    }

    fn depth_dfs<'a>(
        &self,
        node: &'a str,
        graph: &HashMap<&'a str, Vec<&'a str>>,
        depths: &mut HashMap<String, u8>,
        visiting: &mut HashSet<&'a str>,
    ) {
        if depths.contains_key(node) {
            return;
        }
        // If this is a leaf function with no callees (but IS in graph),
        // then visiting doesn't contain it yet.
        // For unknown functions (not in graph, e.g. main), depth is given.
        if !graph.contains_key(node) {
            return;
        }
        visiting.insert(node);
        let callees = &graph[node];
        let mut max_d = 0u8;
        for &callee in callees {
            if callee == node || visiting.contains(callee) {
                continue;
            }
            self.depth_dfs(callee, graph, depths, visiting);
            if let Some(&d) = depths.get(callee) {
                if d + 1 > max_d {
                    max_d = d + 1;
                }
            }
        }
        visiting.remove(node);
        depths.insert(node.to_string(), max_d);
    }

    fn collect_calls<'a>(&self, stmts: &'a [Stmt]) -> Vec<&'a str> {
        let mut v = Vec::new();
        self.collect_calls_stmts(stmts, &mut v);
        v
    }

    fn collect_calls_stmts<'a>(&self, stmts: &'a [Stmt], acc: &mut Vec<&'a str>) {
        for s in stmts {
            self.collect_calls_stmt(s, acc);
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

    // -- ENTRY POINT ----------------------------------------------
    pub fn generate(&mut self, program: &Program) -> Vec<u8> {
        self.ng = program.vars.len();
        let depths = self.compute_depths(program);
        self.fn_depths = depths;

        let max_depth = self.fn_depths.values().max().copied().unwrap_or(0);
        let save_size = (self.work_end() + 1) as usize; // V0..V{work_end}

        // -- layout --------------------------------------------------
        // 0x200:  JP code_start
        //        save areas (one per depth level, zeroed)
        //        sprite data
        //        pad to even
        // code:  var init
        //        JP after_fns
        //        function bodies (with fixup CALLs)
        // after: main
        //        JP self (end)

        // Boot jump (patched later)
        let boot_jump = self.rom.len();
        self.emit(0x1000);

        // Save areas (one per depth level d = 0..max_depth)
        let num_save_areas = max_depth as usize + 1;
        for _ in 0..num_save_areas {
            let addr = self.current_addr();
            self.save_area_addrs.push(addr);
            for _ in 0..save_size {
                self.rom.push(0);
            }
        }

        // Sprites
        for sprite in &program.sprites {
            let addr = self.current_addr();
            self.sprite_addrs.insert(sprite.name.clone(), addr);
            let bytes = self.load_sprite(sprite);
            self.sprite_heights
                .insert(sprite.name.clone(), bytes.len() as u8);
            for b in bytes {
                self.rom.push(b);
            }
            // IR: record sprite location
            self.quad(
                IrOp::SpriteData,
                Some(&sprite.name),
                Some(&addr.to_string()),
                None,
            );
        }

        // Align to even address (Cowgod: instructions must be at even addresses)
        if self.rom.len() % 2 != 0 {
            self.rom.push(0);
        }

        // Patch boot jump
        let code_start = self.current_addr();
        self.patch_jump(boot_jump, code_start);

        // Assign global registers (top-down, V{15-ng}..V14)
        for (i, var) in program.vars.iter().enumerate() {
            let reg = (15 - self.ng + i) as u8;
            self.registers.insert(var.name.clone(), reg);
        }

        // Initialize global variables
        for var in &program.vars {
            let r = self.reg(&var.name);
            self.emit_load_expr(r, &var.value);
        }

        // Jump over function bodies (placeholder)
        let fn_jump_offset = self.rom.len();
        self.emit(0x1000);

        // Emit all function bodies
        for f in &program.functions {
            let addr = self.current_addr();
            self.fn_addrs.insert(f.name.clone(), addr);
            let depth = *self.fn_depths.get(&f.name).unwrap_or(&0);
            self.emit_fn(f, depth);
        }

        // Patch jump over functions
        let after_fns = self.current_addr();
        self.patch_jump(fn_jump_offset, after_fns);

        // Emit main body
        self.current_depth = 0;
        self.emit_block(&program.main);

        // Endless loop
        let end = self.current_addr();
        self.emit(0x1000 | end);

        // Backpatch all call fixups
        let fixups = self.call_fixups.clone();
        let fn_addrs = self.fn_addrs.clone();
        for &(offset, ref fn_name) in &fixups {
            if let Some(&addr) = fn_addrs.get(fn_name.as_str()) {
                self.patch_call(offset, addr);
            } else {
                fail(
                    CODEGEN,
                    format!(
                        "call to unknown function '{}' (no matching declaration)",
                        fn_name
                    ),
                );
            }
        }

        // ROM size check
        if self.rom.len() > ROM_MAX {
            fail(
                CODEGEN,
                format!(
                    "program too large: {} bytes (CHIP-8 memory is 0x200–0xFFF = 3584 bytes max)",
                    self.rom.len()
                ),
            );
        }

        self.rom.clone()
    }

    // -- SPRITE LOADING -------------------------------------------
    fn load_sprite(&self, sprite: &SpriteDecl) -> Vec<u8> {
        match &sprite.data {
            SpriteData::Inline(b) => b.clone(),
            SpriteData::File(path) => std::fs::read(path).unwrap_or_else(|_| {
                fail(CODEGEN, format!("could not read sprite file: '{}'", path))
            }),
        }
    }

    // -- FUNCTION -------------------------------------------------
    fn emit_fn(&mut self, f: &FnDecl, depth: u8) {
        self.quad(IrOp::Label, Some(&f.name), None, None);

        // Set current depth so calls inside this function use the right save area
        self.current_depth = depth;

        // Allocate frame registers starting at V1
        for (i, arg) in f.args.iter().enumerate() {
            let r = (i + 1) as u8;
            self.registers.insert(arg.clone(), r);
        }
        let ret_reg = (f.args.len() + 1) as u8;
        self.registers.insert(f.ret.clone(), ret_reg);
        self.next_reg = ret_reg + 1;

        self.emit_block(&f.body);

        // Return
        self.quad(IrOp::Return, None, None, None);
        self.emit(0x00EE);

        // Free frame registers
        for arg in &f.args {
            self.registers.remove(arg);
        }
        self.registers.remove(&f.ret);
        self.next_reg = 1;
    }

    // -- CALL -------------------------------------------------
    fn emit_call(&mut self, name: &str, args: &[Expr], dest: Option<u8>, depth: u8) {
        let argc = args.len() as u8;
        let we = self.work_end();

        // 1. Save V0..V{we} to save area for this depth
        let save_addr = self.save_area_addrs[depth as usize];
        self.quad(IrOp::Call, Some(name), Some(&argc.to_string()), None);
        self.emit(0xA000 | save_addr);
        self.emit(0xF000 | ((we as u16) << 8) | 0x55);

        // 2. Evaluate args into V1..V{argc}
        let saved_next = self.next_reg;
        self.next_reg = 1;
        for (i, arg) in args.iter().enumerate() {
            let r = (i + 1) as u8;
            self.emit_load_expr(r, arg);
            self.next_reg = std::cmp::max(self.next_reg, (i + 2) as u8);
        }
        self.next_reg = std::cmp::max(self.next_reg, argc + 2);

        // 3. CALL (placeholder if forward reference)
        if let Some(&addr) = self.fn_addrs.get(name) {
            self.emit(0x2000 | addr);
        } else {
            let offset = self.rom.len();
            self.emit(0x2000);
            self.call_fixups.push((offset, name.to_string()));
        }

        // 4. Copy return value to VF (which is not in the save/restore region)
        let ret_reg = argc + 1;
        self.emit(0x8000 | ((0xF as u16) << 8) | ((ret_reg as u16) << 4));

        // 5. Restore V0..V{we}
        self.emit(0xA000 | save_addr);
        self.emit(0xF000 | ((we as u16) << 8) | 0x65);

        // 6. Copy VF → dest if needed
        if let Some(d) = dest {
            self.emit(0x8000 | ((d as u16) << 8) | ((0xF as u16) << 4));
        }

        self.next_reg = saved_next;
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
            }

            Stmt::If(cond, body, elseifs, else_body) => {
                self.emit_if(cond, body, elseifs, else_body.as_deref());
            }

            Stmt::Loop(body) => {
                let lbl = self.fresh_label();
                self.quad(IrOp::Label, Some(&lbl), None, None);
                let loop_start = self.current_addr();
                self.emit_block(body);
                self.quad(IrOp::Jump, Some(&lbl), None, None);
                self.emit(0x1000 | loop_start);
            }

            Stmt::While(cond, body) => {
                self.emit_while(cond, body);
            }

            Stmt::Call(name, args) => {
                self.emit_call(name, args, None, self.current_depth);
            }

            Stmt::Clear => {
                self.quad(IrOp::Clear, None, None, None);
                self.emit(0x00E0);
            }

            Stmt::Delay(e) => {
                self.quad(IrOp::SetDelay, Some("V0"), None, None);
                self.emit_load_expr(0, e);
                self.emit(0xF015);
            }

            Stmt::Beep(e) => {
                self.quad(IrOp::SetSound, Some("V0"), None, None);
                self.emit_load_expr(0, e);
                self.emit(0xF018);
            }
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
        let end_lbl = self.fresh_label();
        let mut end_jumps: Vec<usize> = Vec::new();

        // if branch
        let skip_lbl = self.fresh_label();
        self.quad(IrOp::JumpFalse, Some("cond"), Some(&skip_lbl), None);
        let skip_offset = self.emit_cond_jump(cond);
        self.emit_block(body);
        self.quad(IrOp::Jump, Some(&end_lbl), None, None);
        end_jumps.push(self.rom.len());
        self.emit(0x1000);
        let after_body = self.current_addr();
        self.quad(IrOp::Label, Some(&skip_lbl), None, None);
        self.patch_jump(skip_offset, after_body);

        // elif branches
        for elif in elseifs {
            let elif_skip = self.fresh_label();
            self.quad(IrOp::JumpFalse, Some("cond"), Some(&elif_skip), None);
            let skip = self.emit_cond_jump(&elif.condition);
            self.emit_block(&elif.body);
            self.quad(IrOp::Jump, Some(&end_lbl), None, None);
            end_jumps.push(self.rom.len());
            self.emit(0x1000);
            let after = self.current_addr();
            self.quad(IrOp::Label, Some(&elif_skip), None, None);
            self.patch_jump(skip, after);
        }

        // else branch
        if let Some(body) = else_body {
            self.emit_block(body);
        }

        // patch all end jumps
        self.quad(IrOp::Label, Some(&end_lbl), None, None);
        let end_addr = self.current_addr();
        for off in end_jumps {
            self.patch_jump(off, end_addr);
        }
    }

    // -- WHILE ----------------------------------------------------
    fn emit_while(&mut self, cond: &Expr, body: &[Stmt]) {
        let loop_lbl = self.fresh_label();
        let end_lbl = self.fresh_label();
        self.quad(IrOp::Label, Some(&loop_lbl), None, None);
        let loop_start = self.current_addr();
        self.quad(IrOp::JumpFalse, Some("cond"), Some(&end_lbl), None);
        let skip_offset = self.emit_cond_jump(cond);
        self.emit_block(body);
        self.quad(IrOp::Jump, Some(&loop_lbl), None, None);
        self.emit(0x1000 | loop_start);
        let loop_end = self.current_addr();
        self.quad(IrOp::Label, Some(&end_lbl), None, None);
        self.patch_jump(skip_offset, loop_end);
    }

    // -- CONDITIONAL JUMP -----------------------------------------
    fn emit_cond_jump(&mut self, cond: &Expr) -> usize {
        match cond {
            Expr::Var(name) => {
                let r = self.reg(name);
                self.emit(0x3000 | ((r as u16) << 8) | 0x01);
                let offset = self.rom.len();
                self.emit(0x1000);
                offset
            }
            Expr::BinOp(left, op, right) => match op {
                Op::EqEq | Op::NotEq | Op::Lt | Op::Gt | Op::LtEq | Op::GtEq => {
                    self.emit_comparison_jump(left, op, right)
                }
                _ => {
                    let r = self.alloc_reg();
                    self.emit_load_expr(r, cond);
                    self.emit(0x3000 | ((r as u16) << 8) | 0x01);
                    let offset = self.rom.len();
                    self.emit(0x1000);
                    self.free_regs(1);
                    offset
                }
            },
            _ => {
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
                self.emit(0x5000 | ((lr as u16) << 8) | ((rr as u16) << 4));
                let o = self.rom.len();
                self.emit(0x1000);
                o
            }
            Op::NotEq => {
                self.emit(0x9000 | ((lr as u16) << 8) | ((rr as u16) << 4));
                let o = self.rom.len();
                self.emit(0x1000);
                o
            }
            Op::Lt => {
                // VX < VY: VX - VY causes borrow, VF = 0
                self.emit(0x8000 | ((lr as u16) << 8) | ((rr as u16) << 4) | 0x5);
                self.emit(0x3F00);
                let o = self.rom.len();
                self.emit(0x1000);
                o
            }
            Op::Gt => {
                // VX > VY: VY - VX causes borrow, VF = 0
                self.emit(0x8000 | ((rr as u16) << 8) | ((lr as u16) << 4) | 0x5);
                self.emit(0x3F00);
                let o = self.rom.len();
                self.emit(0x1000);
                o
            }
            Op::LtEq => {
                // VX <= VY: VY - VX no borrow, VF = 1
                self.emit(0x8000 | ((rr as u16) << 8) | ((lr as u16) << 4) | 0x5);
                self.emit(0x3F00 | 0x01);
                let o = self.rom.len();
                self.emit(0x1000);
                o
            }
            Op::GtEq => {
                // VX >= VY: VX - VY no borrow, VF = 1
                self.emit(0x8000 | ((lr as u16) << 8) | ((rr as u16) << 4) | 0x5);
                self.emit(0x3F00 | 0x01);
                let o = self.rom.len();
                self.emit(0x1000);
                o
            }
            _ => fail(
                CODEGEN,
                "internal error: non-comparison op passed to emit_comparison_jump",
            ),
        };

        self.free_regs(2);
        offset
    }

    // -- LOAD EXPR INTO REGISTER ----------------------------------
    fn emit_load_expr(&mut self, dest: u8, expr: &Expr) {
        let dest_s = format!("V{:X}", dest);
        match expr {
            Expr::Int(n) => {
                self.quad(IrOp::LoadImm, Some(&n.to_string()), None, Some(&dest_s));
                self.emit(0x6000 | ((dest as u16) << 8) | (*n as u16));
            }

            Expr::Bool(b) => {
                let n: u16 = if *b { 1 } else { 0 };
                self.quad(IrOp::LoadImm, Some(&n.to_string()), None, Some(&dest_s));
                self.emit(0x6000 | ((dest as u16) << 8) | n);
            }

            Expr::Var(name) => {
                let src = self.reg(name);
                if src != dest {
                    self.quad(IrOp::Copy, Some(name), None, Some(&dest_s));
                    self.emit(0x8000 | ((dest as u16) << 8) | ((src as u16) << 4));
                }
            }

            Expr::BinOp(left, op, right) => {
                self.emit_binop(dest, left, op, right);
            }

            Expr::Not(e) => {
                // load operand into dest, then XOR with 1 via V0 scratch
                self.emit_load_expr(dest, e);
                self.quad(IrOp::Not, Some(&dest_s), None, Some(&dest_s));
                self.emit(0x6000 | (0 << 8) | 0x01); // LD V0, 1
                self.emit(0x8000 | ((dest as u16) << 8) | (0 << 4) | 0x3); // XOR dest, V0
            }

            Expr::Call(name, args) => {
                self.emit_call(name, args, Some(dest), self.current_depth);
            }

            Expr::Draw(x, y, sprite_name) => {
                let xr = self.alloc_reg();
                let yr = self.alloc_reg();
                self.quad(
                    IrOp::Draw,
                    Some(&format!("V{:X}", xr)),
                    Some(&format!("V{:X}", yr)),
                    Some(sprite_name),
                );
                self.emit_load_expr(xr, x);
                self.emit_load_expr(yr, y);
                let addr = *self
                    .sprite_addrs
                    .get(sprite_name.as_str())
                    .unwrap_or_else(|| {
                        fail(
                            CODEGEN,
                            format!("sprite '{}' has no recorded address", sprite_name),
                        )
                    });
                let height = *self
                    .sprite_heights
                    .get(sprite_name.as_str())
                    .unwrap_or_else(|| {
                        fail(
                            CODEGEN,
                            format!("sprite '{}' has no recorded height", sprite_name),
                        )
                    }) as u16;
                self.emit(0xA000 | addr);
                self.emit(0xD000 | ((xr as u16) << 8) | ((yr as u16) << 4) | height);
                self.emit(0x8000 | ((dest as u16) << 8) | (0xF << 4));
                self.free_regs(2);
            }

            Expr::DrawDigit(x, y, n) => {
                let xr = self.alloc_reg();
                let yr = self.alloc_reg();
                let nr = self.alloc_reg();
                self.quad(
                    IrOp::DrawDigit,
                    Some(&format!("V{:X}", xr)),
                    Some(&format!("V{:X}", yr)),
                    Some(&format!("V{:X}", nr)),
                );
                self.emit_load_expr(xr, x);
                self.emit_load_expr(yr, y);
                self.emit_load_expr(nr, n);
                // Mask digit value to 0–15 using V0 scratch
                self.emit(0x6000 | (0 << 8) | 0x0F); // LD V0, 0x0F
                self.emit(0x8000 | ((nr as u16) << 8) | (0 << 4) | 0x2); // AND nr, V0
                self.emit(0xF029 | ((nr as u16) << 8));
                self.emit(0xD000 | ((xr as u16) << 8) | ((yr as u16) << 4) | 0x5);
                self.emit(0x8000 | ((dest as u16) << 8) | (0xF << 4));
                self.free_regs(3);
            }

            Expr::GetKey => {
                self.quad(IrOp::GetKey, None, None, Some(&dest_s));
                self.emit(0xF00A | ((dest as u16) << 8));
            }

            Expr::GetDelay => {
                self.quad(IrOp::GetDelay, None, None, Some(&dest_s));
                self.emit(0xF007 | ((dest as u16) << 8));
            }

            Expr::KeyPressed(key) => {
                let kr = self.alloc_reg();
                self.quad(
                    IrOp::KeyPressed,
                    Some(&format!("V{:X}", kr)),
                    None,
                    Some(&dest_s),
                );
                self.emit_load_expr(kr, key);
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01);
                self.emit(0xE09E | ((kr as u16) << 8));
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
                self.free_regs(1);
            }

            Expr::Rand(mask) => {
                if let Expr::Int(n) = mask.as_ref() {
                    self.quad(IrOp::Rand, Some(&n.to_string()), None, Some(&dest_s));
                    self.emit(0xC000 | ((dest as u16) << 8) | (*n as u16));
                } else {
                    fail(
                        CODEGEN,
                        "rand() mask must be an integer literal (this should have been caught by the analyzer)",
                    );
                }
            }
        }
    }

    fn emit_binop(&mut self, dest: u8, left: &Expr, op: &Op, right: &Expr) {
        let lr = self.alloc_reg();
        let rr = self.alloc_reg();
        let dest_s = format!("V{:X}", dest);
        let lr_s = format!("V{:X}", lr);
        let rr_s = format!("V{:X}", rr);
        self.emit_load_expr(lr, left);
        self.emit_load_expr(rr, right);

        match op {
            Op::Add => {
                self.quad(IrOp::Add, Some(&lr_s), Some(&rr_s), Some(&dest_s));
                self.emit(0x8000 | ((lr as u16) << 8) | ((rr as u16) << 4) | 0x4);
                self.emit(0x8000 | ((dest as u16) << 8) | ((lr as u16) << 4));
            }
            Op::Sub => {
                self.quad(IrOp::Sub, Some(&lr_s), Some(&rr_s), Some(&dest_s));
                self.emit(0x8000 | ((lr as u16) << 8) | ((rr as u16) << 4) | 0x5);
                self.emit(0x8000 | ((dest as u16) << 8) | ((lr as u16) << 4));
            }
            Op::Mul => {
                self.quad(IrOp::Mul, Some(&lr_s), Some(&rr_s), Some(&dest_s));
                let counter = self.alloc_reg();
                self.emit(0x6000 | ((dest as u16) << 8)); // dest = 0
                self.emit(0x8000 | ((counter as u16) << 8) | ((rr as u16) << 4)); // counter = rr
                let loop_start = self.current_addr();
                self.emit(0x3000 | ((counter as u16) << 8) | 0x00); // SE counter, 0
                let skip = self.rom.len();
                self.emit(0x1000);
                self.emit(0x8000 | ((dest as u16) << 8) | ((lr as u16) << 4) | 0x4); // dest += lr
                self.emit(0x6000 | (0 << 8) | 0x01); // LD V0, 1
                self.emit(0x8000 | ((counter as u16) << 8) | (0 << 4) | 0x5); // counter -= 1 (via V0)
                self.emit(0x1000 | loop_start);
                let end = self.current_addr();
                self.patch(skip, 0x1000 | end);
                self.free_regs(1);
            }
            Op::Div => {
                self.quad(IrOp::Div, Some(&lr_s), Some(&rr_s), Some(&dest_s));
                if rr == 0 {
                    fail(CODEGEN, "division by zero (constant zero divisor)");
                }
                let counter = self.alloc_reg();
                self.emit(0x6000 | ((counter as u16) << 8)); // counter = 0
                let loop_start = self.current_addr();
                let tmp = self.alloc_reg();
                self.emit(0x8000 | ((tmp as u16) << 8) | ((lr as u16) << 4)); // tmp = lr
                self.emit(0x8000 | ((tmp as u16) << 8) | ((rr as u16) << 4) | 0x5); // tmp -= rr
                self.free_regs(1);
                self.emit(0x3F00); // SE VF, 0 (borrow = true, lr < rr → done)
                let skip = self.rom.len();
                self.emit(0x1000);
                self.emit(0x8000 | ((lr as u16) << 8) | ((rr as u16) << 4) | 0x5); // lr -= rr
                self.emit(0x6000 | (0 << 8) | 0x01); // LD V0, 1
                self.emit(0x8000 | ((counter as u16) << 8) | (0 << 4) | 0x4); // counter += 1 (via V0)
                self.emit(0x1000 | loop_start);
                let end = self.current_addr();
                self.patch(skip, 0x1000 | end);
                self.emit(0x8000 | ((dest as u16) << 8) | ((counter as u16) << 4));
                self.free_regs(1);
            }
            Op::Mod => {
                self.quad(IrOp::Mod, Some(&lr_s), Some(&rr_s), Some(&dest_s));
                if rr == 0 {
                    fail(CODEGEN, "modulo by zero (constant zero divisor)");
                }
                let loop_start = self.current_addr();
                let tmp = self.alloc_reg();
                self.emit(0x8000 | ((tmp as u16) << 8) | ((lr as u16) << 4)); // tmp = lr
                self.emit(0x8000 | ((tmp as u16) << 8) | ((rr as u16) << 4) | 0x5); // tmp -= rr
                self.free_regs(1);
                self.emit(0x3F00); // SE VF, 0
                let skip = self.rom.len();
                self.emit(0x1000);
                self.emit(0x8000 | ((lr as u16) << 8) | ((rr as u16) << 4) | 0x5); // lr -= rr
                self.emit(0x1000 | loop_start);
                let end = self.current_addr();
                self.patch(skip, 0x1000 | end);
                self.emit(0x8000 | ((dest as u16) << 8) | ((lr as u16) << 4));
            }
            Op::EqEq => {
                self.quad(IrOp::CmpEq, Some(&lr_s), Some(&rr_s), Some(&dest_s));
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
                self.emit(0x5000 | ((lr as u16) << 8) | ((rr as u16) << 4));
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01);
            }
            Op::NotEq => {
                self.quad(IrOp::CmpNeq, Some(&lr_s), Some(&rr_s), Some(&dest_s));
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
                self.emit(0x9000 | ((lr as u16) << 8) | ((rr as u16) << 4));
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01);
            }
            Op::Lt => {
                self.quad(IrOp::CmpLt, Some(&lr_s), Some(&rr_s), Some(&dest_s));
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
                self.emit(0x8000 | ((lr as u16) << 8) | ((rr as u16) << 4) | 0x5);
                self.emit(0x3F00);
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01);
            }
            Op::Gt => {
                self.quad(IrOp::CmpGt, Some(&lr_s), Some(&rr_s), Some(&dest_s));
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
                self.emit(0x8000 | ((rr as u16) << 8) | ((lr as u16) << 4) | 0x5);
                self.emit(0x3F00);
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01);
            }
            Op::LtEq => {
                self.quad(IrOp::CmpLtEq, Some(&lr_s), Some(&rr_s), Some(&dest_s));
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
                self.emit(0x8000 | ((rr as u16) << 8) | ((lr as u16) << 4) | 0x5);
                self.emit(0x3F00 | 0x01);
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01);
            }
            Op::GtEq => {
                self.quad(IrOp::CmpGtEq, Some(&lr_s), Some(&rr_s), Some(&dest_s));
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
                self.emit(0x8000 | ((lr as u16) << 8) | ((rr as u16) << 4) | 0x5);
                self.emit(0x3F00 | 0x01);
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01);
            }
            Op::And => {
                self.quad(IrOp::And, Some(&lr_s), Some(&rr_s), Some(&dest_s));
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01);
                self.emit(0x3000 | ((lr as u16) << 8) | 0x01);
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
                self.emit(0x3000 | ((rr as u16) << 8) | 0x01);
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
            }
            Op::Or => {
                self.quad(IrOp::Or, Some(&lr_s), Some(&rr_s), Some(&dest_s));
                self.emit(0x6000 | ((dest as u16) << 8) | 0x00);
                self.emit(0x3000 | ((lr as u16) << 8) | 0x01);
                self.emit(0x4000 | ((rr as u16) << 8) | 0x01);
                self.emit(0x6000 | ((dest as u16) << 8) | 0x01);
            }
        }

        self.free_regs(2);
    }
}
