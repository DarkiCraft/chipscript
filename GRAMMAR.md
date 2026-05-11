```
program     ::= section*

section     ::= sprites_block
              | vars_block
              | fn_decl
              | main_block

sprites_block ::= 'sprites' '{' sprite_decl* '}'
sprite_decl   ::= IDENT '=' sprite_data ';'
sprite_data   ::= '[' (INT (',' INT)*)? ']'
                | STRING_LIT

vars_block  ::= 'vars' '{' var_decl* '}'
var_decl    ::= IDENT '=' expr ';'

fn_decl     ::= 'fn' IDENT '(' param_list? ')' '->' IDENT block
param_list  ::= IDENT (',' IDENT)*

main_block  ::= 'main' block

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

expr        ::= unary (binop unary)*
unary       ::= 'not' unary | primary
primary     ::= INT
              | BOOL
              | '(' expr ')'
              | 'draw' '(' expr ',' expr ',' IDENT ')'
              | 'drawdigit' '(' expr ',' expr ',' expr ')'
              | 'getkey' '(' ')'
              | 'getdelay' '(' ')'
              | 'keypressed' '(' expr ')'
              | 'rand' '(' expr ')'
              | IDENT '(' arg_list? ')'
              | IDENT

arg_list    ::= expr (',' expr)*

binop       ::= '+' | '-' | '*' | '/' | '%'
              | '==' | '!=' | '<' | '>' | '<=' | '>='
              | 'and' | 'or'

INT         ::= [0-9]+ | '0x'[0-9a-fA-F]+ | '-'[0-9]+
BOOL        ::= 'true' | 'false'
IDENT       ::= [a-zA-Z_][a-zA-Z0-9_]*
STRING_LIT  ::= '"' [^"]* '"'
```