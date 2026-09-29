# Default `expr-v0` source profile

`expr-v0` is the default, expression-based spelling of NIL's current i64 core.
It lowers through the same AST, checker, validated HIR, and reference interpreter
as `lines-v0`. It remains experimental: the paired study covers three small arithmetic programs, not model generation or repair trajectories. The default can change as evidence grows.

```text
f0(x:i64,y:i64)->i64=(x+y)*3-2
f1(x)=x*x
```

`nil run FILE [FUNCTION_ID [I64_ARGUMENT...]]` uses `expr-v0`; `check` and `hir` do as well. Select legacy lines syntax with `nil --profile lines-v0 ...`. The Rust APIs `compile(source)` and `parser::parse(source)` default to `expr-v0`. Use `compile_with_profile(source, SourceProfile::LinesV0)` or `parser::parse_lines(source)` explicitly for `lines-v0`.

## Grammar

```ebnf
program   = function, { newline, function }, [ newline ] ;
function  = "f", uint, "(", [ parameter, { ",", parameter } ], ")",
            [ "->", "i64" ], "=", expression ;
parameter = identifier, [ ":", "i64" ] ;
expression = sum ;
sum       = product, { ("+" | "-"), product } ;
product   = primary, { ("*" | "/"), primary } ;
primary   = identifier | integer | call | "(", expression, ")" ;
call      = "f", uint, "(", [ expression, { ",", expression } ], ")" ;
uint      = "0" | nonzero_digit, { digit } ;
integer   = uint | "-", nonzero_digit, { digit } ;
identifier = (letter | "_"), { letter | digit | "_" } ;
```

Operators of equal precedence associate left to right. Function calls and
parentheses are expressions. Each function occupies one nonempty line; blank
lines, ASCII spaces and tabs, LF/CRLF, and a missing final newline are accepted.
Comments and multiline functions are unsupported. Identifiers use ASCII
letters, digits and underscores, starting with a letter or underscore. Names
beginning with `f` followed by a digit are reserved for canonical `f<ID>`
function labels. Duplicate or unknown parameters are errors.

All parameters and results are `i64`. An omitted annotation is inferred as
`i64` because this version has no other type. An explicit annotation must be
`i64`. Literals are canonical decimal i64 values: no leading zeros, plus sign,
or negative zero. A minus before a numeric literal is accepted; unary minus on
an expression is not yet supported. Function IDs fit `u32`. Expressions may
nest up to 128 parser levels. Source remains limited to 1 MiB.

The parser emits constants and operations in evaluation order with generated
value IDs. Forward calls resolve against all declared functions. The existing
checker validates call arity and value types; the evaluator preserves checked
i64 overflow, truncating division, zero-division traps, and execution limits.
