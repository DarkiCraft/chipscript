use std::process;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: codegen <input.cs>");
        process::exit(1);
    });

    let out_path = std::env::args().nth(2).unwrap_or_else(|| "out.ch8".to_string());

    let source = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("error: could not read '{}': {}", path, e);
        process::exit(1);
    });

    let tokens = chipscript::lexer::lex(source);
    let mut parser = chipscript::parser::Parser::new(tokens);
    let mut program = parser.parse();

    let mut opt = chipscript::optimizer::Optimizer::new();
    opt.optimize(&mut program);

    let mut cg = chipscript::codegen::Codegen::new();
    let rom = cg.generate(&program);

    std::fs::write(&out_path, &rom).unwrap_or_else(|e| {
        eprintln!("error: could not write '{}': {}", out_path, e);
        process::exit(1);
    });

    eprintln!("ok  {} -> {} ({} bytes)", path, out_path, rom.len());
}
