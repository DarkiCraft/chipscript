#![allow(dead_code)]
#![cfg(test)]

use chipscript::analyzer::Analyzer;
use chipscript::ast::*;
use chipscript::codegen::Codegen;
use chipscript::lexer;
use chipscript::optimizer::Optimizer;
use chipscript::parser::Parser;

// ---------------------------------------------------------------------------
// Helper: lex + parse a source string into a Program
// ---------------------------------------------------------------------------
pub fn parse(src: &str) -> Program {
    let tokens = lexer::lex(src.to_string());
    let mut p = Parser::new(tokens);
    p.parse()
}

// Helper: full pipeline (lex → parse → analyze → optimize → codegen)
pub fn compile(src: &str) -> Vec<u8> {
    let mut program = parse(src);
    Analyzer::new().analyze(&program);
    Optimizer::new().optimize(&mut program);
    Codegen::new().generate(&program)
}

// Helper: compile without the optimizer (for codegen unit tests)
pub fn compile_no_opt(src: &str) -> Vec<u8> {
    let program = parse(src);
    Analyzer::new().analyze(&program);
    Codegen::new().generate(&program)
}

// Helper: compile to ir without emiting bytecode
pub fn compile_ir(src: &str) -> Vec<chipscript::ir::Quad> {
    let tokens = chipscript::lexer::lex(src.to_string());
    let mut parser = chipscript::parser::Parser::new(tokens);
    let program = parser.parse();
    let mut cg = chipscript::codegen::Codegen::new();
    cg.generate(&program);
    cg.get_ir().to_vec()
}

// Helper: run a closure and assert it panics (catches process::exit too)
pub fn assert_panics<F: FnOnce() + std::panic::UnwindSafe>(f: F) {
    assert!(
        std::panic::catch_unwind(f).is_err(),
        "expected a compile error (panic/exit) but got none"
    );
}
