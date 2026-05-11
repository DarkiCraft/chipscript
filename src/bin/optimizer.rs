use std::process;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: optimizer <input.cs>");
        process::exit(1);
    });

    let source = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("error: could not read '{}': {}", path, e);
        process::exit(1);
    });

    let tokens = chipscript::lexer::lex(source);
    let mut parser = chipscript::parser::Parser::new(tokens);
    let mut program = parser.parse();

    let mut opt = chipscript::optimizer::Optimizer::new();
    opt.optimize(&mut program);

    eprintln!("--- {} constant fold(s), {} dead branch(es), {} strength reduction(s)",
        opt.folds, opt.dce, opt.reductions);
    println!("{:#?}", program);
}
