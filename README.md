# ChipScript

A compiled, statically-typed language that targets the [CHIP-8](https://en.wikipedia.org/wiki/CHIP-8) fantasy console. Write programs in clean high-level syntax and compile them down to `.ch8` ROM files ready to run in any CHIP-8 emulator.

```
chipscript game.cs → out.ch8
```

## Features

- **Full 6-phase compilation pipeline** — lexing, parsing, semantic analysis, AST optimization, IR generation, and target code generation
- **Two types, zero surprises** — `int` (8-bit unsigned, 0–255) and `bool`; no implicit conversions
- **Built-in hardware abstractions** — draw sprites, poll the keypad, use timers and sound with simple function calls
- **Multi-pass AST optimizer** — constant folding, dead code elimination, and strength reduction via fixed-point iteration
- **Quadruple-based IR** — structured intermediate representation exposed via `--emit-ir`
- **Pipeline inspection** — dump the output of any compilation phase with `--emit-*` flags
- **Meaningful errors** — clear messages at each stage of compilation

## Installation

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable, 1.70+)

### Build from source

```bash
git clone https://github.com/your-username/chipscript.git
cd chipscript
cargo build --release
```

The binary will be at `target/release/chipscript`. Optionally install it to your `PATH`:

```bash
cargo install --path .
```

## Usage

```
Usage: chipscript [OPTIONS] <file.cs>

Options:
  -o <file>           Write output ROM to <file>  [default: out.ch8]
  -v, --verbose       Print compilation stages and ROM size info
      --no-analyze    Skip semantic analysis
      --no-opt        Skip optimization pass
      --emit-tokens   Lex only       — print tokens and stop
      --emit-ast       Lex + parse    — print AST before optimization and stop
      --emit-ast-opt  Lex + parse + optimize — print optimized AST and stop
      --emit-symtable Lex + parse + analyze — print symbol table and stop
      --emit-ir       Full pipeline  — print IR quads and stop
      --emit-rom-hex  Full pipeline  — print ROM as hex dump instead of writing file
  -V, --version       Print version and exit
  -h, --help          Print this help and exit
```

```bash
# Compile a source file (outputs out.ch8)
chipscript game.cs

# Compile with a custom output path
chipscript game.cs -o roms/game.ch8

# Verbose output — shows each stage and final ROM size
chipscript game.cs -v

# Inspect each phase
chipscript game.cs --emit-tokens     # token stream
chipscript game.cs --emit-ast        # raw AST
chipscript game.cs --emit-ast-opt    # AST after optimization (compare with --emit-ast)
chipscript game.cs --emit-ir         # IR quadruples
chipscript game.cs --emit-rom-hex    # final ROM as hex dump
```

## Compilation Pipeline

ChipScript compiles in 6 sequential phases:

```
Source (.cs)
    │
    ▼
[1] Lexer        — source text → token stream
    │
    ▼
[2] Parser       — token stream → Abstract Syntax Tree (AST)
    │
    ▼
[3] Analyzer     — semantic checks (undefined variables, type misuse, register limits)
    │
    ▼
[4] Optimizer    — multi-pass AST optimization (constant folding, DCE, strength reduction)
    │
    ▼
[5] IR Generator — AST → quadruple-based Intermediate Representation
    │
    ▼
[6] Code Generator — IR → CHIP-8 bytecode (.ch8 ROM)
```

Use `--emit-*` flags to stop the pipeline at any phase and inspect its output.

### Optimizer

The optimizer runs three passes to a fixed point (until no more changes occur):

- **Constant Folding** — evaluates compile-time expressions (`3 + 4 → 7`, `true and false → false`)
- **Dead Code Elimination** — removes unreachable branches (`if (false) { ... }` → nothing)
- **Strength Reduction** — replaces expensive ops with cheaper equivalents (`x * 2 → x + x`, `x * 1 → x`, `x / 1 → x`)

On CHIP-8, multiply and divide are software loops, so strength reduction has real impact on ROM size and speed.

### Intermediate Representation

The IR uses a **quadruple** format — each instruction has four fields:

```
(op, arg1, arg2, result)
```

Example output of `chipscript game.cs --emit-ir`:

```
(LoadImm, 32, _, V0)
(LoadImm, 16, _, V1)
(Label, main_loop, _, _)
(Draw, V0, V1, ball)
(Add, V0, V2, V0)
(JumpFalse, cond, L1, _)
...
```

## The Language

A ChipScript program is made up of four top-level sections:

```
sprites { ... }              // sprite pixel data
vars    { ... }              // global variable declarations
fn name(args) -> ret { ... } // function definitions
main    { ... }              // entry point
```

