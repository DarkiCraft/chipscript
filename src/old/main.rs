mod error;
mod lexer;
mod ast;
mod parser;
mod analyzer;
mod codegen;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 2 {
        eprintln!("usage: chipscript <file.cs>");
        std::process::exit(1);
    }

    let path = &args[1];
    let source = std::fs::read_to_string(path)
        .unwrap_or_else(|_| {
            eprintln!("error: could not read file '{}'", path);
            std::process::exit(1);
        });

    let tokens = lexer::lex(source);
    let mut parser = parser::Parser::new(tokens);
    let program = parser.parse();

    let mut analyzer = analyzer::Analyzer::new();
    analyzer.analyze(&program);

    let mut codegen = codegen::Codegen::new();
    let rom = codegen.generate(&program);

    let out_path = path.replace(".cs", ".ch8");
    std::fs::write(&out_path, &rom).unwrap_or_else(|_| {
        eprintln!("error: could not write '{}'", out_path);
        std::process::exit(1);
    });

    println!("ok! wrote {} bytes to {}", rom.len(), out_path);
}