/// Intermediate Representation — Quadruples
/// Each instruction: (op, arg1, arg2, result)
/// Fields are Option<String> — None means "not applicable" for that slot.

#[derive(Debug, Clone)]
pub struct Quad {
    pub op:     IrOp,
    pub arg1:   Option<String>,
    pub arg2:   Option<String>,
    pub result: Option<String>,
}

impl Quad {
    pub fn new(op: IrOp, arg1: Option<&str>, arg2: Option<&str>, result: Option<&str>) -> Self {
        Quad {
            op,
            arg1:   arg1.map(str::to_string),
            arg2:   arg2.map(str::to_string),
            result: result.map(str::to_string),
        }
    }
}

impl std::fmt::Display for Quad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let a1 = self.arg1.as_deref().unwrap_or("_");
        let a2 = self.arg2.as_deref().unwrap_or("_");
        let r  = self.result.as_deref().unwrap_or("_");
        write!(f, "({:?}, {}, {}, {})", self.op, a1, a2, r)
    }
}

#[derive(Debug, Clone)]
pub enum IrOp {
    // Data movement
    LoadImm,    // result = arg1 (immediate integer/bool)
    Copy,       // result = arg1

    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,

    // Logic
    And,
    Or,
    Not,        // result = !arg1

    // Comparison  (result is a bool register)
    CmpEq,
    CmpNeq,
    CmpLt,
    CmpGt,
    CmpLtEq,
    CmpGtEq,

    // Control flow
    Label,      // arg1 = label name
    Jump,       // arg1 = label
    JumpFalse,  // if arg1 == false, jump to arg2
    Call,       // arg1 = fn name, arg2 = arg count, result = return reg
    Return,

    // CHIP-8 specific
    Clear,
    Draw,       // arg1 = x reg, arg2 = y reg, result = sprite name
    DrawDigit,  // arg1 = x reg, arg2 = y reg, result = n reg
    SetDelay,   // arg1 = value reg
    SetSound,   // arg1 = value reg
    GetDelay,   // result = dest reg
    GetKey,     // result = dest reg
    KeyPressed, // arg1 = key reg, result = dest reg
    Rand,       // arg1 = mask, result = dest reg

    // Sprites
    SpriteData, // arg1 = sprite name, arg2 = address
}
