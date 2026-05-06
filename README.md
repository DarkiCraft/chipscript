# ChipScript

A compiled, statically-typed language that targets the [CHIP-8](https://en.wikipedia.org/wiki/CHIP-8) fantasy console. Write programs in clean high-level syntax and compile them down to `.ch8` ROM files ready to run in any CHIP-8 emulator.

```
chipscript game.cs → game.ch8
```

## Features

- **Full compilation pipeline** — lexing, parsing, semantic analysis, and code generation
- **Two types, zero surprises** — `int` (8-bit signed) and `bool`; no implicit conversions
- **Built-in hardware abstractions** — draw sprites, poll the keypad, use timers and sound with simple function calls
- **Meaningful errors** — clear messages at each stage of compilation
- **Verbose mode** — inspect each compilation stage and final ROM size

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
  -o <file>        Write output ROM to <file> instead of <input>.ch8
  -v, --verbose    Print compilation stages and ROM size info
      --no-analyze   Skip semantic analysis (for debugging the parser)
  -V, --version    Print version and exit
  -h, --help       Print this help and exit
```

```bash
# Compile a source file (outputs game.ch8)
chipscript game.cs

# Compile with a custom output path
chipscript game.cs -o roms/out.ch8

# Verbose output — shows each stage and final ROM size
chipscript game.cs -v
```

## The Language

A ChipScript program is made up of four top-level sections:

```
sprites { ... }             // sprite pixel data
vars    { ... }             // global variable declarations
fn name(args) -> ret { ... } // function definitions
main    { ... }             // entry point
```

Only `main` is required. Sections can appear in any order.

### Types

ChipScript has exactly two types — there is no implicit conversion between them.

| Type   | Values          | Notes                           |
|--------|-----------------|---------------------------------|
| `int`  | −128 to 127     | 8-bit signed, wraps on overflow |
| `bool` | `true`, `false` | Cannot mix with `int`           |

Types are always inferred — there are no type annotations.

### Variables

All globals are declared in `vars {}` and must be initialised with a literal. Maximum of 15 variables (one register per variable, VF is reserved by hardware).

```
vars {
    x   = 32;
    y   = 16;
    hit = false;
}
```

### Control flow

```cs
if (condition) { ... } elif (condition) { ... } else { ... }

while (condition) { ... }

loop { ... }   // infinite loop — the standard game loop shell
```

### Built-in functions

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
| `rand(mask)` | `int` | Random byte ANDed with a literal mask |

## Examples

### Bouncing ball

A sprite that bounces around the screen, reversing direction on collision.

```cs
sprites {
    ball = [0x3C, 0x7E, 0x7E, 0x3C];  // 4-row circle
}

vars {
    bx  = 32;
    by  = 16;
    dx  = 1;
    dy  = 1;
    hit = false;
}

main {
    loop {
        // erase at current position (XOR draw)
        hit = draw(bx, by, ball);

        // move
        bx = bx + dx;
        by = by + dy;

        // bounce off edges
        if ((bx <= 0) or (bx >= 60)) { dx = dx * -1; }
        if ((by <= 0) or (by >= 28)) { dy = dy * -1; }

        // redraw
        hit = draw(bx, by, ball);

        // pace the loop to ~60 fps
        delay(1);
        hit = false;
        while (not hit) { hit = (getdelay() == 0); }
    }
}
```

### Keyboard counter

Press any key to increment a digit displayed in the centre of the screen.

```cs
vars {
    count = 0;
    k     = 0;
}

main {
    clear();
    loop {
        // display current count as a hex digit
        k = drawdigit(28, 12, count);

        // wait for a keypress, then erase and increment
        k = getkey();
        k = drawdigit(28, 12, count);
        count = count + 1;

        // wrap at 16
        if (count == 16) { count = 0; }
    }
}
```

### Dice roller

Press any key to roll a die and show the result. Beeps on a six.

```cs
vars {
    roll = 0;
    k    = 0;
}

fn roll_die(seed) -> result {
    result = rand(0x07);              // 0–7
    if (result > 5) { result = rand(0x05); }  // re-roll if out of range
    result = result + 1;              // shift to 1–6
}

main {
    clear();
    loop {
        k    = getkey();              // wait for any key
        roll = roll_die(k);
        clear();
        k = drawdigit(28, 12, roll);

        if (roll == 6) { beep(20); } // beep on a six
    }
}
```

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

| Resource      | Limit       | Notes                              |
|---------------|-------------|------------------------------------|
| Screen        | 64 × 32 px  | Monochrome; sprites wrap at edges  |
| Variables     | 15 max      | One CHIP-8 register each           |
| Sprite height | 1–15 rows   | 8 pixels wide, fixed               |
| Integer range | −128 to 127 | Overflow wraps silently            |
| Call stack    | 16 levels   | CHIP-8 hardware limit              |
| Timer freq    | 60 Hz       | Both delay and sound timers        |

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

## License

MIT — see [LICENSE](LICENSE) for details.