Only `main` is required. Sections can appear in any order.

### Types

ChipScript has exactly two types — there is no implicit conversion between them.

| Type   | Values        | Notes                           |
|--------|---------------|---------------------------------|
| `int`  | 0 to 255      | 8-bit unsigned, wraps mod 256   |
| `bool` | `true`, `false` | Cannot mix with `int`           |

Types are always inferred — there are no type annotations.

### Variables

All globals are declared in `vars {}` and must be initialised with a constant expression (literals combined with operators). Maximum of 14 variables (V0 is scratch, VF is the hardware flag).

```
vars {
    x   = 32;
    y   = 16;
    hit = false;
}
```

### Control Flow

```cs
if (condition) { ... } elif (condition) { ... } else { ... }

while (condition) { ... }

loop { ... }   // infinite loop — the standard game loop shell
```

### Functions

```cs
fn add(a, b) -> result {
    result = a + b;
}
```

Functions are declared with `fn`, take named arguments, and return via a named return variable. Called as expressions or statements — including nested calls and calls from other functions. Recursion is detected and rejected at compile time.

```cs
vars { total = 0; }
main {
    total = add(3, 4);
}
```

### Built-in Functions

| Function | Returns | Description |
|---|---|---|
| `draw(x, y, sprite)` | `bool` | Draw a sprite; returns `true` on pixel collision |
| `drawdigit(x, y, n)` | `bool` | Draw built-in hex digit 0–F |
| `clear()` | — | Clear the screen |
| `keypressed(n)` | `bool` | Non-blocking key check |
| `getkey()` | `int` | Block until a key is pressed |
| `delay(n)` | — | Set delay timer (counts down at 60 Hz) |
| `getdelay()` | `int` | Read current delay timer value |
| `beep(n)` | — | Set sound timer (beeps while nonzero) |
| `rand(mask)` | `int` | Random byte ANDed with a literal mask (0–255) |

### Operators

| Category | Operators | Precedence |
|---|---|---|
| Unary | `not` | 5 (tightest) |
| Multiplicative | `*` `/` `%` | 4 |
| Additive | `+` `-` | 3 |
| Comparison | `==` `!=` `<` `>` `<=` `>=` | 2 |
| Logic | `and` `or` | 1 (loosest) |

## Running ROMs

Load your compiled `.ch8` file in any CHIP-8 emulator:

- [Octo](https://johnearnest.github.io/Octo/) — web-based, great for quick testing
- [CHIP-8 Research Facility](https://chip-8.github.io/links/) — curated emulator list

### Keypad layout

```
  Keyboard     CHIP-8
  1 2 3 4  →  1 2 3 C
  Q W E R  →  4 5 6 D
  A S D F  →  7 8 9 E
  Z X C V  →  A 0 B F
```

## Hardware Limits

| Resource      | Limit          | Notes                              |
|---------------|----------------|------------------------------------|
| Screen        | 64 × 32 px     | Monochrome; sprites wrap at edges  |
| Variables     | 14 max         | V0 scratch, VF flag, V1–V14 work  |
| Sprite height | 1–15 rows      | 8 pixels wide, fixed               |
| Integer range | 0–255          | 8-bit unsigned; wraps mod 256      |
| Call stack    | 16 levels      | Enforced by compiler               |
| ROM size      | ≤ 3584 bytes   | 0x200–0xFFF; enforced by compiler |
| Timer freq    | 60 Hz          | Both delay and sound timers        |

## Project Structure

```
src/
├── main.rs       # CLI entry point and compilation driver
├── error.rs      # Error reporting
├── lexer.rs      # Tokenizer
├── ast.rs        # AST node definitions
├── parser.rs     # Parser (tokens → AST)
├── analyzer.rs   # Semantic analyzer
├── optimizer.rs  # Multi-pass AST optimizer
├── ir.rs         # IR quadruple definitions
└── codegen.rs    # CHIP-8 bytecode generator
```

## Sample Program

Program that displays a random hex digit on the screen at intervals
```
vars {
    n = 0;
    t = 0;
    b = false;
}
 
main {
    loop {
        // set delay timer to 60 ticks (~1 second at 60 Hz)
        delay(60);
 
        // spin until the timer hits zero
        t = getdelay();
        while (t != 0) {
            t = getdelay();
        }
 
        // rand(0x0F) gives a random value 0-15, drawdigit handles 0-F natively
        n = rand(0x0F);
 
        clear();
        b = drawdigit(30, 13, n);
    }
}
```

## License

MIT — see [LICENSE](LICENSE) for details.