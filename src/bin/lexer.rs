use std::process;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: lexer <input.cs>");
        process::exit(1);
    });

    let source = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("error: could not read '{}': {}", path, e);
        process::exit(1);
    });

    let tokens = chipscript::lexer::lex(source);
    for tok in &tokens {
        println!("{:?}", tok);
    }
    eprintln!("--- {} token(s)", tokens.len());
}
