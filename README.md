# ChipScript

A compiler for the CHIP-8 fantasy console, written in Rust. Write games and programs in a clean, high-level scripting language and compile them down to CHIP-8 ROM files (`.ch8`).

```
chipscript game.cs → game.ch8
```

## Features

- **Full compilation pipeline** — lexing, parsing, semantic analysis, and code generation
- **Meaningful errors** — clear messages at each stage of compilation
- **Flexible output** — specify a custom output path or let ChipScript derive one automatically
- **Verbose mode** — inspect each compilation stage and final ROM size
- **Fast** — single-pass pipeline with no runtime dependencies

## Installation

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable, 1.70+)

### Build from source

```bash
git clone https://github.com/your-username/chipscript.git
cd chipscript
cargo build --release
```

The compiled binary will be at `target/release/chipscript`.

Optionally, install it to your `PATH`:

```bash
cargo install --path .
```

## Usage

```
Usage: chipscript [OPTIONS] <file.cs>

Options:
  -o <file>        Write output ROM to <file> instead of <input>.ch8
  -v, --verbose    Print compilation stages and ROM size info
      --no-analyze   Skip semantic analysis (for debugging the parser)
  -V, --version    Print version and exit
  -h, --help       Print this help and exit
```

### Examples

```bash
# Compile a source file (outputs game.ch8)
chipscript game.cs

# Compile with a custom output path
chipscript game.cs -o roms/out.ch8

# Verbose output — shows each stage and final ROM size
chipscript game.cs -v

# Skip semantic analysis (useful when debugging parser output)
chipscript game.cs --no-analyze
```

## How It Works

ChipScript compiles `.cs` source files through a classic four-stage pipeline:

```
Source (.cs)
    │
    ▼
[1] Lexer        → Token stream
    │
    ▼
[2] Parser       → Abstract Syntax Tree (AST)
    │              (sprites, variables, functions)
    ▼
[3] Analyzer     → Semantic validation
    │
    ▼
[4] Code Generator → CHIP-8 ROM (.ch8)
```

Pass `-v` / `--verbose` to see each stage logged as it runs, including token counts, sprite/variable/function counts, and the final byte size of the ROM.

## Project Structure

```
src/
├── main.rs       # CLI entry point and compilation driver
├── error.rs      # Error types and reporting
├── lexer.rs      # Tokenizer
├── ast.rs        # AST node definitions
├── parser.rs     # Parser (tokens → AST)
├── analyzer.rs   # Semantic analyzer
└── codegen.rs    # CHIP-8 bytecode generator
```

## Running CHIP-8 ROMs

Once compiled, you can run your `.ch8` ROM in any CHIP-8 emulator. Some popular options:

- [Octo](https://johnearnest.github.io/Octo/) — web-based, great for quick testing
- [CHIP-8 Research Facility](https://chip-8.github.io/links/) — curated list of emulators
- [Chippy](https://github.com/starrhorne/chip8-rust) — Rust-based desktop emulator

## Contributing

Contributions are welcome! Feel free to open an issue or pull request for:

- Bug fixes
- Language feature additions
- Improved error messages
- Documentation improvements

## License

MIT License. See [LICENSE](LICENSE) for details.