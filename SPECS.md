# ChipScript Language Specification

**Version 1.0.0**

ChipScript is a statically-typed compiled language that targets the CHIP-8 virtual machine.
Source files use the `.cs` extension and compile to `.ch8` ROM images.


## Table of Contents

1. [Source Files](#1-source-files)
2. [Program Structure](#2-program-structure)
3. [Types](#3-types)
4. [Sprites](#4-sprites)
5. [Variables](#5-variables)
6. [Functions](#6-functions)
7. [Statements](#7-statements)
8. [Expressions](#8-expressions)
9. [Operators](#9-operators)
10. [Built-in Functions](#10-built-in-functions)
11. [Hardware Limits](#11-hardware-limits)
12. [Compile Errors](#12-compile-errors)


## 1. Source Files

- Encoding: UTF-8, Unix or Windows line endings
- Extension: `.cs`
- Comments: single-line only, introduced by `//`; everything after `//` to end of line is ignored
- Whitespace: spaces, tabs, and newlines are all ignored between tokens

```
// this is a comment
```


## 2. Program Structure

A ChipScript file is made up of four optional top-level sections that may appear in any order:

```
sprites { ... }   // sprite data declarations
vars    { ... }   // global variable declarations
fn name(...) -> ret { ... }   // function definitions (one or more)
main    { ... }   // program entry point
```

Only `main` is required for a program that does anything at runtime.
Sections may appear in any order and each may appear at most once,
except `fn` which may appear multiple times (one per function).

### Minimal valid program

```
main {
    clear();
}
```


## 3. Types

ChipScript has exactly two types. There is no implicit conversion between them.

| Type   | Values      | Storage          | Notes                              |
|--------|-------------|------------------|------------------------------------|
| `int`  | 0 to 255    | 8-bit unsigned   | Wraps mod 256 on overflow/underflow |
| `bool` | `true`, `false` | 8-bit (0 or 1) | Sugar over 0/1; cannot mix with int |

Type is always inferred from context — there are no type annotations.
Variables take the type of their initialiser; function arguments and return values are always `int`.


## 4. Sprites

Sprites are declared in the `sprites {}` block before use.
Each sprite has a name and pixel data, either inline or from a file.

```
sprites {
    name = [byte, byte, ...];   // inline bytes
    name = "path/to/file.spr";  // load from disk at compile time
}
```

### Inline sprites

Bytes are comma-separated integer literals (decimal or hex).
Each byte represents one 8-pixel-wide row of the sprite.

```
sprites {
    ball   = [0x3C];              // 1 row: ..111100
    paddle = [0xFF, 0xFF, 0xFF];  // 3 rows: solid bar
    ship   = [0x18, 0x3C, 0x7E, 0xFF, 0x7E];  // 5 rows
}
```

### File sprites

The path is relative to the working directory when the compiler runs.
`.spr` files are raw bytes with no header — one byte per row.

```
sprites {
    player = "assets/player.spr";
}
```

### Sprite constraints

| Constraint          | Limit | Reason                                 |
|---------------------|-------|----------------------------------------|
| Width               | 8 px  | Fixed by CHIP-8 hardware               |
| Height              | 1–15 rows | Hardware draw opcode uses 4-bit height |
| Empty sprite        | Not allowed | Zero-height causes hardware no-op  |


## 5. Variables

All global variables are declared in the `vars {}` block.
Every variable must be initialised at declaration.

```
vars {
    x    = 32;      // int (inferred from integer literal)
    y    = 16;      // int
    hit  = false;   // bool (inferred from bool literal)
    mask = 0xFF;    // int (hex literal)
}
```

### Rules

- Maximum **14 variables** total (V0 is reserved as a general-purpose scratch register, VF is a hardware flag)
- Variables may only be declared inside `vars {}` — not inside functions or `main`
- All variables are global and accessible from `main` and all functions
- No shadowing; every name must be unique across vars, function names, and sprite names
- Initial value must be a **constant expression** (literals combined with arithmetic/logic operators only — no variables, function calls, or builtins)

### Integer literals

| Form        | Example  | Notes                            |
|-------------|----------|----------------------------------|
| Decimal     | `42`     | Unsigned byte, range 0–255       |
| Hexadecimal | `0xFF`   | Prefix `0x`, digits `0-9 a-f A-F`|


## 6. Functions

```
fn name(arg1, arg2) -> ret {
    ret = arg1 + arg2;
}
```

- Arguments and the return variable are **temporary register aliases** — they exist only for the duration of the call
- All arguments and the return variable are typed `int`
- The return value is whatever `ret` holds when the function body finishes
- No `return` keyword; just assign to the return variable name
- Functions must be declared at the top level; no nested functions
- **No recursion** — calling a function from within itself or through a mutual cycle is a compile-time error (detected by call-graph analysis)
- The compiler enforces a **register budget**: `variables used + function frame (args + 1 return) + max expression temporaries ≤ 14`
- Function calls may appear **anywhere** an expression is expected — including nested calls (`add(a, add(b, c))`) and calls from other functions
- The maximum **call chain depth** from `main` is **16 levels** (the CHIP-8 hardware call-stack limit); the compiler rejects deeper chains

### Calling a function

As a statement (return value discarded):

```
move(dx, dy);
```

As an expression (return value used):

```
dist = add(a, b);
```


## 7. Statements

### Assignment

```
variable = expression;
```

The type of the expression must match the type of the variable.

### Function call statement

```
name(arg1, arg2);
```

Return value is discarded.

### If / elif / else

```
if (condition) {
    // ...
} elif (condition) {
    // ...
} else {
    // ...
}
```

- Condition must be `bool`
- `elif` and `else` are optional; any number of `elif` branches is allowed
- No one-liner form — braces are always required

### While loop

```
while (condition) {
    // ...
}
```

Condition must be `bool`. Re-evaluated on every iteration.

### Infinite loop

```
loop {
    // ...
}
```

Runs forever. Use as the outer shell of a game loop.

### Built-in statements

```
clear();        // clear the screen
delay(n);       // set delay timer to n (int)
beep(n);        // set sound timer to n (int); beeps while nonzero
```


## 8. Expressions

### Literals

```
42        // int literal (decimal, 0–255)
0xFF      // int literal (hex, 0x00–0xFF)
true      // bool literal
false     // bool literal
```

### Variable reference

```
x         // evaluates to the current value of variable x
```

### Binary operations

```
a + b
a == b
a and b
// etc. — see section 9
```

ChipScript implements **conventional precedence tiers** with left associativity within each tier. From lowest to highest:

| Tier     | Operators                          |
|----------|------------------------------------|
| `or`     | `or`                               |
| `and`    | `and`                              |
| comparison | `==` `!=` `<` `>` `<=` `>=`     |
| additive | `+` `-`                            |
| multiplicative | `*` `/` `%`                  |

Precedence is always resolved explicitly:

```
x + y * z        → x + (y * z)   // multiplication first
x < 5 and y < 6  → (x < 5) and (y < 6)
not a and b      → (not a) and b
```

Use parentheses to override:

### Unary not

```
not condition   // flips bool: true -> false, false -> true
```

### Parenthesised expression

```
(expr)
```

### Function call expression

```
name(arg1, arg2)   // returns int
```

### Built-in expressions

See [Section 10](#10-built-in-functions) for full signatures and return types.


## 9. Operators

### Arithmetic — both operands must be `int`, result is `int`

| Operator | Operation      | Notes                               |
|----------|----------------|-------------------------------------|
| `a + b`  | Addition       | Wraps on overflow                   |
| `a - b`  | Subtraction    | Wraps on underflow                  |
| `a * b`  | Multiplication | Software loop — slow for large values |
| `a / b`  | Division       | Software loop — slow for large values; result is integer quotient |
| `a % b`  | Modulo         | Software loop — remainder after division |

Division and modulo with a zero divisor produce undefined behaviour (hardware loop).

### Comparison — operands must be the same type, result is `bool`

| Operator | Meaning               |
|----------|-----------------------|
| `a == b` | Equal                 |
| `a != b` | Not equal             |
| `a < b`  | Less than             |
| `a > b`  | Greater than          |
| `a <= b` | Less than or equal    |
| `a >= b` | Greater than or equal |

### Logic — both operands must be `bool`, result is `bool`

| Operator  | Meaning     |
|-----------|-------------|
| `a and b` | Logical AND |
| `a or b`  | Logical OR  |
| `not a`   | Logical NOT |

### Precedence

ChipScript implements conventional precedence tiers (see §8):
`or` < `and` < comparison < additive < multiplicative.
All operators are left-associative within their tier.
When in doubt, add parentheses.


## 10. Built-in Functions

Built-ins are part of the language and cannot be shadowed by user functions.

### draw(x, y, sprite) → bool

Draws `sprite` at screen position (`x`, `y`).
Returns `true` if any pixel that was turned ON collided with a pixel already ON (XOR draw, CHIP-8 collision).

```
hit = draw(px, py, ball);
```

- `x`, `y` — `int` coordinates; CHIP-8 screen is 64×32, wraps automatically
- `sprite` — must be a sprite name declared in `sprites {}`
- Return type: `bool`

### drawdigit(x, y, n) → bool

Draws the built-in CHIP-8 hex digit sprite for the value `n` (0–15) at (`x`, `y`).
All digit sprites are 5 rows × 8 pixels. Returns collision flag same as `draw`.

```
hit = drawdigit(10, 5, score);
```

- `n` — `int`, interpreted as 0x0–0xF; the compiler masks values with `AND 0x0F` so values outside 0–15 are always defined
- Return type: `bool`

### clear()

Clears the entire screen to black. Use as a statement.

```
clear();
```

### delay(n)

Sets the delay timer to `n`. The timer counts down at 60 Hz.

```
delay(60);   // set timer to 60 (about 1 second)
```

- `n` — `int`
- No return value (statement only)

### getdelay() → int

Reads the current value of the delay timer.

```
t = getdelay();
```

- Return type: `int`

### beep(n)

Sets the sound timer to `n`. The CHIP-8 emits a tone for as long as the sound timer is nonzero.

```
beep(10);
```

- `n` — `int`
- No return value (statement only)

### getkey() → int

Blocks execution until a key is pressed, then returns the key code (0x0–0xF).

```
k = getkey();
```

- Return type: `int`

### keypressed(n) → bool

Returns `true` if key `n` is currently held down (non-blocking).

```
if (keypressed(0x5)) { px = px + 1; }
```

- `n` — `int`, key code 0x0–0xF
- Return type: `bool`

### rand(mask) → int

Returns a random byte ANDed with `mask`. `mask` must be an **integer literal** — not a variable.

```
r = rand(0x03);   // random value 0, 1, 2, or 3
r = rand(0xFF);   // random value 0–255
```

- `mask` — compile-time integer literal only
- Return type: `int`

### CHIP-8 Keypad Layout

```
  Keyboard   →   CHIP-8 key
  ─────────────────────────
  1 2 3 4         1 2 3 C
  Q W E R    →    4 5 6 D
  A S D F         7 8 9 E
  Z X C V         A 0 B F
```

Key codes in ChipScript: `0`–`9` and `0xA`–`0xF`.


## 11. Hardware Limits

These limits are imposed by the CHIP-8 architecture and enforced by the compiler where possible.

| Resource            | Limit    | Notes                                               |
|---------------------|----------|-----------------------------------------------------|
| Screen              | 64 × 32 px | Monochrome; draw wraps at edges                    |
| Memory              | 4 KB total | ~3.5 KB (3584 bytes) available for program + sprites; enforced by the compiler |
| Registers           | 14 work + V0 scratch + VF flag | V0 reserved as scratch; V1–V14 work region; VF flag |
| Variables           | 14 max     | One per register; V0 is scratch, VF is flag        |
| Sprite height       | 1–15 rows  | Hardware nibble; enforced at compile time          |
| Sprite width        | 8 px fixed | Cannot be changed                                   |
| Call stack depth    | 16 levels  | Enforced by the compiler; maximum call chain depth from main |
| Timer frequency     | 60 Hz      | Both delay and sound timers count down at 60 Hz     |
| Integer range       | 0–255      | 8-bit unsigned; wraps mod 256 on overflow/underflow |
| ROM size            | ≤ 3584 bytes | Program image (code + sprites + save areas) must fit; the compiler rejects larger programs |


## 12. Compile Errors

The compiler exits with a non-zero status and prints a message to stderr on any of the following:

| Error                              | Cause                                                          |
|------------------------------------|----------------------------------------------------------------|
| `too many variables`               | More than 14 variables declared in `vars {}`                   |
| `undeclared variable`              | Reference to a name not declared in `vars {}`                  |
| `undeclared function`              | Call to a function not defined with `fn`                       |
| `undeclared sprite`                | `draw()` references a sprite name not in `sprites {}`          |
| `type mismatch`                    | Assigning a `bool` to an `int` variable or vice versa          |
| `if/elif/while condition not bool` | Condition expression evaluates to `int`                        |
| `arithmetic requires int`          | `+`, `-`, `*`, `/`, `%` used on `bool` operands                |
| `logic requires bool`              | `and`, `or`, `not` used on `int` operands                      |
| `comparison on bool`               | `<`, `>`, `<=`, `>=` on `bool` operands                          |
| `ordered comparison requires int`  | `<`, `>`, `<=`, `>=` comparing bools (only `==`/`!=` allowed)    |
| `not enough registers`             | Function or expression exceeds the 14-register budget (globals + frame + temps) |
| `recursive call detected`          | Call graph contains a cycle (no recursion allowed)             |
| `call chain too deep`              | More than 16 nested function calls (CHIP-8 call stack limit)   |
| `non-constant initializer`         | Variable initializer is not a literal/constant expression       |
| `division by zero`                 | Literal or constant-folded division or modulo by zero           |
| `sprite file not found`            | File-backed sprite path does not exist at compile time         |
| `sprite too tall`                  | Sprite exceeds 15 rows                                         |
| `sprite is empty`                  | Sprite has zero bytes                                          |
| `rand() mask must be a literal`    | `rand()` called with a variable instead of a literal           |
| `integer literal out of range`     | Decimal literal > 255 or hex literal > 0xFF                    |
| `duplicate section`                | A `sprites`/`vars`/`main` section appears more than once       |
| `duplicate name`                   | Name used in multiple declarations within the same namespace   |
| `name collision`                   | Same name used as both a variable and a function or sprite     |
| `function argument not int`        | Non-int argument passed to a user-defined function             |
| `program too large`                | Compiled ROM image exceeds 3584 bytes (CHIP-8 memory limit)   |
| `could not read file`              | Input `.cs` file could not be opened                           |