mod analyzer;
mod ast;
mod codegen;
mod error;
mod ir;
mod lexer;
mod optimizer;
mod parser;

use std::process;

const VERSION: &str = "1.0.0";

const HELP: &str = "\
Usage: chipscript [OPTIONS] <file.cs>

Options:
  -o <file>           Write output ROM to <file>  [default: out.ch8]
  -v, --verbose       Print compilation stages and ROM size info
      --no-analyze    Skip semantic analysis
      --no-opt        Skip optimization pass
      --emit-tokens   Lex only       — print tokens and stop
      --emit-ast      Lex + parse    — print AST before optimization and stop
      --emit-ast-opt  Lex + parse + optimize — print optimized AST and stop
      --emit-ir       Full pipeline  — print IR quads and stop
      --emit-rom-hex  Full pipeline  — print ROM as hex dump instead of writing file
  -V, --version       Print version and exit
  -h, --help          Print this help and exit

Examples:
  chipscript game.cs                    # compile to out.ch8
  chipscript game.cs -o roms/out.ch8   # custom output path
  chipscript game.cs -v                # verbose stage-by-stage output
  chipscript game.cs --emit-tokens     # inspect lexer output
  chipscript game.cs --emit-ast        # inspect raw AST
  chipscript game.cs --emit-ast-opt    # inspect optimized AST
  chipscript game.cs --emit-ir         # inspect IR quads
  chipscript game.cs --emit-rom-hex    # inspect generated ROM bytes
";

struct Opts {
    input:        String,
    output:       Option<String>,
    verbose:      bool,
    no_analyze:   bool,
    no_opt:       bool,
    emit_tokens:  bool,
    emit_ast:     bool,
    emit_ast_opt: bool,
    emit_ir:      bool,
    emit_rom_hex: bool,
}

fn parse_args() -> Opts {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("{}", HELP);
        process::exit(1);
    }

    let mut input        = None;
    let mut output       = None;
    let mut verbose      = false;
    let mut no_analyze   = false;
    let mut no_opt       = false;
    let mut emit_tokens  = false;
    let mut emit_ast     = false;
    let mut emit_ast_opt = false;
    let mut emit_ir      = false;
    let mut emit_rom_hex = false;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print!("{}", HELP);
                process::exit(0);
            }
            "-V" | "--version" => {
                println!("chipscript {}", VERSION);
                process::exit(0);
            }
            "-v" | "--verbose"  => verbose      = true,
            "--no-analyze"      => no_analyze   = true,
            "--no-opt"          => no_opt        = true,
            "--emit-tokens"     => emit_tokens  = true,
            "--emit-ast"        => emit_ast      = true,
            "--emit-ast-opt"    => emit_ast_opt  = true,
            "--emit-ir"         => emit_ir       = true,
            "--emit-rom-hex"    => emit_rom_hex  = true,
            "-o" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: -o requires an argument");
                    process::exit(1);
                }
                output = Some(args[i].clone());
            }
            flag if flag.starts_with('-') => {
                eprintln!("error: unknown flag '{}'\n", flag);
                eprint!("{}", HELP);
                process::exit(1);
            }
            _ => {
                if input.is_some() {
                    eprintln!(
                        "error: unexpected argument '{}' (input file already set)",
                        args[i]
                    );
                    process::exit(1);
                }
                input = Some(args[i].clone());
            }
        }
        i += 1;
    }

    let input = match input {
        Some(p) => p,
        None => {
            eprintln!("error: no input file specified\n");
            eprint!("{}", HELP);
            process::exit(1);
        }
    };

    Opts { input, output, verbose, no_analyze, no_opt,
           emit_tokens, emit_ast, emit_ast_opt, emit_ir, emit_rom_hex }
}

fn main() {
    let opts = parse_args();

    let out_path = opts.output.unwrap_or_else(|| "out.ch8".to_string());

    // -- [1] lex --------------------------------------------------
    if opts.verbose { eprintln!("[1/5] lexing '{}'", opts.input); }

    let source = std::fs::read_to_string(&opts.input).unwrap_or_else(|e| {
        eprintln!("error: could not read '{}': {}", opts.input, e);
        process::exit(1);
    });

    let tokens = lexer::lex(source);

    if opts.emit_tokens {
        for tok in &tokens {
            println!("{:?}", tok);
        }
        eprintln!("--- {} token(s)", tokens.len());
        return;
    }

    if opts.verbose { eprintln!("      {} token(s)", tokens.len()); }

    // -- [2] parse ------------------------------------------------
    if opts.verbose { eprintln!("[2/5] parsing"); }

    let mut parser = parser::Parser::new(tokens);
    let mut program = parser.parse();

    if opts.emit_ast {
        println!("{:#?}", program);
        return;
    }

    if opts.verbose {
        eprintln!("      {} sprite(s), {} var(s), {} function(s)",
            program.sprites.len(), program.vars.len(), program.functions.len());
    }

    // -- [3] analyze ----------------------------------------------
    if opts.no_analyze {
        if opts.verbose { eprintln!("[3/5] analysis skipped (--no-analyze)"); }
    } else {
        if opts.verbose { eprintln!("[3/5] analyzing"); }
        let mut analyzer = analyzer::Analyzer::new();
        analyzer.analyze(&program);
        if opts.verbose { eprintln!("      ok"); }
    }

    // -- [4] optimize ---------------------------------------------
    if opts.no_opt {
        if opts.verbose { eprintln!("[4/5] optimization skipped (--no-opt)"); }
    } else {
        if opts.verbose { eprintln!("[4/5] optimizing"); }
        let mut opt = optimizer::Optimizer::new();
        opt.optimize(&mut program);
        if opts.verbose {
            eprintln!("      {} constant fold(s), {} dead branch(es) eliminated, {} strength reduction(s)",
                opt.folds, opt.dce, opt.reductions);
        }
    }

    if opts.emit_ast_opt {
        println!("{:#?}", program);
        return;
    }

    // -- [5] codegen + IR -----------------------------------------
    if opts.verbose { eprintln!("[5/5] generating ROM"); }

    let mut cg = codegen::Codegen::new();
    let rom = cg.generate(&program);

    if opts.emit_ir {
        for quad in cg.get_ir() {
            println!("{}", quad);
        }
        eprintln!("--- {} quad(s)", cg.get_ir().len());
        return;
    }

    if opts.emit_rom_hex {
        for (i, byte) in rom.iter().enumerate() {
            if i % 16 == 0 {
                if i > 0 { println!(); }
                print!("{:04X}:  ", 0x200 + i);
            }
            print!("{:02X} ", byte);
        }
        println!();
        eprintln!("--- {} byte(s)", rom.len());
        return;
    }

    // -- write output ---------------------------------------------
    if let Some(parent) = std::path::Path::new(&out_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).unwrap_or_else(|e| {
                eprintln!("error: could not create output directory '{}': {}", parent.display(), e);
                process::exit(1);
            });
        }
    }

    std::fs::write(&out_path, &rom).unwrap_or_else(|e| {
        eprintln!("error: could not write '{}': {}", out_path, e);
        process::exit(1);
    });

    if opts.verbose { eprintln!("      {} bytes written", rom.len()); }

    println!("ok  {} -> {} ({} bytes)", opts.input, out_path, rom.len());
}