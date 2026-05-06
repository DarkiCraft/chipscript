mod error;
mod lexer;
mod ast;
mod parser;
mod analyzer;
mod codegen;

use std::process;

const VERSION: &str = "1.0.0";

const HELP: &str = "\
Usage: chipscript [OPTIONS] <file.cs>

Options:
  -o <file>        Write output ROM to <file> instead of <input>.ch8
  -v, --verbose    Print compilation stages and ROM size info
      --no-analyze   Skip semantic analysis (for debugging the parser)
  -V, --version    Print version and exit
  -h, --help       Print this help and exit

Examples:
  chipscript game.cs                    # outputs game.ch8
  chipscript game.cs -o roms/out.ch8    # custom output path
  chipscript game.cs -v                 # verbose stage-by-stage output
";

struct Opts {
    input:      String,
    output:     Option<String>,
    verbose:    bool,
    no_analyze: bool,
}

fn parse_args() -> Opts {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("{}", HELP);
        process::exit(1);
    }

    let mut input      = None;
    let mut output     = None;
    let mut verbose    = false;
    let mut no_analyze = false;
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
            "-v" | "--verbose" => {
                verbose = true;
            }
            "--no-analyze" => {
                no_analyze = true;
            }
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
                    eprintln!("error: unexpected argument '{}' (input file already set)", args[i]);
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

    Opts { input, output, verbose, no_analyze }
}

fn main() {
    let opts = parse_args();

    // ── resolve output path ──────────────────────────────────────
    let out_path = opts.output.unwrap_or_else(|| {
        if opts.input.ends_with(".cs") {
            opts.input.replace(".cs", ".ch8")
        } else {
            format!("{}.ch8", opts.input)
        }
    });

    // ── read source ──────────────────────────────────────────────
    let source = std::fs::read_to_string(&opts.input)
        .unwrap_or_else(|e| {
            eprintln!("error: could not read '{}': {}", opts.input, e);
            process::exit(1);
        });

    if opts.verbose { eprintln!("[1/4] lexing '{}'", opts.input); }

    // ── lex ──────────────────────────────────────────────────────
    let tokens = lexer::lex(source);

    if opts.verbose {
        eprintln!("      {} tokens", tokens.len());
        eprintln!("[2/4] parsing");
    }

    // ── parse ────────────────────────────────────────────────────
    let mut parser = parser::Parser::new(tokens);
    let program = parser.parse();

    if opts.verbose {
        eprintln!("      {} sprite(s), {} var(s), {} function(s)",
            program.sprites.len(), program.vars.len(), program.functions.len());
        if opts.no_analyze {
            eprintln!("[3/4] analysis skipped (--no-analyze)");
        } else {
            eprintln!("[3/4] analyzing");
        }
    }

    // ── analyze ──────────────────────────────────────────────────
    if !opts.no_analyze {
        let mut analyzer = analyzer::Analyzer::new();
        analyzer.analyze(&program);
    }

    if opts.verbose { eprintln!("[4/4] generating ROM"); }

    // ── codegen ──────────────────────────────────────────────────
    let mut codegen = codegen::Codegen::new();
    let rom = codegen.generate(&program);

    // ── write output (create parent dirs if needed) ──────────────
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