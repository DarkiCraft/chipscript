# ChipScript — Compiler Construction Project Documentation

**Version 1.0.0**

A compiled, statically-typed language targeting the CHIP-8 fantasy console.

## Table of Contents

1. [Project Overview and Objectives](#1-project-overview-and-objectives)
2. [Formal Grammar](#2-formal-grammar)
3. [Detailed Working of Each Compiler Phase](#3-detailed-working-of-each-compiler-phase)
4. [Error Handling](#4-error-handling)
5. [Hardware Limits](#5-hardware-limits)
6. [Built-in Functions Reference](#6-built-in-functions-reference)

## 1. Project Overview and Objectives

ChipScript is a compiled, statically-typed programming language that targets the CHIP-8 fantasy console — an interpreted virtual machine widely used in hobbyist and educational computing since the 1970s. The goal of the project is to design and implement a complete, end-to-end compiler toolchain demonstrating all six canonical phases of compiler construction.

The source language is purpose-built for CHIP-8: it exposes the hardware directly (sprites, timers, keypad, sound) through clean high-level syntax, and compiles down to `.ch8` ROM files that run in any standard CHIP-8 emulator.

### 1.1 Objectives

- Design a simple, well-specified source language suitable for CHIP-8 development
- Implement all six compiler phases: lexing, parsing, semantic analysis, optimization, IR generation, and code generation
- Produce correct, runnable CHIP-8 bytecode from ChipScript source files
- Provide developer-facing tools to inspect each compilation phase independently
- Implement meaningful compile-time error reporting with phase and line information

### 1.2 Target Platform

The CHIP-8 is a virtual machine with a 64×32 monochrome display, 16 8-bit registers (V0–VF), 4 KB of memory, a 16-level call stack, and a 16-key hexadecimal keypad. These constraints directly shape the language design — for example, the 15-variable limit comes directly from the 16 available registers (VF is reserved as a hardware flag).

### 1.3 Pipeline Overview

```
Source (.cs)
    │
    ▼
[1] Lexer          — source text → token stream
    │
    ▼
[2] Parser         — token stream → Abstract Syntax Tree (AST)
    │
    ▼
[3] Analyzer       — semantic checks (types, scopes, register limits)
    │
    ▼
[4] Optimizer      — multi-pass AST optimization (constant folding, DCE, strength reduction)
    │
    ▼
[5] IR Generator   — AST → quadruple-based Intermediate Representation
    │
    ▼
[6] Code Generator — IR → CHIP-8 bytecode (.ch8 ROM)
```

Use `--emit-*` flags to stop the pipeline at any phase and inspect its output.

## 2. Formal Grammar

The following BNF (Backus-Naur Form) grammar defines the complete syntax of ChipScript. The parser implements this grammar using a hand-written recursive descent strategy.

### 2.1 Program Structure

```bnf
program     ::= section*

section     ::= sprites_block
              | vars_block
              | fn_decl
              | main_block
```

### 2.2 Top-Level Sections

```bnf
sprites_block ::= 'sprites' '{' sprite_decl* '}'
sprite_decl   ::= IDENT '=' sprite_data ';'
sprite_data   ::= '[' (INT (',' INT)*)? ']'
                | STRING_LIT

vars_block    ::= 'vars' '{' var_decl* '}'
var_decl      ::= IDENT '=' expr ';'

fn_decl       ::= 'fn' IDENT '(' param_list? ')' '->' IDENT block
param_list    ::= IDENT (',' IDENT)*

main_block    ::= 'main' block
```

### 2.3 Statements

```bnf
block       ::= '{' stmt* '}'

stmt        ::= 'if' '(' expr ')' block elif_clause* else_clause?
              | 'while' '(' expr ')' block
              | 'loop' block
              | 'clear' '(' ')' ';'
              | 'delay' '(' expr ')' ';'
              | 'beep' '(' expr ')' ';'
              | IDENT '=' expr ';'
              | IDENT '(' arg_list? ')' ';'

elif_clause ::= 'elif' '(' expr ')' block
else_clause ::= 'else' block
```

### 2.4 Expressions

```bnf
expr        ::= unary (binop unary)*
unary       ::= 'not' unary | primary

primary     ::= INT
              | BOOL
              | '(' expr ')'
              | 'draw'       '(' expr ',' expr ',' IDENT ')'
              | 'drawdigit'  '(' expr ',' expr ',' expr ')'
              | 'getkey'     '(' ')'
              | 'getdelay'   '(' ')'
              | 'keypressed' '(' expr ')'
              | 'rand'       '(' expr ')'
              | IDENT '(' arg_list? ')'
              | IDENT

arg_list    ::= expr (',' expr)*

binop       ::= '+' | '-' | '*' | '/' | '%'
              | '==' | '!=' | '<' | '>' | '<=' | '>='
              | 'and' | 'or'
```

### 2.5 Terminals

```bnf
INT         ::= [0-9]+  |  '0x' [0-9a-fA-F]+  |  '-' [0-9]+
BOOL        ::= 'true' | 'false'
IDENT       ::= [a-zA-Z_] [a-zA-Z0-9_]*
STRING_LIT  ::= '"' [^"]* '"'
```

> **Note:** All binary operators have equal precedence and associate left-to-right. Parentheses must be used explicitly to control evaluation order when mixing operator kinds.

## 3. Detailed Working of Each Compiler Phase

### Phase 1 — Lexical Analysis (`lexer.rs`)

The lexer transforms raw source text into a flat sequence of tokens. It scans character by character using a peekable iterator, tracking line numbers throughout so that all downstream errors can report the source line.

**Key responsibilities:**
- Recognise and emit all keywords, operators, literals, and delimiters
- Disambiguate multi-character tokens: `->` vs `-`, `==` vs `=`, `!=` vs `!`, `<=` vs `<`, `>=` vs `>`
- Handle line comments (`//`) by skipping to end of line
- Parse both decimal and hexadecimal (`0x...`) integer literals
- Track line numbers and attach them to every token as a `(Token, line)` pair
- Emit lexer errors with phase tag and line number: `error[lexer] line N: ...`

**Output:** `Vec<(Token, u32)>` — a list of (token, line) pairs ending with `(EOF, line)`.

**Inspect with:** `chipscript file.cs --emit-tokens`

### Phase 2 — Syntax Analysis / Parsing (`parser.rs`)

The parser consumes the token stream and constructs an Abstract Syntax Tree (AST). It uses a hand-written recursive descent strategy, with one parsing function per grammar rule.

**Key responsibilities:**
- Parse all four top-level sections (`sprites`, `vars`, `fn`, `main`) in any order
- Construct typed AST nodes: `Program`, `SpriteDecl`, `VarDecl`, `FnDecl`, `Stmt`, `Expr`
- Parse expressions with a unified `parse_binop` loop (left-to-right, equal precedence)
- Handle all statement forms: assignment, call, `if`/`elif`/`else`, `while`, `loop`, `clear`, `delay`, `beep`
- Parse built-in expression forms: `draw()`, `drawdigit()`, `getkey()`, `keypressed()`, `rand()`
- Report syntax errors with descriptive messages

**Output:** `Program` — the root AST node containing all declarations and the main body.

**Inspect with:** `chipscript file.cs --emit-ast`

### Phase 3 — Semantic Analysis (`analyzer.rs`)

The semantic analyzer performs type checking and scope validation over the AST. It maintains a symbol table mapping names to types and validates all language rules that cannot be enforced by the grammar alone.

**Symbol table contents:**

| Entry | Tracked Info |
|---|---|
| Global variables | name → `Type` (`Int` or `Bool`), scope: global |
| Functions | name → (arg list, return variable name) |
| Sprites | name, scope: sprite |

**Checks performed:**
- All referenced variables are declared (no undeclared variable access)
- All function calls reference a declared function with the correct argument count
- All sprite references in `draw()` are declared in `sprites {}`
- Type correctness: arithmetic on `int`, logic on `bool`, no implicit conversion
- Condition expressions in `if`/`elif`/`while` are `bool`
- Register budget at every call site: `vars + args + return slot ≤ 15`
- Sprite height: 1–15 rows (CHIP-8 hardware limit)
- `rand()` mask must be a compile-time literal, not a variable

**Output:** Validation only (exits with error on failure).

**Inspect with:** `chipscript file.cs --emit-symtable`

### Phase 4 — Optimization (`optimizer.rs`)

The optimizer performs multi-pass AST-level transformations to reduce ROM size and improve runtime performance. Passes run to a fixed point — repeated until no further changes occur.

#### Constant Folding

Evaluates expressions whose operands are known at compile time, replacing them with their computed results.

```
3 + 4          →  7
true and false →  false
not true       →  false
```

#### Dead Code Elimination

Removes unreachable branches from `if`/`elif`/`else` chains whose conditions are compile-time constants.

```
if (false) { ... }            →  removed entirely
if (true) { A } else { B }   →  A  (else branch dropped)
```

#### Strength Reduction

Replaces expensive arithmetic operations with cheaper equivalents. On CHIP-8, multiplication and division are implemented as software loops, making this impactful on both speed and ROM size.

```
x * 2  →  x + x
x * 1  →  x
x / 1  →  x
x * 0  →  0
```

**Inspect with:** `chipscript file.cs --emit-ast-opt` (compare with `--emit-ast` to see changes)

### Phase 5 — IR Generation (`ir.rs` / `codegen.rs`)

The IR generator traverses the optimized AST and emits a sequence of **quadruples** — a structured intermediate representation where each instruction has exactly four fields:

```
(op,  arg1,  arg2,  result)
```

Unused fields are represented as `_` in textual output. All operands are either virtual register names (`V0`, `V1`, ...) or label strings.

**IR opcode categories:**

| Category | Opcodes |
|---|---|
| Data movement | `LoadImm`, `Copy` |
| Arithmetic | `Add`, `Sub`, `Mul`, `Div`, `Mod` |
| Logic | `And`, `Or`, `Not` |
| Comparison | `CmpEq`, `CmpNeq`, `CmpLt`, `CmpGt`, `CmpLtEq`, `CmpGtEq` |
| Control flow | `Label`, `Jump`, `JumpFalse`, `Call`, `Return` |
| CHIP-8 hardware | `Clear`, `Draw`, `DrawDigit`, `SetDelay`, `SetSound`, `GetDelay`, `GetKey`, `KeyPressed`, `Rand` |

**Example output:**
```
(LoadImm, 32, _, V0)
(LoadImm, 16, _, V1)
(Label, main_loop, _, _)
(Draw, V0, V1, ball)
(JumpFalse, cond, L1, _)
```

**Inspect with:** `chipscript file.cs --emit-ir`

### Phase 6 — Target Code Generation (`codegen.rs`)

The code generator translates IR quadruples into CHIP-8 binary opcodes. CHIP-8 opcodes are 2 bytes each (big-endian) and programs load at address `0x200`. Register allocation maps the 15 ChipScript variables directly to CHIP-8 registers V0–V14.

**Key responsibilities:**
- Emit correct 2-byte CHIP-8 opcodes for all IR instructions
- Manage control flow: resolve forward jump targets by backpatching label addresses
- Emit sprite data into the ROM and record addresses for draw opcodes
- Handle function calls via the CHIP-8 call/return stack
- Produce a complete, runnable `.ch8` ROM binary

**Inspect with:** `chipscript file.cs --emit-rom-hex`

**Compile:** `chipscript file.cs  →  out.ch8`

## 4. Error Handling

Every error message includes the compiler phase and, where available, the source line number:

```
error[lexer] line 12: unexpected character '@'
error[parser] line 7: expected ';' but got '}'
error[analyzer]: type mismatch: 'hit' is Bool but assigned Int
```

| Error Category | Examples |
|---|---|
| Lexical | Unexpected character, unterminated string, hex literal too large |
| Syntax | Missing semicolon, unexpected token, wrong delimiter |
| Semantic | Undeclared variable/function/sprite, type mismatch, too many variables, register budget exceeded, empty sprite, sprite too tall, `rand()` mask not a literal |

The compiler exits with a non-zero status on any error and prints nothing to stdout, making it safe to use in shell pipelines.

## 5. Hardware Limits

| Resource | Limit | Notes |
|---|---|---|
| Screen | 64 × 32 px | Monochrome; sprites wrap at edges |
| Variables | 15 max | One CHIP-8 register each; VF reserved |
| Integer range | −128 to 127 | 8-bit signed; overflow wraps silently |
| Sprite height | 1–15 rows | 8 pixels wide, fixed |
| Call stack | 16 levels | CHIP-8 hardware limit |
| Timer frequency | 60 Hz | Both delay and sound timers |
| ROM memory | ~3.5 KB | Available for program + sprite data |

## 6. Built-in Functions Reference

| Signature | Returns | Description |
|---|---|---|
| `draw(x, y, sprite)` | `bool` | Draw sprite at (x, y). Returns `true` on pixel collision (XOR draw). |
| `drawdigit(x, y, n)` | `bool` | Draw built-in hex digit 0–F at (x, y). Returns collision flag. |
| `clear()` | — | Clear the entire screen to black. |
| `delay(n)` | — | Set delay timer to n. Counts down at 60 Hz. |
| `getdelay()` | `int` | Read current delay timer value. |
| `beep(n)` | — | Set sound timer to n. CHIP-8 beeps while nonzero. |
| `getkey()` | `int` | Block until a key is pressed; return key code 0x0–0xF. |
| `keypressed(n)` | `bool` | Non-blocking: return `true` if key n is currently held. |
| `rand(mask)` | `int` | Random byte ANDed with literal mask. Mask must be a literal. |