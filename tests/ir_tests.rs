// ir_tests.rs
#[cfg(test)]

mod common;

    use common::compile_ir;

    use chipscript::ir::*;

    #[test]
    fn load_immediate_produces_loadimm_quad() {
        let ir = compile_ir("vars { x = 42; } main { clear(); }");
        assert!(ir.iter().any(|q| matches!(q.op, IrOp::LoadImm) && q.arg1.as_deref() == Some("42")));
    }

    #[test]
    fn clear_produces_clear_quad() {
        let ir = compile_ir("main { clear(); }");
        assert!(ir.iter().any(|q| matches!(q.op, IrOp::Clear)));
    }

#[test]
fn while_produces_label_and_jumpfalse() {
    let ir = compile_ir("vars { x = 0; } main { while(x) { x = 0; } }");
    assert!(ir.iter().any(|q| matches!(q.op, IrOp::Label)));
    assert!(ir.iter().any(|q| matches!(q.op, IrOp::JumpFalse)));
}

    #[test]
    fn function_produces_label_and_return() {
        let ir = compile_ir("fn f() -> r { r = 0; } main { f(); }");
        assert!(ir.iter().any(|q| matches!(q.op, IrOp::Label) && q.arg1.as_deref() == Some("f")));
        assert!(ir.iter().any(|q| matches!(q.op, IrOp::Return)));
    }

    #[test]
    fn add_produces_add_quad() {
        let ir = compile_ir("vars { x = 0; } main { x = 1 + 2; }");
        assert!(ir.iter().any(|q| matches!(q.op, IrOp::Add)));
    }