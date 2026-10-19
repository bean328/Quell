# Quell grammar (v0.1 draft)

This chapter gives the complete syntax of Quell: files, items, statements, expressions, types and patterns. It is written so that a recursive-descent parser can be built from it directly. Every rule is followed by notes in plain English.

The grammar reads the token stream defined in [lexical.md](lexical.md), after the [newline rules](lexical.md#12-the-newline-rules) have decided where `NEWLINE` tokens are. The notation is explained in [README.md](README.md#how-to-read-the-grammar).

Only syntax is specified here. Where a rule has a meaning that a later stage checks (types, ownership, what is allowed at compile time), the note says so and stops there.

Contents:

1. [Conventions](#1-conventions)
2. [Files](#2-files)
3. [Items](#3-items)
4. [Blocks and statements](#4-blocks-and-statements)
5. [Precedence and associativity](#5-precedence-and-associativity)
6. [Expressions](#6-expressions)
7. [Types](#7-types)
8. [Patterns](#8-patterns)
9. [Errors the parser reports](#9-errors-the-parser-reports)
10. [Ambiguities and how they are resolved](#10-ambiguities-and-how-they-are-resolved)
11. [Rejected syntax](#11-rejected-syntax)
12. [Example coverage](#12-example-coverage)

---

## 1. Conventions

### 1.1 Notation

A short summary (the full description is in the [README](README.md#how-to-read-the-grammar)):

| Notation | Meaning |
|---|---|
| `Rule = ... ;` | defines `Rule` |
| `"fn"`, `"=>"` | a keyword or punctuation token, spelled exactly like this |
| `"any"`, `"type"`, `"parallel"`, `"test"`, `"c"` | a **contextual keyword**: an `IDENT` token with exactly this spelling |
| `IDENT`, `INT`, `NEWLINE`, ... | a token kind from [lexical.md](lexical.md#13-summary-of-token-kinds) |
| `A B` | `A` followed by `B` |
| <code>A &#124; B</code> | `A` or `B` |
| `[ A ]` | `A` is optional |
| `{ A }` | `A` repeated zero or more times |
| `( A B )` | grouping |

### 1.2 Token classes used in the grammar

| Name | Meaning |
|---|---|
| `IDENT` | any identifier |
| `UPPER_IDENT` | an identifier whose first letter is uppercase ([lexical.md, 5.2](lexical.md#52-upper-and-lower-names)) |
| `LOWER_IDENT` | any other identifier |
| `INT`, `FLOAT`, `CHAR` | number and character literals |
| `STRING` | any string literal, possibly with holes |
| `PLAIN_STRING` | a `STRING` with no holes (every raw string is plain) |
| `NEWLINE` | a significant line break |
| `EOF` | end of file |

**Declaration names.** Where the grammar says `UPPER_IDENT` or `LOWER_IDENT` at a place that declares a name, the parser actually accepts any `IDENT`, reports a naming error with a fix if the class is wrong, and carries on. This gives a better message than "unexpected token".

### 1.3 Newlines in the grammar

`NEWLINE` appears in the grammar only where it may appear. Because the newline rules have already removed line breaks inside `( )` and `[ ]`, after operators and commas, and before `.`, `?.`, `else`, `catch`, `??`, `and` and `or`, the rules below rarely need to mention them. A `NEWLINE` anywhere the grammar does not allow one is a syntax error.

Two facts from the newline rules are used often:

- There is never a `NEWLINE` right after `{` (an opening bracket is in the continue-after set), but there can be one right before `}`.
- There is never a `NEWLINE` right after a `,`, so `,` and `NEWLINE` never appear next to each other.

---

## 2. Files

### 2.1 File structure

```
File        = [ TopElement { NEWLINE TopElement } ] EOF ;
TopElement  = Item | Statement ;
```

- A file is a list of items and statements, one per line.
- **How the parser tells them apart:** a `TopElement` is an `Item` if its first token is `@`, `pub`, `fn`, `mut`, `static`, `comptime`, `struct`, `enum`, `trait`, `extend`, `import`, `let` or `var`, or if it is the contextual keyword `test` followed by a string literal. Otherwise it is a `Statement`.
- So a `let` or `var` at the top level is always an item (a `GlobalLet`, section 3.12), never a `LetStmt`. In an entry file it still runs in order with the other statements; that is a matter for later stages.
- `mut` and `static` at the top level start a function declaration, which the parser then reports as an error ("`mut fn` and `static fn` only make sense inside a type").
- `comptime` starts three different items. The token after it decides: `fn` gives a `FnDecl`, `let` or `var` a `GlobalLet`, and `{` a `ComptimeBlock`.

### 2.2 Entry files and library files

PLAN section 4.13 says top-level code runs only in the program's entry file. The parser handles this with a **file role** given to it by the build driver. There are exactly two roles, and every file has one:

- An **entry file** may contain items and statements in any order. The entry files are: the file given to `quell run` (such as `19_modules/main.ql` or `20_testing.ql`); the main file of a program package, which in the examples is `main.ql` at the package root (`28_c_interop/main.ql`, `29_embedded_blink/main.ql`); and every code example in a doc comment ([lexical.md, 4.3](lexical.md#43-doc-comments)).
- A **library file** may contain only items. The library files are all other files: the main module of a library package (`21_library/src/lib.ql`, which `import` reaches as "a package's main module", section 3.9) and every other module (`geometry/point.ql`).

Exactly which file of a package is its main file, and how the build tool knows whether a package is a program or a library, belongs to the build tool and its manifest, not to this syntax spec. The examples follow the layout above; making it official is an [open question](README.md#open-questions).

The role does not depend on attributes or on the build profile. `@entry` (example 29) marks the function where a program without scripts starts; it does not change the file's role. Whether top-level statements are allowed on a target with no operating system (`profile = "bare"`) is checked by a later stage.

The grammar is the same for both roles. After parsing a library file, the parser reports each top-level `Statement` as an error: "code at the top level only runs in the entry file; move it into a function". Top-level `let` is an item, so it is allowed in library files (example 21 has `pub let red = ...`). The rule that a library's top-level `let` values must be computable at compile time is checked later. A top-level `var` in a library file is reported by the same later stage.

---

## 3. Items

### 3.1 Items, attributes and doc comments

```
Item        = Attributes [ "pub" ] ItemKind ;
ItemKind    = FnDecl | StructDecl | EnumDecl | TraitDecl | ExtendDecl
            | ImportDecl | TestDecl | ComptimeBlock | GlobalLet ;

Attributes  = { Attribute [ NEWLINE ] } ;
Attribute   = "@" IDENT { "." IDENT } [ "(" [ ArgList ] ")" ] ;
```

- An attribute is `@` followed by a name and optional arguments. The arguments use the same syntax as call arguments (`ArgList`, section 6.10), so `@derive(Json, Compare)` and `@entry` both parse.
- Attributes may be on their own line before the item (examples 25, 27, 29) or on the same line (`@entry fn main() -> Never { ... }`).
- The parser accepts attributes before any item or member. Which attributes exist and where each one is allowed is checked later.
- **Doc comments** are trivia (see [lexical.md, 4.3](lexical.md#43-doc-comments)). The parser attaches them to the item, field, method, enum option or trait member that starts right after them, before its attributes.

### 3.2 Visibility

`pub` makes an item part of the module's public API. Everything is private by default.

| `pub` is allowed on | `pub` is an error on |
|---|---|
| `fn`, `struct`, `enum`, `trait`, top-level `let` | `import`, `test`, `extend`, `comptime { }`, top-level `var` |
| struct fields | enum options (they share the enum's visibility) |
| methods in `struct`, `enum` and `extend` | trait members (they share the trait's visibility) |

The parser reports `pub` in the wrong place with a fix that removes it. Re-exporting with `pub import` is not in v0.1 ([open question](README.md#open-questions)).

### 3.3 Functions

```
FnDecl        = [ FnModifier ] "fn" LOWER_IDENT [ GenericParams ]
                "(" [ ParamList ] ")" [ "->" Type ] [ ThrowsClause ] [ FnBody ] ;
FnModifier    = "mut" | "static" | "comptime" ;
ParamList     = Param { "," Param } [ "," ] ;
Param         = IDENT [ ":" ParamType ] [ "=" Expr ] ;
ParamType     = [ "mut" ] Type ;
ThrowsClause  = "throws" [ Type ] ;
FnBody        = "=" Expr | Block ;
```

- `fn name(params) -> ReturnType` with an optional `throws` clause and a body.
- **Modifiers.** `mut fn` is a method that may change `self`. `static fn` belongs to the type, not to one value. Both are allowed only inside `struct`, `enum`, `trait` and `extend` bodies. `comptime fn` runs at compile time and is allowed wherever a function is. A function has at most one modifier, and it comes after `pub`: `pub static fn from_hex(...)`.
- **Parameters.** Each parameter is a name with an optional type and an optional default value: `greeting: String = "Hello"`. A parameter with no type is allowed by the grammar. The rule that `pub fn` must declare all parameter and return types (PLAN 4.3) is checked later.
- **`mut` parameters.** `counter: mut Counter` means the function may change the argument. `mut` is allowed only at the start of a parameter's type (section 7.2).
- **Type parameters of `comptime fn`.** A parameter whose type is `type` holds a type and is named like one: `comptime fn field_names(T: type)`. Its name is an `UPPER_IDENT`; every other parameter name is a `LOWER_IDENT` ([lexical.md, 5.3](lexical.md#53-the-naming-rule)).
- **Methods** have no explicit `self` parameter. Inside a method, `self` is a keyword expression.
- **Return type.** No `->` means the function gives no value. `-> Never` means it never returns; `Never` is an ordinary type name to the parser.
- **`throws`.** `throws` alone means the function can fail with any error (examples 16 and 26). `throws AgeError` names the error type. `throws` comes after the return type if there is one: `-> Int throws AgeError`. Because a type can never start with `{` or `=`, the parser knows whether a type follows `throws`.
- **Bodies.** `= expr` for a function whose whole body is one expression, or a block. Either way the function's value is the value of the body (section 4.1). The formatter rewrites a block body that holds exactly one expression statement, `{ x * x }`, to `= x * x` (decision 0004). This holds whatever the return type, so `fn greet(name: String) = print("Hi {name}")` is the formatted form. A body whose only statement is an assignment, a `let` or a loop is not an expression and keeps its block (`mut fn take_damage(amount: Int) { self.health -= amount }`). The formatter rule does not affect parsing: both forms are valid.
- **No body.** A function without a body is a **requirement** and is allowed only directly inside a `trait`. Anywhere else the parser reports "this function needs a body: add `{ }` or `= value`".
- **The body's `{` must be on the same line** as the end of the signature (a `NEWLINE` before it is an error with a fix, see [lexical.md, 12.6](lexical.md#126-worked-examples)). In a trait this matters: `fn area() -> Float` followed by a line break is a complete requirement.

### 3.4 Generic parameters

```
GenericParams = "<" GenericParam { "," GenericParam } [ "," ] [ NEWLINE ] ">" ;
GenericParam  = UPPER_IDENT [ ":" Requirement { "and" Requirement } ] ;
Requirement   = TypePath ;
```

- `<T>`, `<K, V>`, `<T: Compare>`, `<T: Compare and Show>`.
- Several requirements on one parameter are joined with `and` (decision 0004). Here `and` is a separator, not the logic operator; the two never meet because this rule is only used in declarations.
- In a declaration, `<` right after the name always starts generic parameters, so there is no ambiguity with less-than.
- The closing `>` may be split from `>>`, `>=` or `>>=` (section 7.3).
- **Over several lines.** `<` and `,` are in the continue-after set, so the list can be split after them. `<` is not a bracket for the lexer, so a line break before the closing `>` becomes a `NEWLINE`; the rule allows one there, like the import list (section 3.9). So both `fn pair<` / `K: Hash,` / `V: Show` / `>(k: K, v: V)` and the same with a trailing comma work. This applies to declarations only, where `<` is never less-than. `GenericArgs` (section 6.11) has no such `NEWLINE`.
- `where` clauses and default type arguments are not part of v0.1.

### 3.5 Structs

```
StructDecl    = "struct" UPPER_IDENT [ GenericParams ] [ Conformance ]
                "{" [ MemberList ] "}" ;
Conformance   = ":" TypePath { "and" TypePath } ;
MemberList    = Member { MemberSep Member } [ MemberSep ] ;
MemberSep     = NEWLINE | "," ;
Member        = Attributes [ "pub" ] ( Field | FnDecl ) ;
Field         = LOWER_IDENT ":" Type [ "=" Expr ] ;
```

- A struct has fields and methods, in any order.
- **Fields** have a name, a type and an optional default value: `health: Int = 100`. The type is required.
- **Separators.** Fields on separate lines need no commas. Fields on one line are separated with commas: `struct Point { x: Float, y: Float }`. A trailing separator before `}` is allowed. A comma may only follow a field; after a method, the separator must be a line break (the parser reports "put the next member on a new line").
- **Conformance.** `struct Circle: Shape` says the struct has the abilities of the trait `Shape`. Several traits are joined with `and`, like generic requirements: `struct Dog: Describe and Compare`. The examples only show one trait, so this joining word is a choice made in this spec (see [README](README.md#decisions-made-in-this-spec)).
- **Methods** are `FnDecl`s, with any modifier. `pub` before a field or method makes it public.

### 3.6 Enums

```
EnumDecl      = "enum" UPPER_IDENT [ GenericParams ] [ Conformance ]
                "{" [ EnumMemberList ] "}" ;
EnumMemberList= EnumMember { MemberSep EnumMember } [ MemberSep ] ;
EnumMember    = Attributes ( EnumOption | [ "pub" ] FnDecl ) ;
EnumOption    = UPPER_IDENT [ "(" OptionField { "," OptionField } [ "," ] ")" ] ;
OptionField   = LOWER_IDENT ":" Type ;
```

- An enum is a list of options. An option is a name, optionally with named fields: `Sunny`, `Rainy(mm: Float)`, `Rect(width: Float, height: Float)`.
- Option fields are always named. `Circle(Float)` is an error ("name this field, for example `Circle(radius: Float)`"), because building and matching use the names.
- An option with no fields has no parentheses. `Empty()` is an error with a fix that removes them.
- Options are separated like struct members: line breaks, or commas on one line (`enum Light { Red, Yellow, Green }`).
- `enum AgeError: Error` declares a conformance, as for structs.
- **Methods** are allowed in an enum body, after or between the options. The examples don't show this, but traits like `Error` need somewhere to put methods and it keeps enums consistent with structs. This is a choice made in this spec.
- The parser tells an option from a method by its first token: an `IDENT` starts an option; `fn`, a modifier or `pub` starts a method.

### 3.7 Traits

```
TraitDecl     = "trait" UPPER_IDENT [ GenericParams ] "{" [ TraitMembers ] "}" ;
TraitMembers  = TraitMember { NEWLINE TraitMember } [ NEWLINE ] ;
TraitMember   = Attributes FnDecl ;
```

- A trait lists **requirements** (functions with no body) and **default methods** (functions with a body).

  ```quell
  trait Shape {
      fn area() -> Float                       // requirement
      fn summary() -> String = "{self.name()}" // default method
  }
  ```

- Trait members are separated by line breaks only.
- Supertraits (`trait Compare: Equal`) are not in v0.1 ([open question](README.md#open-questions)).

### 3.8 Extend

```
ExtendDecl    = "extend" TypePath [ Conformance ] "{" [ ExtendMembers ] "}" ;
ExtendMembers = ExtendMember { NEWLINE ExtendMember } [ NEWLINE ] ;
ExtendMember  = Attributes [ "pub" ] FnDecl ;
```

- `extend Int { fn is_even() -> Bool = self % 2 == 0 }` adds methods to an existing type, even a built-in one.
- `extend Int: Describe { ... }` also adds a trait conformance.
- An `extend` body holds only methods. Fields can't be added.
- How to extend a generic type for all its type arguments (for example all `List<T>`) is an [open question](README.md#open-questions). The grammar accepts `extend List<Int>`, with concrete type arguments.

### 3.9 Imports

```
ImportDecl    = "import" ( ModulePath [ "." "{" ImportList "}" ] | "c" PLAIN_STRING ) ;
ModulePath    = IDENT { "." IDENT } ;
ImportList    = IDENT { "," IDENT } [ "," ] [ NEWLINE ] ;
```

All the forms in the examples:

| Form | Meaning |
|---|---|
| `import std.fs` | the module `std.fs`, used as `fs.read_text(...)` |
| `import json` | a package's main module, used as `json.encode(...)` |
| `import std.math.{pi}` | one name from a module |
| `import std.math.{pi, sqrt}` | several names |
| `import geometry.point.{Point, distance}` | names from a module in this package |
| `import http.{Server, Request, Response}` | names from a package |
| `import c "sqlite3.h"` | a C header, used as `c.sqlite3_libversion()` |

- A module path is a list of names separated by dots. `geometry/point.ql` is imported as `geometry.point`.
- The name list in `{ }` may be split over several lines, with or without a trailing comma ([lexical.md, 12.6](lexical.md#126-worked-examples)). The braces here hold names, not a block. This is allowed because an import is never in expression position (section 10.11).
- `ModulePath` and the import list both use `.`: after a `.`, an `IDENT` continues the path and `{` starts the list.
- `import c` is recognized when `c` is followed by a string literal. The string must be plain.
- Imports are items, so the grammar allows them anywhere at the top level of a file (never inside a block). Whether they must come before other items is an [open question](README.md#open-questions). Aliases (`import x as y`) are not in v0.1.

### 3.10 Tests

```
TestDecl      = "test" PLAIN_STRING Block ;
```

- `test "multiples of three say Fizz" { assert fizzbuzz(3) == "Fizz" }`.
- `test` is a contextual keyword. The name must be a plain string (holes are an error: "a test name can't contain `{ }` values").
- Test blocks are items, so they are allowed in library files.

### 3.11 Compile-time items

```
ComptimeBlock = "comptime" Block ;
```

- `comptime { assert squares.length() == 16, "the table has the wrong size" }` runs at compile time (example 27).
- `comptime let` is part of `GlobalLet` (next section), and `comptime fn` is part of `FnDecl`.
- In v0.1, `comptime` blocks and `comptime let` are top-level items only. Whether they may also appear inside function bodies is an [open question](README.md#open-questions).

### 3.12 Top-level let and var

```
GlobalLet     = [ "comptime" ] ( "let" | "var" ) BindingPattern [ ":" Type ] "=" Expr ;
```

- `let name = "Ava"`, `pub let red = Color(r: 255, g: 0, b: 0)`, `comptime let squares = ...`.
- The syntax is the same as a `LetStmt` (section 4.3), with optional `pub` (from `Item`) and `comptime` in front.
- Every `let` and `var` at the top level of a file is a `GlobalLet`, including those in scripts such as `let name = "Ava"` and `let (low, high) = min_max(...)` (section 2.1). Only a `let` or `var` inside a block is a `LetStmt`.
- `pub var` and `comptime var` are errors.

---

## 4. Blocks and statements

### 4.1 Blocks and their value

```
Block         = "{" [ Statement { NEWLINE Statement } [ NEWLINE ] ] "}" ;
```

- A block is a list of statements, one per line, inside `{ }`. The braces are always required.
- **Several statements on one line** are an error: after a complete statement, the next token must be `NEWLINE` or `}`. The message is "put each statement on its own line" (Quell has no semicolons).
- **The block's value is its last line** (decision 0004): if the last statement is an expression statement, the block gives that expression's value. Otherwise the block gives no value. This one rule covers function bodies, `if` branches, `match` arms with blocks, lambdas with blocks, `catch` blocks, `unsafe` blocks and plain blocks. Whether a value is needed and has the right type is checked later.
- A block can be used as an expression anywhere (section 6.14). At the start of a statement, `{` always opens a block, because no other Quell construct starts with `{`.

### 4.2 Statements

```
Statement     = LetStmt | AssignStmt | ForStmt | WhileStmt | RepeatStmt
              | DeferStmt | AssertStmt | ExprStmt ;
```

The parser chooses by the first token: `let` or `var` gives a `LetStmt`, `for` a `ForStmt`, `while` a `WhileStmt`, `repeat` a `RepeatStmt`, `defer` a `DeferStmt`, `assert` an `AssertStmt`. Otherwise it parses an expression, and then: if the next token is an assignment operator, the statement is an `AssignStmt`; if not, it is an `ExprStmt`.

Items (functions, structs and so on) cannot be declared inside a block in v0.1. The parser reports "declare this at the top level of the file". Local items are an [open question](README.md#open-questions).

### 4.3 let and var

```
LetStmt       = ( "let" | "var" ) BindingPattern [ ":" Type ] "=" Expr ;
```

- `let` declares a name that can't be changed; `var` declares one that can.
- The pattern is a name or a tuple of names: `let (low, high) = min_max(numbers)` (section 8.5).
- The type annotation is optional: `let lives: Int = 3`, `var stock: Map<String, Int> = [:]`.
- **The initial value is required.** `var x: Int` with no `=` is an error in v0.1 ("give `x` a starting value").
- `let x = value else { ... }` is rejected syntax (section 11).

### 4.4 Assignment

```
AssignStmt    = Place AssignOp Expr ;
AssignOp      = "=" | "+=" | "-=" | "*=" | "/=" | "%="
              | "&=" | "|=" | "^=" | "<<=" | ">>=" ;
Place         = PlaceRoot { "." MemberName | "[" Expr "]" } ;
PlaceRoot     = LOWER_IDENT | "self" ;
```

- The left side must be a **place**: a variable, or a field or element reached from one through `.name` and `[index]` steps. Examples: `score = 0`, `self.health = 100`, `ages["Kai"] = 11`, `tasks[number - 1].done = true`.
- The parser reads the left side as an ordinary expression and then checks that it has the shape of a `Place`. Anything else (a call, `?.`, a literal) gets the error "you can only assign to a variable, a field or an element".
- Assignment is a statement, not an expression. It gives no value and cannot be nested: `let a = b = 1` and `f(x = 1)` are errors. In a call, the message for `f(x = 1)` is "named arguments use `:`: write `f(x: 1)`".
- For the same reason, the expression body of a `match` arm or a lambda can't be an assignment: `Add => total += x` and `items.each(x => total += x)` need braces, `Add => { total += x }`. When an assignment operator follows such a body, the parser reports "an assignment needs braces here" and continues as if the braces were there.
- Whether the place may be changed (a `var`, a `mut` parameter, `self` in a `mut fn`) is checked later.
- Tuple assignment (`(a, b) = (b, a)`) is not in v0.1. There is no compound assignment for `div`.

### 4.5 for

```
ForStmt       = "for" BindingPattern "in" HeaderExpr Block ;
```

- `for fruit in fruits { ... }`, `for i in 0..<3 { ... }`, `for (i, s) in scores.enumerate() { ... }`, `for (name, age) in ages { ... }`.
- The expression after `in` ends at the `{` of the body, because `{` never continues an expression (section 10.11).
- `for` is a statement. `parallel for` is an expression (section 6.17).

### 4.6 while

```
WhileStmt     = "while" HeaderExpr Block ;
```

- `while countdown > 0 { ... }`. No parentheses are needed around the condition.

### 4.7 repeat

```
RepeatStmt    = "repeat" [ HeaderExpr ] Block ;
```

- `repeat 3 { ... }` runs the block 3 times. `repeat { ... }` runs it until a `break` (or forever).
- If the token after `repeat` is `{`, it always opens the body. So `repeat { ... }` never treats the body as the count.

### 4.8 defer

```
DeferStmt     = "defer" Block ;
```

- `defer { print("finished") }` runs the block when the enclosing block ends, however it ends.
- `defer` takes a block only, as in all examples.

### 4.9 assert

```
AssertStmt    = "assert" Expr [ "," Expr ] ;
```

- `assert p.health == 0`, or with a message: `assert squares.length() == 16, "the table has the wrong size"`.
- The message comes after a comma. The comma is safe because the condition can't contain a comma outside brackets.
- `assert` is a statement, so it can't be used as a value.

### 4.10 Expression statements

```
ExprStmt      = Expr ;
```

- Any expression can be a statement: calls (`print(x)`), `if`, `match`, `group { }`, `try save(tasks, file)`, `return x`, `break`.
- **A statement that starts with `-` or `~`** right after an expression statement, a `let` or `var`, or an assignment in the same block (or file) gets a **parser warning**, because it is most likely the second half of a split line: "this line starts with `-`, so it is a new statement. To continue the line above, end that line with `-`. To make a negative value here on purpose, write `(-penalty)`." This matters most on the last line of a block, where `-penalty` silently becomes the block's value ([lexical.md, 12.6](lexical.md#126-worked-examples)).
- An expression statement whose value is not used and that has no effect (such as a lone `-discount` in the middle of a block) is also reported later as a warning.

### 4.11 `if x = 5` is a compile error

Decision 0003 says writing `=` where `==` was meant is a compile error with a fix. **The parser reports it.** The condition of `if` and `while`, and a `match` guard, is parsed as an expression. An expression never contains `=` at its top level, so after `if x` the parser expects `{`. When it finds `=` instead, it reports:

```
error: `=` gives a variable a new value. To compare, use `==`.
  if temperature = 23 { ... }
                 ^ help: write `==`
```

and then continues as if `==` had been written. The parser can do this alone because it knows it is in a condition, and nothing else could make the code valid.

---

## 5. Precedence and associativity

From the **tightest** binding (level 1) to the **loosest** (level 17):

| Level | Operators | Kind | Associativity | Example |
|---|---|---|---|---|
| 1 | `f(x)`, `f<T>(x)`, `.name`, `?.name`, `[i]` | postfix | left | `movies.filter(f)[0].title` |
| 2 | `-x`, `~x`, `try x`, `spawn x` | prefix | right | `-x.abs()` is `-(x.abs())` |
| 3 | `*` `/` `div` `%` | binary | left | `a * b div c` |
| 4 | `+` `-` | binary | left | `a - b + c` is `(a - b) + c` |
| 5 | `<<` `>>` | binary | left | `1 << n + 1` is `1 << (n + 1)` |
| 6 | `&` | binary | left | |
| 7 | `^` | binary | left | |
| 8 | <code>&#124;</code> | binary | left | `a & m == 0` is `(a & m) == 0` |
| 9 | `..<` `..=` | binary | **none** | `0..<n + 1` is `0..<(n + 1)` |
| 10 | `??` | binary | right | `a ?? b ?? c` is `a ?? (b ?? c)` |
| 11 | `catch` | binary (two forms) | left | `parse(s) catch 0` |
| 12 | `is` | binary, right side is a type | **none** | `s is Circle` |
| 13 | `==` `!=` `<` `<=` `>` `>=` | binary | **none** (chaining is an error) | `a + 1 < b` |
| 14 | `not` | prefix | right | `not a == b` is `not (a == b)` |
| 15 | `and` | binary | left | |
| 16 | `or` | binary | left | `a or b and c` is `a or (b and c)` |
| 17 | `x => e`, `return e`, `throw e`, `break`, `continue` | prefix forms | extend as far right as possible | `m => m.year >= 2010 and m.year < 2020` |

Notes on the choices:

- **Bitwise operators bind tighter than comparisons.** C's order, where `a & mask == 0` means `a & (mask == 0)`, is a famous trap. Quell follows Rust and Swift instead.
- **`not` binds looser than comparisons**, as in Python: `not x == 5` reads as "not (x equals 5)", which is what a beginner means. `not raining and warm` is still `(not raining) and warm`. A `not` directly after a comparison operator (`a == not b`), or after any other operator of levels 3 to 13 (`x ?? not y`, `a + not b`), is an error with the fix "wrap it in parentheses: `a == (not b)`". After `and` and `or` it is fine: `warm and not raining`.
- **`??` and `catch` sit between ranges and comparisons**, like Kotlin's `?:` and Zig's `catch`. So `x ?? 0 > 5` is `(x ?? 0) > 5`. `??` is right-associative so that chains of fallbacks read left to right.
- **A fallback with arithmetic needs parentheses.** By the table, `count ?? 0 + 1` would be `count ?? (0 + 1)`, but a reader going left to right expects `(count ?? 0) + 1`. Both type-check, so the program would be silently wrong. So the right operand of `??`, and of the fallback form of `catch`, may not have a binary operator of levels 3 to 9 (`*`, `/`, `div`, `%`, `+`, `-`, `<<`, `>>`, `&`, `^`, <code>&#124;</code>, `..<`, `..=`) at its top level. The parser reports "add parentheses to say which you mean: `(count ?? 0) + 1` or `count ?? (0 + 1)`". Simple fallbacks are not affected: `?? 0`, `?? -1`, `?? "guest"`, `?? f(x)`, `?? a.b`, `catch []`, a jump, and another `??`.
- **Ranges, `is` and comparisons are non-associative.** `a..<b..<c`, `x is A is B` and `a < b < c` are errors.
- **Chained comparisons are an error**, not a feature: `a < b < c` gets "comparisons can't be chained: write `a < b and b < c`". This keeps one meaning for `<` and avoids the different behaviours of Python (chains) and C (compares a Bool to a number).
- **Prefix operators bind tighter than binary ones but looser than postfix ones.** `-2.pow(2)` is `-(2.pow(2))`. `try a.b().c()` covers the whole chain.
- **Level 17 forms** are not operators in a table sense. They can only appear where the grammar allows a full expression (section 6.1). There, their right side takes everything to the end of the expression. Jumps (`return`, `throw`, `break`, `continue`) may also appear on the right of `??` and `catch`; lambdas may not. In that position a jump's value stops at level 10 (section 6.6), so `x ?? return a + b` returns `a + b`, while `x ?? return false or y` is an error with a fix.

**Where a table-driven parser needs special cases.** The chain of rules in section 6 and this table describe the same language. A precedence-climbing parser built from the table must add these checks to match the chain exactly:

1. Levels 9, 12 and 13 are non-associative: a second operator of the same level is an error, not a loop.
2. `not` (level 14) may not be the operand of an operator of levels 3 to 13 (`a == not b`, `x ?? not y`). That is an error with a fix (section 9).
3. Lambdas and jumps are parsed only at the start of a full `Expr` (section 6.1), and jumps also as the right operand of `??` and `catch`, where their value is parsed at level 10 (section 6.6).
4. The right operand of `??` and of the fallback form of `catch` may not have a top-level operator of levels 3 to 9 (the note above).
5. The binding form of `catch` ends its operand: after its block, only another `catch` or an operator of level 12 or looser may follow (section 6.5).
6. In the header of `if`, `while`, `for`, `parallel for`, `match` and `repeat`, the expression is a `HeaderExpr` (section 6.1).

---

## 6. Expressions

### 6.1 The top rule

```
Expr          = LambdaExpr | JumpExpr | OrExpr ;
HeaderExpr    = OrExpr ;
```

- `Expr` is used wherever the grammar needs "an expression": values of `let` and assignments, statements, call arguments, list elements, map keys and values, indexes, `match` arm bodies, lambda bodies, default values, `= expr` function bodies, `assert` parts and string holes.
- **Headers.** The expression right before a body block (the condition of `if` and `while`, the items of `for` and `parallel for`, the scrutinee of `match` and the count of `repeat`) is a `HeaderExpr`: an `OrExpr`, so it can't be a lambda or a jump as a whole. A lambda or jump there would take the body `{` as part of itself (`for x in a => { ... }` would read the body as the lambda's body). A jump can still appear on the right of `??` or `catch` inside a header, as in `for line in read_lines(path) ?? return { ... }` (section 6.19 says why that `return` takes no value).
- **Lambdas and jumps (`return`, `throw`, `break`, `continue`) are only allowed as a whole `Expr`.** The one exception: a jump may also be the right side of `??` or `catch` (sections 6.5 and 6.6), as in `?? return none`. `a + return b`, `a + x => x` and `f ?? x => x` are errors ("wrap this in parentheses"). This keeps the right side of each lambda and jump easy to see.
- **How the parser recognizes a lambda** at the start of an `Expr` (looking ahead, without consuming anything):
  1. an `IDENT` or `_` followed by `=>`;
  2. `mut` followed by an `IDENT` or `_` followed by `=>`;
  3. `(`, and the token right after the matching `)` is `=>`.

  If none of these match, the expression is not a lambda. Rules 1 and 2 accept a name of either class, because a lambda parameter is a declaration site (section 1.2): `Item => Item.name` is recognized as a lambda and then gets the naming error with the fix `item => item.name`. Nothing else can start with a name followed by `=>` here, because `match` arm patterns are read with the pattern grammar and guards are an `OrExpr` (section 10.5). So capitalization does not change how a lambda is recognized ([lexical.md, 5.4](lexical.md#54-what-the-parser-relies-on)).

  Rule 3 is one step, not a scan: the lexer records the matching closer of every bracket ([lexical.md, 12.2](lexical.md#122-the-algorithm)), so the parser looks at the token after it directly. Nested parentheses therefore cost nothing extra.

### 6.2 Logic: or, and, not

```
OrExpr        = AndExpr { "or" AndExpr } ;
AndExpr       = NotExpr { "and" NotExpr } ;
NotExpr       = "not" NotExpr | CompareExpr ;
```

- `temperature > 15 and not raining`, `weekend or temperature > 25`, `age < 0 or age > 150`.
- `and` binds tighter than `or`. Both are left-associative.

### 6.3 Comparisons

```
CompareExpr   = TypeTestExpr [ CompareOp TypeTestExpr ] ;
CompareOp     = "==" | "!=" | "<" | "<=" | ">" | ">=" ;
```

- At most one comparison operator per `CompareExpr`. A second one is an error with the fix shown in section 5.
- When `>` is used here, a `NEWLINE` directly after it is skipped ([lexical.md, 12.5](lexical.md#125-lines-that-end-with-a-greater-than-sign)). The same applies to `>>` in `ShiftExpr`.

### 6.4 Type tests

```
TypeTestExpr  = CatchExpr [ "is" Type ] ;
```

- `s is Circle` checks the exact type. The right side is parsed with the type grammar (section 7), not as an expression.
- Narrowing (after `if s is Circle`, `s.radius` works) is a later stage's job.
- Two habits from other languages get their own message, because the type grammar would otherwise just say "expected a type": `shape is not Circle` (write `not (shape is Circle)`) and `user is none` (write `user == none`).

### 6.5 catch

```
CatchExpr     = CoalesceExpr { "catch" CatchRight } ;
CatchRight    = CatchBinding Block | FallbackJump | CoalesceExpr ;
CatchBinding  = LOWER_IDENT | "_" ;
```

There are two forms:

| Form | Example | Meaning |
|---|---|---|
| fallback | `parse_age("abc") catch 0` | if the left side fails, use the right side |
| binding | `parse_age("200") catch err { ... }` | if the left side fails, run the block with the error named `err`; the block's last line is the value (example 14) |

**The rule that tells them apart:** after `catch`, if the next token is a `LOWER_IDENT` or `_` **and** the token after that is `{`, it is the binding form. In every other case it is the fallback form, and the right side is a `CoalesceExpr` (or a jump).

- The rule needs no backtracking: two tokens of lookahead decide it.
- It never takes away a valid fallback: in Quell, a lowercase name directly followed by `{` can never be an expression (there are no trailing lambdas and no struct literals with braces).
- An uppercase name after `catch` is always a fallback value, so `load() catch Empty` works even before a `{`.
- `catch _ { ... }` runs the block and ignores the error.
- `try load() catch 0` is `(try load()) catch 0`: `try` passes the error up before `catch` can see it. When the whole left side of `catch` is a `try` expression, the parser gives a warning: "`try` passes the error up, so this `catch` never runs; remove `try`".
- `catch` is left-associative: `a() catch b() catch 0` is `(a() catch b()) catch 0`.
- **The binding form ends its operand.** After the block of `catch err { ... }`, the next token may only be another `catch`, `is`, a comparison operator, `and`, `or`, or a token that ends the expression. A postfix operator (`.abs()`, even at the start of the next line) or an operator of levels 3 to 10 (`+ 1`, `?? 0`) is an error: "wrap the `catch` in parentheses: `(parse(s) catch e { 0 }) + 1`". This is what the chain of rules says, and a table-driven parser must check it (section 5).
- **The fallback** is a `CoalesceExpr` with no top-level arithmetic, bitwise or range operator (section 5): `parse(a) catch 0 + parse(b) catch 0` is an error with the fix to add parentheses, instead of silently meaning `(parse(a) catch (0 + parse(b))) catch 0`.
- A line may start or end with `catch`; either way it joins the two lines (`catch` is in both the continue-before and the continue-after sets).
- The one trap is a fallback variable in a condition: see section 10.4.

### 6.6 Defaults: ??

```
CoalesceExpr  = RangeExpr [ "??" ( FallbackJump | CoalesceExpr ) ] ;
FallbackJump  = "return" [ CoalesceExpr ] | "throw" CoalesceExpr | "break" | "continue" ;
```

- `ages["Sam"] ?? 0`, `find_user(users, "Sam")?.email ?? "no email"`.
- **Leaving early** (decision 0004): the right side may be `return`, `throw`, `break` or `continue`. `let email = user.email ?? return none`, `let age = trimmed.to_int() ?? throw NotANumber(text: trimmed)`. A call that never returns, such as `exit(1)`, is an ordinary expression to the parser.
- **A jump's value here stops at level 10.** On the right of `??` or `catch`, the value of `return` or `throw` is a `CoalesceExpr`, not a full `Expr`. So in `cached_flag ?? return false or fallback_ok`, the value is just `false`. If the next token is an operator looser than `??` (`catch`, `is`, a comparison, `and`, `or`), the parser reports "this is unclear: write `(cached_flag ?? return false) or fallback_ok` to leave early, or `?? return (false or fallback_ok)` to return the whole value". Without this rule the value would silently swallow `or fallback_ok`. `return` takes a value only under the conditions of section 6.19.
- **No top-level arithmetic on the right** (section 5): `count ?? 0 + 1` is an error with a fix. Write `(count ?? 0) + 1` or `count ?? (0 + 1)`.
- Right-associative: `a ?? b ?? c` is `a ?? (b ?? c)`.
- There is no `??=`. `cache ??= load()` lexes as `??` followed by `=`, and the parser reports "Quell has no `??=`: write `cache = cache ?? load()`".
- `not` can't be the right operand: `flag ?? not visible` is an error with the fix `flag ?? (not visible)` (section 5).

### 6.7 Ranges

```
RangeExpr     = BitOrExpr [ ( "..<" | "..=" ) BitOrExpr ] ;
```

- `0..<10` stops before 10. `1..=10` includes 10. There is no bare `..` (it is a lexer error token).
- Both ends are required in v0.1. Open ranges like `5..=` are not in v0.1.
- Non-associative: `a..<b..<c` is an error.
- To call a method on a range, use parentheses: `(0..<16).map(n => n * n)`. Without them, `0..<10.step(2)` is `0..<(10.step(2))`. When the right end of a range is a number literal followed by `.`, the parser gives a warning with the fix `(0..<10).step(2)`.

### 6.8 Bitwise and arithmetic operators

```
BitOrExpr     = BitXorExpr { "|" BitXorExpr } ;
BitXorExpr    = BitAndExpr { "^" BitAndExpr } ;
BitAndExpr    = ShiftExpr { "&" ShiftExpr } ;
ShiftExpr     = AddExpr { ( "<<" | ">>" ) AddExpr } ;
AddExpr       = MulExpr { ( "+" | "-" ) MulExpr } ;
MulExpr       = PrefixExpr { ( "*" | "/" | "div" | "%" ) PrefixExpr } ;
```

- `/` is exact division (`7 / 2 == 3.5`), `div` is whole-number division (`7 div 2 == 3`), `%` is the remainder. These meanings are for the type checker; the parser only knows the precedence.
- All of these are left-associative.

### 6.9 Prefix operators

```
PrefixExpr    = ( "-" | "~" | "try" ) PrefixExpr | SpawnExpr | PostfixExpr ;
SpawnExpr     = "spawn" ( Block | PostfixExpr ) ;
```

- `-x` negates, `~x` is bitwise not.
- **Negative number literals.** When `-` is applied directly to an `INT` or `FLOAT` literal, the later check that the number fits its type uses the negated value, so `-9223372036854775808` is a valid `Int` ([lexical.md, 7.1](lexical.md#71-integer-literals)). This matches negative literal patterns (section 8.1).
- **`-5.abs()` is `-(5.abs())`**, which is -5, because postfix operators bind tighter. When `-` is followed by a number literal and then `.`, the parser gives a warning: "this is `-(5.abs())`; to call the method on -5, write `(-5).abs()`".
- `try f()` passes an error up. Its operand is everything at level 2 or tighter, so `try fs.read_text(path)` covers the whole call chain, and `try a() + b()` is `(try a()) + b()`.
- `spawn` starts a task. Its operand is a block (`spawn { ... }`) or a postfix expression that **ends in a call** (`spawn download("photos.zip")`). If the token after `spawn` is `{`, it is always the block form. Anything else, such as `spawn x`, is an error: "`spawn` needs a function call or a block". Like `try`, `spawn` covers the whole postfix chain: `spawn a.b()` spawns the call `a.b()`.
- `not` is not here: it is at level 14 (section 6.2).

### 6.10 Postfix operators and calls

```
PostfixExpr   = Primary { PostfixOp } ;
PostfixOp     = CallSuffix
              | "." MemberName [ GenericArgs ]
              | "?." MemberName [ GenericArgs ]
              | "[" Expr "]" ;
MemberName    = IDENT | a reserved keyword ;
CallSuffix    = "(" [ ArgList ] ")" ;
ArgList       = Arg { "," Arg } [ "," ] ;
Arg           = [ IDENT ":" ] ( LambdaExpr | "mut" Place | Expr ) ;
```

- **Calls.** `print(x)`, `greet("Sam", greeting: "Welcome")`, `heal(mut hero, 20)`. Creating a value uses the same syntax: `Point(x: 1.0, y: 2.0)`, `Rainy(mm: 2.5)`, `Stack<String>()`.
- **Named arguments** are `name: value`. An identifier followed by `:` at the start of an argument is always an argument name, since no expression contains `name :` outside brackets. The name may be of either class, because a parameter of type `type` has an uppercase name (`field_names(T: Point)`). Whether the names exist, and whether named and positional arguments are in a valid order, is checked later.
- **How an argument is read.** After the optional `name :`, the three alternatives are tried in order, so each argument has one parse. First the lambda test of section 6.1 (rules 1 to 3): `mut entries => ...` and `on_change: mut list => ...` are lambdas with a `mut` parameter. Otherwise, a `mut` starts a **`mut` argument**, and what follows must be a `Place` (section 4.4): a variable, or a field or element reached from one. The parser reads a postfix expression and checks its shape, as it does for assignment. Anything else, such as `mut 5`, `mut f()` or `heal(mut hero.health + 1)`, gets "only a variable, a field or an element can be passed with `mut`". Otherwise the argument is an ordinary `Expr`.
- **`mut` arguments** (`bump(mut c)`) show that the call may change the value (section 10.7).
- **Trailing commas** are allowed, so long calls can be split one argument per line (example 21).
- **Member access.** `.name` reads a field or names a method. A method call is a member followed by a call: `scores.length()`. Methods always use parentheses, even with no arguments ([examples/README.md](../examples/README.md), item 5). Generic arguments may follow a member name: `text.parse<Int>()`.
- **Optional member access.** `?.name`: if the value on the left is `none`, the result is `none`. Syntactically, `?.` is just a postfix operator; how far the "skip if missing" reaches along the rest of the chain is a semantic question. Generic arguments may follow it too, exactly as after `.`: `maybe_text?.parse<Int>()`.
- **Keywords as member names.** After `.` or `?.`, a reserved keyword is read as a name ([lexical.md, 6.1](lexical.md#61-reserved-keywords)), because nothing else can follow there. This lets Quell reach outside names that happen to be Quell keywords, such as `c.div(7, 2)` from C's `stdlib.h` or a JSON field named `in`. Declaring a Quell field or method with a keyword name is not possible in v0.1 ([open question](README.md#open-questions)).
- **Indexing.** `scores[0]`, `ages["Kai"]`, `world[hall]`. Only one index expression. A range is one expression, so `text[0..<2]` is allowed by the grammar.
- **Only names follow `.`.** `pair.0` (numbered access to a tuple element) is not in v0.1. Tuples are taken apart with `let (a, b) = pair` or with `match` ([open question](README.md#open-questions)).
- **Postfix operators must be on the same line** as what they apply to, except `.` and `?.`, which may start the next line. Outside `( )` and `[ ]`, a `(` or `[` at the start of a line always starts a new statement. Inside them the line break is removed, so it continues the line above, and the parser warns about a likely forgotten comma (section 6.12).

### 6.11 Generic arguments in expressions

```
GenericArgs   = "<" Type { "," Type } [ "," ] ">" ;
```

`List<Int>()`, `Stack<String>()`, `Arena<Room>()`, `Mutex(List<String>())` and `channel<String>()` put generic arguments in an expression. There, `<` could also be less-than. This is the rule that decides (the same idea as Swift and C#):

> When the parser is at a `<` that is the next token after the name in a `NameExpr`, or after the member name that follows `.` or `?.`, it tries to read `GenericArgs` with the type grammar, without committing. It accepts them as generic arguments only if all three hold:
>
> 1. the type grammar reads a complete `GenericArgs`;
> 2. the outermost closing `>` uses up its token completely: it is a whole `>` token, or the `>` left over after an inner list split a `>>`. It must not split a token and leave `>`, `=` or `>=` behind (section 7.3);
> 3. the token after the closing `>` is `(`, `{`, or a token that **cannot start an expression**.
>
> Otherwise the parser goes back to the `<` and reads it as less-than.

"The next token" means spaces don't matter: `Stack <String>()` is the same as `Stack<String>()` ([lexical.md, 3](lexical.md#3-whitespace-and-line-breaks)).

The tokens that can start an expression are: any identifier, `INT`, `FLOAT`, `CHAR`, `STRING`, `_`, `(`, `[`, `{`, `-`, `~`, and the keywords `self`, `true`, `false`, `none`, `not`, `try`, `spawn`, `mut`, `if`, `match`, `group`, `unsafe`, `return`, `throw`, `break` and `continue`. `mut` and `_` are on the list only because they can be the first token of a lambda (`mut list => ...`, `_ => 0`); neither starts an expression on its own.

Rule 3 lets `{` follow the closing `>` even though `{` can start an expression. A comparison against a block (`f(a < b, c > { x })`) is not worth keeping, while a generic type compared as a value right before a body is natural `comptime` code: `if T == List<Int> { ... }` and `match T == Map<String, Int> { ... }`.

**The speculation is bounded and has no side effects.**

- **Each `<` is tried once.** The result of reading `GenericArgs` with the type grammar from a given `<` token (failure, or success with the position where it ended) depends only on the tokens. The parser remembers it per `<` token, and a nested `<` reached during an outer attempt is stored too. So input like `a < (b < (c < d))` costs time in proportion to its length, not its square.
- **It fails fast.** The type grammar stops at the first token that can't be part of a type. Only identifiers, `.`, `,`, `<`, `>`, `>>`, `>=`, `>>=`, `?`, `(`, `)` and `->` can be.
- **Nothing is kept from a failed attempt.** Any split of `>>`, `>=` or `>>=` (section 7.3) is undone, and no diagnostics are reported. Only the attempt that is accepted has an effect.

Why this is safe:

- The type grammar fails quickly on anything that is not a type (`a < b + c` fails at `+`; `age < 0` fails at `0`), so ordinary comparisons are never read as generics.
- If the generic reading succeeds, the less-than reading would have to read `a < b > ...` as a chain of two comparisons, which Quell already rejects. So choosing the generic reading takes away no valid program, except in one case: **two comparisons around a comma** in any comma-separated list (call arguments, tuples, lists, maps). `f(a < b, c > (d))` is read as `f` called with one argument, `a<b, c>(d)`. To pass two comparisons, write `f((a < b), c > (d))` or `f(a < b, c > d)`. Inside `[ ]` and `( )` line breaks disappear, so the same trap can hide across lines:

  ```quell
  let flags = [
      low < x,
      high > (y),
  ]
  ```

  This is a list with one element, `low<x, high>(y)`. When the generic reading wins and its `<` and its closing `>` are on different lines, the parser gives a warning: "this reads as generic arguments `low<x, high>(...)`; to compare, wrap each comparison in parentheses".
- Rule 2 keeps `a < b >> c` (a comparison with a shift) and `a < b >= c` from being misread. `List<List<Int>>()` still works, because the inner list takes the first `>` of `>>` and the outer list takes the `>` that is left.
- It also works for a generic type used as a value, such as `field_names(List<Int>)`, because `)` cannot start an expression.

### 6.12 Primary expressions

```
Primary       = Literal | STRING | NameExpr | "self" | ParenExpr | ListExpr
              | Block | IfExpr | MatchExpr | ParallelFor | GroupExpr | UnsafeExpr ;
Literal       = INT | FLOAT | CHAR | "true" | "false" | "none" ;
NameExpr      = IDENT [ GenericArgs ] ;
ParenExpr     = "(" Expr ")"
              | "(" Expr "," Expr { "," Expr } [ "," ] ")" ;
ListExpr      = "[" "]"
              | "[" ":" "]"
              | "[" Expr { "," Expr } [ "," ] "]"
              | "[" MapEntry { "," MapEntry } [ "," ] "]" ;
MapEntry      = Expr ":" Expr ;
```

- **Names** are plain identifiers, of either case: `score`, `Player`, `Sunny`, `print`. A dotted name like `Weather.Rainy` or `geometry.point.square` is a name followed by member access. Name resolution decides later whether a name is a variable, a function, a type, a module or an enum option.
- **Parentheses** group an expression: `(Int(x) + Int(y)) div 2`. With a comma, they make a **tuple**: `(low, high)`, `(request.method, request.path)`. A tuple has at least two elements. `()` and `(x,)` are errors in v0.1 (the unit value and one-element tuples are [open questions](README.md#open-questions)). A trailing comma is allowed when there are two or more elements.
- **Lists** are `[1, 2, 3]`; `[]` is the empty list.
- **Maps** are `["Ava": 12, "Sam": 14]`; `[:]` is the empty map. The parser reads the first element, then looks at the next token: `:` means a map, anything else a list. Mixing the two in one literal is an error. This is why `{` is never needed for maps (decision 0003).
- Trailing commas are allowed in lists and maps, so they can be written one element per line.
- **A forgotten comma.** Inside `( )` and `[ ]` line breaks are removed ([lexical.md, 12.2](lexical.md#122-the-algorithm)), so a missing comma at the end of a line can silently join two elements: `[` / `a` / `-b` / `]` is the one-element list `[a - b]`, and `[` / `(1, 2)` / `(3, 4)` / `]` is a call `(1, 2)(3, 4)`. The parser checks this from tokens alone. When the innermost bracket is `(` or `[`, and a `-` used as a binary operator, or a `(` or `[` used as a call or an index, is the **first token on its line**, it reports "did you forget a comma at the end of the line above?". In a list or map literal this is an **error**, because one element per line is the normal style there. Elsewhere (call arguments, tuples, parentheses) it is a **warning**. To continue a subtraction on purpose, put the `-` at the end of the line above, which is the style the newline rules ask for anyway, and the warning goes away ([lexical.md, 12.4](lexical.md#124-the-continue-before-set)).

### 6.13 String literals and holes

A `STRING` token carries its holes as lists of tokens ([lexical.md, 9.3](lexical.md#93-interpolation)). For each hole, the parser parses those tokens as **exactly one `Expr`**, with all line breaks ignored. Leftover tokens are an error ("only one value can go inside `{ }`"). The format spec, if any, is already checked by the lexer and is kept with the hole.

### 6.14 Block expressions

A `Block` (section 4.1) is an expression. Its value is its last line.

```quell
let total = {
    let base = price * 3
    base + shipping
}
```

### 6.15 if

```
IfExpr        = "if" HeaderExpr Block [ "else" ( Block | IfExpr ) ] ;
```

- `if` is an expression: `let clothes = if temperature > 20 { "t-shirt" } else { "jacket" }`. There is no separate ternary operator.
- `else if` chains are an `IfExpr` in the `else` branch. That `IfExpr` is not a `Primary`, so the parser reads it **without** the postfix loop of section 6.10. A postfix operator after the whole chain applies to the outermost `if`: in `if a { xs } else if b { ys } else { zs }.length()`, `.length()` applies to the value of the whole chain, not to `{ zs }` or to the inner `if`. Such code is hard to read; the formatter wraps the chain in parentheses.
- The condition needs no parentheses. It ends at the `{` of the block.
- `else` may start a new line (it is in the continue-before set).
- Using an `if` without `else` as a value is checked later.
- `if let` is rejected syntax (section 11). After `if`, the token `let` or `var` gets the error and its fix.

### 6.16 match

```
MatchExpr     = "match" HeaderExpr "{" [ MatchArm { ArmSep MatchArm } [ ArmSep ] ] "}" ;
ArmSep        = NEWLINE | "," ;
MatchArm      = Pattern [ "if" OrExpr ] "=>" Expr ;
```

- `match weather { Sunny => "Sunglasses!" ... }`. A `match` is an expression; its value is the value of the chosen arm.
- **Arms are separated by line breaks.** Commas also work, so a short `match` can be written on one line: `match n { 0 => "zero", _ => "many" }`. This mirrors the separator rule for struct fields.
- Each arm is a pattern (section 8), an optional **guard** after `if`, `=>`, and an expression. A block arm (`Equal => { ... }`) is an ordinary block expression.
- **The guard is an `OrExpr`**, not a full `Expr`. So a guard can never be a lambda, and in `Some(x) if ready => ...` the `ready =>` is not mistaken for a lambda (section 10.5).
- Whether the arms cover every case is checked later.

### 6.17 Concurrency and unsafe expressions

```
ParallelFor   = "parallel" "for" BindingPattern "in" HeaderExpr Block ;
GroupExpr     = "group" Block ;
UnsafeExpr    = "unsafe" Block ;
```

- `let squares = parallel for n in numbers { slow_square(n) }` runs the body for every item at the same time and gives a list of the results in order. `parallel` is a contextual keyword, recognized when it is directly followed by `for`.
- `group { ... }` runs the tasks started inside it with `spawn` and waits for all of them at the closing brace. `spawn` is a prefix operator (section 6.9).
- `unsafe { ... }` is an expression, so it can be a function body: `fn sqlite_version() -> String = unsafe { ... }`.
- What tasks may read or change, and what is allowed inside `unsafe`, are later stages' rules.

### 6.18 Lambdas

```
LambdaExpr    = LambdaParams "=>" Expr ;
LambdaParams  = [ "mut" ] LambdaName
              | "(" [ LambdaParam { "," LambdaParam } [ "," ] ] ")" ;
LambdaParam   = [ "mut" ] LambdaName [ ":" Type ] ;
LambdaName    = LOWER_IDENT | "_" ;
```

All the lambda forms:

| Form | Example |
|---|---|
| one parameter | `s => s + 5` |
| several parameters | `(a, b) => a + b` |
| typed parameter | `(text: String) => text.upper() + "!"` |
| no parameters | `() => print("hi")` |
| a parameter the lambda changes | `mut entries => entries.append("{name} joined")` |
| a block body | `(m: Movie) => { let age = 2026 - m.year ... }` |
| an unused parameter | `_ => 0` |

- **The body** is an `Expr` and extends as far to the right as possible: in `m => m.year >= 2010 and m.year < 2020` the body is the whole `and` expression. It stops at a token that can't continue an expression, such as `,` or `)` in an argument list, or a `NEWLINE`.
- **A block body** is a block expression; its last line is the lambda's value.
- **`(a, b) =>` always means two parameters.** A lambda parameter is a name, never a pattern, so a lambda can't take a tuple apart the way `for (name, age) in ages` does. `ages.map((name, age) => age)` is a lambda with two parameters; if `map` passes one tuple, a later stage reports the mismatch (and can suggest `entry => ...` or a `for` loop). Taking tuples apart in lambda parameters is not in v0.1 ([open question](README.md#open-questions)).
- **Parameter names** are declaration sites, so an uppercase name such as `Item => Item.name` is read as a lambda and gets the naming error with a fix (section 6.1).
- **`mut` comes before the parameter name** (decision 0004). A typed `mut` parameter is written `(mut list: List<Int>) => ...`. Writing `(list: mut List<Int>) =>` is an error with a fix that moves `mut`. Function parameters put `mut` in the type instead (`counter: mut Counter`); this difference comes from the settled decisions and is listed as an [open question](README.md#open-questions).
- Declaring the lambda's return type is not in v0.1.

### 6.19 Jump expressions

```
JumpExpr      = "return" [ Expr ] | "throw" Expr | "break" | "continue" ;
```

- `return`, `throw`, `break` and `continue` never produce a value. They can be used as statements, as the whole body of a block or arm, and on the right of `??` and `catch`.
- **`return` with or without a value.** `return` takes a value if the next token can start an expression (the list in section 6.11) **and is not `{`**. So `if b == 0 { return 0.0 }` returns `0.0`, and `if done { return }` returns nothing because `}` can't start an expression.
- **`return` before `{` takes no value.** This keeps `?? return` working in headers: in `for line in read_lines(path) ?? return {`, the `{` opens the loop's body; it is not a block returned as the value. To return the value of a block expression, wrap it in parentheses: `return ({ ... })`. When a `return` directly followed by `{` is a statement of its own, the parser says exactly that: "to return the value of this block, write `return ({ ... })`".
- **Line breaks.** Outside `( )` and `[ ]`, a `return` at the end of a line never takes the next line as its value, because the line break becomes a `NEWLINE`. Inside `( )` and `[ ]` line breaks are removed ([lexical.md, 12.2](lexical.md#122-the-algorithm)), so there the value may start on the next line: in `send(` / `value ?? return` / `none_marker` / `)`, the `return` returns `none_marker`. The parser gives a warning when a jump inside brackets takes a value that starts on a later line: "this `return` takes `none_marker` from the next line as its value; put them on one line to make that clear".
- `throw` always needs a value: `throw Empty`, `throw OutOfRange(age: age)`.
- `break` and `continue` take no value and no label in v0.1. `break 5` is an error ("`break` doesn't take a value").

---

## 7. Types

### 7.1 Type syntax

```
Type          = TypeAtom [ "?" ] ;
TypeAtom      = TypePath
              | "any" TypePath
              | "type"
              | ParenType ;
TypePath      = IDENT { "." IDENT } [ GenericArgs ] ;
ParenType     = "(" [ Type { "," Type } [ "," ] ] ")" [ "->" Type ] ;
```

| Type | Example |
|---|---|
| a name | `Int`, `String`, `Player`, `T`, `Never` |
| a path | `http.Request`, `geometry.point.Point` |
| generic arguments | `List<Int>`, `Map<String, Int>`, `List<Handle<Room>>` |
| optional | `String?`, `User?`, `List<Int>?` |
| tuple | `(Int, Int)` |
| function | `(Int) -> Int`, `(String, Int) -> Bool`, `() -> String` |
| dynamic | `any Shape`, `List<any Shape>` |
| the type of types | `type` (in `T: type`) |
| changeable parameter | `mut Counter` (parameter types only, section 7.2) |

- **Optional types** have one `?` at the end. `String??` (which the lexer reads as the `??` token) and `String? ?` are errors: "a type can only be optional once".
- **Parenthesized types** are read like this: if `->` follows the `)`, it is a function type and the list holds its parameter types (zero or more). Otherwise, one type with no trailing comma is just grouping (`(Int)` is `Int`), and two or more types make a tuple. `()` without `->` and `(Int,)` are errors.
- **Function types** are right-associative and the result is a full type: `(Int) -> Int?` is a function that returns `Int?`. An optional function is written `((Int) -> Int)?`. Function types that can throw are an [open question](README.md#open-questions).
- **`any Trait`** asks for dynamic dispatch. `any Shape?` is `(any Shape)?`.
- **`Never`** is the type of expressions that never finish, such as `exit(1)`. It is an ordinary type name, not a keyword.
- **`type`** is the type of types, used for `comptime` parameters.
- **How `any` and `type` are recognized.** Both are `IDENT`s, so `TypeAtom` could read each of them as a `TypePath` too. The parser decides by the next token, in this order. `any` followed by an `IDENT` is the keyword, and that `IDENT` starts the `TypePath` (`List<any Shape>`). `type` followed by anything except `.` or `<` is the keyword, so `T: type`, `type?` and `List<type>` all use it. In every other case the word is an ordinary `TypePath` name, which a later stage reports because type names start with a capital letter (`List<any>`).
- A trait name used as a parameter type (`fn print_summary(shape: Shape)`) is an ordinary `TypePath`. What it means (a generic shorthand) is decided later.

### 7.2 mut in parameter types

`ParamType = [ "mut" ] Type` (section 3.3) is the only place `mut` can start a type. `let x: mut Int = 1`, `List<mut Int>` and `-> mut Int` are errors: "`mut` is only for function parameters". Lambdas put `mut` before the name instead (section 6.18).

### 7.3 Closing angle brackets

When the parser reads `GenericArgs` or `GenericParams` and needs a closing `>`:

| Current token | What the parser does |
|---|---|
| `>` | consumes it |
| `>>` | consumes one `>`; the current token becomes `>` |
| `>=` | consumes the `>`; the current token becomes `=` |
| `>>=` | consumes one `>`; the current token becomes `>=` |
| anything else | error: "expected `>` to close the generic arguments" |

So `exits: List<Handle<Room>> = []` and `var x: List<Int>= []` both work. In expression position, rule 2 of section 6.11 limits this: the outermost list must not leave anything behind. In type position there is no such limit.

### 7.4 Where types appear

Types are read with this grammar after `:` in `let`, `var`, fields, option fields, parameters and lambda parameters, after `->`, after `throws`, after `is`, in `GenericArgs`, in conformances and requirements, and after `extend`. A type never appears directly in a statement, so the type grammar and the expression grammar only meet in `GenericArgs` inside expressions (section 6.11).

---

## 8. Patterns

### 8.1 Pattern syntax

```
Pattern       = PatternAtom { "or" PatternAtom } ;
PatternAtom   = "_"
              | LOWER_IDENT
              | LiteralPattern
              | RangePattern
              | RefPattern
              | ParenPattern
              | ListPattern ;
LiteralPattern= [ "-" ] ( INT | FLOAT ) | PLAIN_STRING | CHAR | "true" | "false" | "none" ;
RangePattern  = RangeBound ( "..<" | "..=" ) RangeBound ;
RangeBound    = [ "-" ] ( INT | FLOAT ) | CHAR ;
RefPattern    = RefPath [ "(" SubPattern { "," SubPattern } [ "," ] ")" ] ;
RefPath       = UPPER_IDENT | IDENT "." IDENT { "." IDENT } ;
SubPattern    = [ LOWER_IDENT ":" ] Pattern ;
ParenPattern  = "(" Pattern ")"
              | "(" Pattern "," Pattern { "," Pattern } [ "," ] ")" ;
ListPattern   = "[" [ Pattern { "," Pattern } [ "," ] ] "]" ;
```

| Pattern | Example | Matches |
|---|---|---|
| wildcard | `_` | anything, binds nothing |
| binding | `mm`, `title` | anything, and names it |
| literal | `6`, `-1`, `2.5`, `"add"`, `'a'`, `true`, `none` | that exact value |
| range | `1..=5`, `0..<10`, `-10..<0`, `'a'..='z'` | a value in the range |
| enum option, bare | `Sunny`, `Rainy(mm)`, `Rainy(_)`, `Err(NotANumber(text))` | that option, and its fields |
| enum option, qualified | `Weather.Rainy(mm)`, `Ordering.Less` | the same, with the type written out |
| tuple | `(Get, "/hello")`, `(i, task)` | a tuple, element by element |
| list | `["add", title]`, `["list"]`, `[]` | a list of exactly that length |
| alternatives | `6 or 7`, `["list"] or []` | any one of them |

- **Payload fields** are matched by position: `Rainy(mm)` binds the `mm` field to the name `mm`, and `Rect(w, h)` binds `width` to `w`. A field can also be named, which reads like building the value: `Rect(width: w, height: _)`. Named payload patterns are allowed by the grammar for symmetry with construction; this is a choice made in this spec.
- **Only enum options are matched by name.** Decision 0003 says that in patterns, capitalized names are always enum options. Taking a struct apart (`Point(x, y)`) is not in v0.1 ([open question](README.md#open-questions)). The parser reads every capitalized name the same way; if name resolution finds that it names a struct or another type, it reports "only enum options can be matched by name; use a guard such as `p if p.x == 0 => ...`".
- **Literal patterns** use plain strings only (holes are an error). A negative number is a `-` token followed by a number literal; spaces between them don't matter ([lexical.md, 3](lexical.md#3-whitespace-and-line-breaks)).
- **Range bounds** are literals. Both bounds are required.
- **Choosing the alternative.** Most alternatives are decided by the first token: `_`, a `STRING`, `true`, `false`, `none`, `(`, `[`, or an `UPPER_IDENT`. Two need a little more:
  - A number, a `CHAR`, or `-` and a number: read the optional `-` and the literal. If the next token is `..<` or `..=`, it is a `RangePattern`; otherwise it is a `LiteralPattern`. (At most three tokens: `-`, the number, the range operator.)
  - A `LOWER_IDENT`: if the next token is `.`, it starts a dotted `RefPath` (`geometry.Shape.Circle`); otherwise it is a binding.
- **List patterns** match lists of a fixed length. Rest patterns (matching "the first element and the rest") are not in v0.1, because the obvious spelling would need the banned bare `..` ([open question](README.md#open-questions)).
- `ParenPattern` with one pattern is just grouping, for example `Err((A or B))`; it is rarely needed because each payload entry is already a full `Pattern`.

### 8.2 Names in patterns

This is where Quell's naming rule matters to the parser ([lexical.md, 5.4](lexical.md#54-what-the-parser-relies-on)):

- A bare `UPPER_IDENT` (`Sunny`, `Rainy`, `Get`) is always an existing **enum option** (decision 0003).
- A bare `LOWER_IDENT` (`mm`, `title`, `e`) is always a **new binding**. A bare `LOWER_IDENT` followed by `(` is an error: "a new name can't have `( )`; option names start with a capital letter".
- A dotted name (`Weather.Sunny`, `Int.max`) is always a reference.
- `true`, `false` and `none` are keywords, so they are literal patterns, never bindings.

So the parser never needs to know which names are in scope to read a pattern.

### 8.3 or patterns and guards

- `or` between pattern atoms means "any of these". It has the lowest precedence inside a pattern: `Rainy(mm) or Snowy(mm)` is two alternatives. Whether each alternative binds the same names is checked later.
- A **guard** is written after the whole pattern: `Rainy(mm) if mm > 10.0 => ...`. It applies to the whole `or` pattern: in `6 or 7 if late => ...` the guard covers both 6 and 7.
- Once `if` has been read, the parser is in the expression grammar, where `or` is logical or: in `Rainy(mm) if mm > 10.0 or mm < 0.0 => ...` the guard is `mm > 10.0 or mm < 0.0`. A pattern can never contain `if`, so there is no ambiguity about where the pattern ends.
- A line starting with `or` continues the pattern above (continue-before set), so long alternatives can be split over lines.

### 8.4 Where patterns appear

Full patterns appear only in `match` arms. `let`, `var`, `for` and `parallel for` use binding patterns (next section), because they must always match.

### 8.5 Binding patterns

```
BindingPattern = LOWER_IDENT
               | "_"
               | "(" BindingPattern "," BindingPattern { "," BindingPattern } [ "," ] ")" ;
```

- `let x = ...`, `let (low, high) = ...`, `for (i, s) in ...`, `let (sender, receiver) = channel<String>()`, `for _ in 0..<3 { ... }`.
- An uppercase name here gets the naming error ("names of values start with a lowercase letter"), and anything else that is a pattern but not a binding pattern (such as a literal) gets "this pattern might not match; use `match` instead".

---

## 9. Errors the parser reports

### 9.1 Errors

Besides plain "expected X" errors, the parser gives these specific messages, each with a fix. All of them are found without type information.

| Situation | Message (short form) | Section |
|---|---|---|
| `if x = 5 {` (also `while` and guards) | `=` gives a variable a new value. To compare, use `==`. | 4.11 |
| `{` on the line after `if`, `fn`, `for` and so on | the `{` that opens this block must be on the same line | lexical 12.6 |
| two statements on one line | put each statement on its own line | 4.1 |
| `a < b < c`, `a == b == c` | comparisons can't be chained: write `a < b and b < c` | 5 |
| `a..<b..<c`, `x is A is B` | wrap one side in parentheses | 5 |
| `a + return b`, `a + x => x` | wrap this in parentheses | 6.1 |
| assigning to a call, literal or `?.` | you can only assign to a variable, a field or an element | 4.4 |
| `f(x = 1)` | named arguments use `:`: write `f(x: 1)` | 4.4 |
| `var x: Int` with no value | give `x` a starting value | 4.3 |
| a naming-rule violation at a declaration | type names start with a capital letter: write `Player` (and the reverse) | lexical 5.3 |
| a type or enum option named in a script without case (`enum 天气`) | type names must start with an uppercase letter (no automatic fix, because the script has no capital letters) | lexical 5.2 |
| `pub` where it isn't allowed | remove `pub` | 3.2 |
| `mut fn` or `static fn` outside a type | these only make sense inside a type | 3.3 |
| a function with no body outside a trait | add `{ }` or `= value` | 3.3 |
| `mut` in a type outside a parameter | `mut` is only for function parameters | 7.2 |
| `(x: mut T) =>` in a lambda | write `(mut x: T) =>` | 6.18 |
| `spawn x` where `x` isn't a call | `spawn` needs a function call or a block | 6.9 |
| `break 5`, `continue 5` | `break` doesn't take a value | 6.19 |
| `Circle(Float)` in an enum | name this field | 3.6 |
| `Empty()` in an enum | remove the empty parentheses | 3.6 |
| a comma after a method in a struct | put the next member on a new line | 3.5 |
| `()` or `(x,)` | not a value in Quell | 6.12 |
| a hole in a test name, `import c` string or pattern | this string can't contain `{ }` values | 3.9, 3.10, 8.1 |
| an item inside a block | declare this at the top level of the file | 4.2 |
| a top-level statement in a library file | code at the top level only runs in the entry file | 2.2 |
| `String??` | a type can only be optional once | 7.1 |
| `foo(x) =>` in a pattern (a lowercase name with `( )`) | a new name can't have `( )`; option names start with a capital letter | 8.2 |
| `catch err` with its `{` on the next line | put `{` on the same line as `catch err` | 10.3 |
| `count ?? 0 + 1`, `parse(a) catch 0 + parse(b)` | add parentheses to say which you mean: `(count ?? 0) + 1` or `count ?? (0 + 1)` | 5, 6.6 |
| `x ?? return false or y` | write `(x ?? return false) or y`, or `?? return (false or y)` | 6.6 |
| `parse(s) catch e { 0 } + 1` | wrap the `catch` in parentheses: `(parse(s) catch e { 0 }) + 1` | 6.5 |
| `mut` before something that isn't a place: `f(mut a + b)` | only a variable, a field or an element can be passed with `mut` | 6.10 |
| `return { ... }` as a statement | to return the value of this block, write `return ({ ... })` | 6.19 |
| a forgotten comma in a multi-line list or map | did you forget a comma at the end of the line above? | 6.12 |
| `shape is not Circle` | write `not (shape is Circle)` | 6.4 |
| `user is none` | `is` checks a type; to check for a missing value, write `user == none` | 6.4 |
| `not` right after an operator of levels 3 to 13: `a == not b`, `x ?? not y`, `a + not b` | wrap it in parentheses: `a == (not b)` | 5 |
| `cache ??= load()` | Quell has no `??=`: write `cache = cache ?? load()` | 6.6 |
| an assignment as the body of a `match` arm or a lambda: `Add => total += x`, `x => total += x` | an assignment needs braces here: `Add => { total += x }` | 4.4 |
| the rejected syntax of section 11 | see section 11 | 11 |

After each of these errors the parser continues as if the (first) fix had been applied. For example, after `count ?? 0 + 1` it continues as if `(count ?? 0) + 1` had been written, and it wraps an assignment used as an arm or lambda body in a block.

### 9.2 Warnings

These programs are valid, but they very likely don't mean what they seem to. The parser finds each one from tokens alone and gives a warning with a fix. Writing the fix (usually parentheses) silences it.

| Situation | Message (short form) | Section |
|---|---|---|
| a statement starting with `-` or `~` after another statement | this line starts with `-`, so it is a new statement; end the line above with `-`, or write `(-penalty)` | 4.10, lexical 12.6 |
| a line starting with `.` that attaches to the right side of an operator, a prefix operand, a lambda body or a jump's value | this `.upper()` applies only to `user.name`; wrap the expression in parentheses | 10.17, lexical 12.6 |
| a forgotten comma in call arguments, a tuple or parentheses | did you forget a comma at the end of the line above? | 6.12 |
| `-5.abs()` (`-`, a number literal, then `.`) | this is `-(5.abs())`; write `(-5).abs()` to call the method on -5 | 6.9 |
| a range whose right end is a number literal followed by `.`: `0..<10.step(2)` | this is `0..<(10.step(2))`; wrap the range in parentheses: `(0..<10).step(2)` | 6.7 |
| `try` as the whole left side of `catch`: `try load() catch 0` | `try` passes the error up, so this `catch` never runs; remove `try` | 6.5 |
| generic arguments whose `<` and `>` are on different lines in an expression | this reads as generic arguments; to compare, wrap each comparison in parentheses | 6.11 |
| a jump inside brackets whose value starts on the next line | this `return` takes the next line as its value; put them on one line | 6.19 |

---

## 10. Ambiguities and how they are resolved

Every place where the same tokens could mean two things, and the rule that decides. Decision 0004 requires that the parser read every example with no ambiguity; these rules are how.

### 10.1 Generic arguments or less-than

`List<Int>()` versus `a < b`. Resolved by the speculative parse and the three conditions in section 6.11. The only valid program that reads differently than a human might expect is two comparisons around a comma in any comma-separated list (arguments, tuples, lists, maps), such as `f(a < b, c > (d))`, which is a generic call; write `f((a < b), c > (d))` for two comparisons. The same holds when the two comparisons are on separate lines inside `[ ]` or `( )`, where the parser adds a warning (section 6.11).

### 10.2 `>>` closing two generic lists, and `>` at the end of a line

The lexer produces `>>`; the parser splits it when closing generic arguments (section 7.3). A line ending with `>` keeps its `NEWLINE`, and the parser skips that `NEWLINE` only when it uses the `>` as a comparison ([lexical.md, 12.5](lexical.md#125-lines-that-end-with-a-greater-than-sign)). Without this, a field like `items: List<T>` at the end of a line would run into the next field.

### 10.3 The two forms of catch

`e catch fallback` versus `e catch name { block }`. The binding form is chosen exactly when `catch` is followed by a `LOWER_IDENT` or `_` and then `{` (section 6.5). A lowercase name directly followed by `{` is never a valid expression, so the fallback form loses nothing.

As with every block, the `{` must be on the same line as the name. If it is on the next line, `catch err` is read as the fallback form and the block becomes a separate statement. The parser reports this case: when a statement or a top-level item (such as `fn f() -> Int = parse(s) catch err`) ends with `catch` and a lowercase name, and the next statement is a plain block, it says "put `{` on the same line as `catch err`". Inside `( )` and `[ ]` the line break is removed, so the newline rules alone would accept `(f() catch err` with `{ 0 })` on the next line; the parser rejects it with the same message, because a `{` that opens the body of a keyword construct may never have a line break before it ([lexical.md, 12.6](lexical.md#126-worked-examples)).

### 10.4 catch inside a condition

```quell
if load() catch fallback { ... }
```

By the rule in 10.3 this is the binding form (`fallback` names the error), and the `if` then has no block. The parser reports the missing block with a hint: "to use `fallback` as the fallback value here, write `if (load() catch fallback) { ... }`". The same applies after `while`, `for ... in`, `parallel for ... in`, `repeat` and `match`. Uppercase fallbacks and literals (`catch false`, `catch Empty`) are not affected.

The same kind of clash exists for `return`: in `for line in read_lines(path) ?? return {`, the `{` must open the loop's body. This is solved for every position at once, because `return` never takes a value that starts with `{` (section 6.19). And a header is a `HeaderExpr`, so it can't be a lambda that would swallow the body (section 6.1).

### 10.5 Lambda arrows and match arrows

Both use `=>`. In a `match` arm, the part before `=>` is read with the pattern grammar, so `Sunny =>` and `x =>` are patterns, not lambdas. The guard is an `OrExpr`, which can't be a lambda, so in `Some(x) if ready => ...` and `Pair(a, b) if (a, b) => ...` the `=>` always ends the guard. To use a lambda inside a guard, put it in parentheses (it can only appear inside a call anyway).

### 10.6 Lambda parameters, parentheses and tuples

`(a, b) => a + b` versus the tuple `(a, b)` versus `(x)`. Resolved by looking past the matching `)` for `=>` (section 6.1, rule 3). Typed parameters such as `(text: String) =>` could not be anything else, but the same rule covers them.

### 10.7 mut arguments and mut lambdas

In `heal(mut hero)`, `mut` marks a changeable argument. In `log.lock(mut entries => entries.append(x))`, `mut` belongs to the lambda's parameter. Resolved by lookahead, after the optional argument name: `mut`, a name, then `=>` is a lambda (section 6.1, rule 2); anything else after `mut` is a changeable argument, which must be a place (section 6.10). The `Arg` rule lists the three alternatives in the order they are tried, so it has exactly one parse.

### 10.8 or in patterns and in guards

Before `if`, `or` separates pattern alternatives. After `if`, the parser is reading an expression, where `or` is logical or. The guard covers the whole `or` pattern (section 8.3).

### 10.9 Capitalized names

In patterns, an uppercase bare name is an enum option and a lowercase bare name is a new binding (section 8.2). In expressions, capitalization means nothing to the parser: `Player(...)`, `Sunny` and `print(...)` all parse the same way, and name resolution sorts them out. After `catch`, only a lowercase name can be the error binding (section 6.5).

### 10.10 Colons inside interpolation

In `"{price:.2}"` the colon starts a format spec; in `"{greet(name: "Ava")}"` it is a named argument. Resolved by the lexer: only a `:` at depth 0 in the hole starts a format spec, counting `( )`, `[ ]` and `{ }` ([lexical.md, 9.3](lexical.md#93-interpolation)).

### 10.11 `{` always opens a block in code

At the start of a statement, and in any expression position, `{` opens a block. Quell has no map literals with braces, no struct literals with braces and no trailing lambdas, so `{` never continues an expression. This is what makes `if x {`, `for x in items {`, `while n > 0 {`, `match x {` and `repeat n {` unambiguous with no special "no struct literal here" rule (the problem Rust has).

Braces that are not a block appear only where a keyword has already said what follows: the bodies of `struct`, `enum`, `trait` and `extend`, the arm list of `match`, and the name list of `import`. None of these is in expression position. A hole in a string is marked by `{` too, but holes are found by the lexer inside a `STRING` token, so the parser never sees that brace.

The one expression-level case that needs care is a construct that takes a block right after an expression: the conditions of `if` and `while`, the items of `for` and `parallel for`, the count of `repeat` (when it is not `{` itself, section 4.7) and the scrutinee of `match`. Each of these is a `HeaderExpr` (section 6.1). The rule is simple because no operator starts with `{`:

- A `{` that comes where an operator could come (after a complete operand) ends the expression and opens the body. In `if ready {`, the `{` follows the operand `ready`. A `return` with no value is complete too, because `return` never takes a value that starts with `{` (section 6.19), so in `for line in lines ?? return {` the `{` opens the body.
- A `{` that comes where an operand is expected starts a block expression, or the block of `group`, `unsafe`, `spawn` or a `catch` binding. So `if unsafe { flag } { ... }` has the condition `unsafe { flag }`. Such conditions are legal but hard to read; the formatter wraps them in parentheses.

This is also why `group`, `unsafe`, `spawn` and `defer` are reserved words ([lexical.md, 6.3](lexical.md#63-why-the-others-are-reserved)): if `group` were a name, `if group {` would have two readings.

### 10.12 A top-level let: item or statement

`let` at the top level is always parsed as a `GlobalLet` item (section 2.1). In an entry file it still runs in order. This gives one parse for both kinds of file.

### 10.13 return with or without a value

`return` takes a value only when the next token can start an expression and is not `{` (section 6.19). Since `return` is not in the continue-after set, a `return` at the end of a line returns nothing, outside `( )` and `[ ]`. Inside them line breaks are removed, so the value may be on the next line (with a warning).

### 10.14 repeat count or body

After `repeat`, `{` always starts the body (section 4.7). A count that is a block expression would need parentheses: `repeat ({ n }) { ... }`.

### 10.15 spawn with a block or a call

After `spawn`, `{` always starts a block. Otherwise the operand is a postfix expression that must end in a call (section 6.9).

### 10.16 A line that starts with `-`, `(` or `[`

These always start a new statement, because each of them can start an expression (`-1`, a tuple, a list). Only the tokens in the continue-before set join a line to the one above ([lexical.md, 12.4](lexical.md#124-the-continue-before-set)). This is also why a `match` arm can start with a negative number.

### 10.17 A line that starts with `.`

It always continues the line above. Decision 0004 removed the `.Sunny` shorthand for exactly this reason, so an enum option is written `Sunny` or `Weather.Sunny`.

The `.` then binds like any postfix operator, to the operand right before it. After a binary operator, that operand is only the operator's right side: `user.nickname ?? user.name` followed by `.upper()` on the next line means `user.nickname ?? (user.name.upper())`. The parser gives a warning in that case ([lexical.md, 12.6](lexical.md#126-worked-examples)); a plain chain never gets it.

### 10.18 Lists and maps

`[a, b]` versus `[a: b]`: decided by the token after the first element. `[]` is an empty list and `[:]` an empty map (section 6.12).

### 10.19 Numbers, members and ranges

`4.is_even()`, `1.52`, `0..<3`: decided by the lexer's dot rule ([lexical.md, 7.3](lexical.md#73-the-dot-rule)).

### 10.20 Contextual keywords

`any`, `type`, `parallel`, `test` and `c` are keywords only in one position each, where an identifier could not appear ([lexical.md, 6.2](lexical.md#62-contextual-keywords)). In particular, `parallel for` needs `for` directly after `parallel`, and `test` needs a string literal directly after it at the start of an item.

### 10.21 Parenthesized types

`(Int)` versus `(Int, Int)` versus `(Int) -> Int`: decided by whether `->` follows and by the number of elements (section 7.1).

### 10.22 throws with or without an error type

`throws {`, `throws =` and `throws` at the end of a requirement have no error type, because a type can't start with `{`, `=` or a `NEWLINE`. Anything else after `throws` is read as a type.

### 10.23 The `?` suffix and the ternary operator

`?` appears only at the end of a type. In an expression, a `?` token can only be an attempted ternary or a Rust-style `?`, and both are rejected syntax (section 11).

### 10.24 A line that starts with `else`, `catch`, `??`, `and` or `or`

It always continues the line above ([lexical.md, 12.4](lexical.md#124-the-continue-before-set)). None of these words can start a statement, an item, a member or a match arm, so joining the lines never changes the meaning of code that was already valid. It only makes more layouts valid, such as `}` and `else {` on separate lines.

### 10.25 The three uses of comptime

`comptime fn`, `comptime let` (or `var`) and `comptime { }` are told apart by the token right after `comptime` (section 2.1).

### 10.26 Assignment or expression statement

Both start with an expression. The parser reads the expression first and then looks at the next token: an assignment operator makes it an `AssignStmt`, and the left side is then checked to be a place (section 4.4). This needs no backtracking.

### 10.27 Fields, methods and options in a body

In a `struct` body, a member that starts with an identifier is a field, and one that starts with `fn`, `mut`, `static` or `comptime` is a method (after any attributes and `pub`). In an `enum` body, an identifier starts an option. A `mut` here is always a method modifier, because fields and options never start with `mut`.

---

## 11. Rejected syntax

The parser and lexer recognize these forms only to explain the Quell way. Each one is an error with a fix, and parsing continues as if the fix had been applied.

| Written | Reported by | Message and fix |
|---|---|---|
| `a && b` | lexer | Quell uses the word `and`: `a and b` |
| <code>a &#124;&#124; b</code> | lexer | Quell uses the word `or`: `a or b` |
| `!done` | lexer | Quell uses the word `not`: `not done` |
| `0..10` | lexer | a range must say whether it includes the end: `0..<10` or `0..=10` |
| `x = 1;` | lexer | Quell doesn't use semicolons; put each statement on its own line |
| `if let x = maybe { ... }` | parser | check it with a normal `if`: `if maybe != none { ... }`; after the check, Quell knows the value is there |
| `let x = maybe else { return }` | parser | use `??`: `let x = maybe ?? return` |
| `.Sunny` (an expression that starts with `.`) | parser | enum options are written by name: `Sunny`, or `Weather.Sunny` |
| `ok ? a : b` | parser | Quell has no `? :`; `if` gives a value: `if ok { a } else { b }` |
| `value?` (a Rust-style `?` after an expression) | parser | use `try value` to pass the error up, or `value ?? fallback` for a missing value |
| <code>6 &#124; 7 =&gt; ...</code> (a bar between patterns) | parser | patterns are joined with `or`: `6 or 7 => ...` |
| `Point { x: 1.0, y: 2.0 }` (a struct literal with braces) | parser | values are created with parentheses: `Point(x: 1.0, y: 2.0)` |
| `Weather::Rainy` | parser | paths use `.`: `Weather.Rainy` |
| `count++`, `count--` | parser | write `count += 1` or `count -= 1` |

Notes:

- **`.Sunny`.** In argument position (`paint(.Red)`), the parser sees `.` where an expression must start and gives the message directly. At the start of a line (for example a `match` arm written `.Cloudy => ...`), the newline rules have already joined the line to the one above, so the parser sees `"Maybe a jacket.".Cloudy => ...` and fails at `=>`. When a parse error happens on a line that started with `.` followed by a capitalized name, the parser adds the same message as a note.
- **`if let` and `let ... else`.** Decision 0004 removed them. After `if`, the keywords `let` and `var` trigger the message. After the value of a `let`, the keyword `else` (which may be on the same line or start the next one) triggers it.
- **The ternary.** In an expression, after a complete operand, a `?` token (not `?.` or `??`) triggers the message. If a `:` follows later on the line, the fix rewrites the whole thing as an `if`.
- **The last four rows** are habits from other languages. They are made of valid tokens, so the parser detects them by shape: a `|` token where a pattern could continue; a struct literal (see the next note); two `:` tokens with nothing between them; and two `+` or two `-` tokens with nothing between them, right after an operand and right before a line break or a closing bracket. The last check matters because `-` is in the continue-after set: without it, `count--` followed by a new line would silently join the two lines.
- **Struct literals with braces.** The check looks for four tokens **all on one line**: an `UPPER_IDENT`, `{`, an identifier and `:`. It applies only while the parser is reading an expression, when the `UPPER_IDENT` has just completed an operand and the `{` comes where an operator could come. It never applies in item headers (`struct Point {`, `struct Circle: Shape {`, `enum`, `trait`, `extend`), so `struct Point { x: Float, y: Float }` is never mistaken for a struct literal. It also never applies in patterns, or in the header of `if`, `while`, `for`, `parallel for`, `match` and `repeat`, where that `{` opens the body (section 6.1). In those headers, `for p in Points { x: 1 }` gets the ordinary error for the body instead. After `catch` (`load() catch Empty { x: 1 }`) the check does apply.

---

## 12. Example coverage

Every program in [examples/](../examples/README.md) must be derivable from this grammar, with zero errors ([decision 0005](../docs/decisions/0005-front-end-architecture.md)). Comment lines that show code which "is a compile error" are comments and are not part of the program. This table lists, for each file, the constructs it uses beyond the basics (calls, names, literals, and `let` and `var`) and the sections that cover them. A `let` or `var` at the top level of a file is a `GlobalLet` item (section 3.12); inside a block it is a `LetStmt` (section 4.3).

| File | Constructs | Sections |
|---|---|---|
| 01_hello.ql | a single top-level call | 2.1, 6.10 |
| 02_variables.ql | top-level `let` and `var` (items), `=`, `+=`, type annotation, holes | 3.12, 4.4, 6.13 |
| 03_numbers.ql | `/`, `div`, `%`, `1_000_000`, format spec `.2` | 6.8, lexical 7.1, lexical 9.4 |
| 04_conditions.ql | `if`, `else if`, `else`, `and`, `not`, `or`, `if` as a value | 6.2, 6.15 |
| 05_loops.ql | `for`, `..<`, `..=`, `repeat n`, `repeat`, `while`, `-=`, `break`, `continue` in one-line blocks | 4.5 to 4.7, 6.7, 6.19 |
| 06_functions.ql | `= expr` bodies (including `greet`, whose body is one call), block bodies, `return`, defaults, named arguments, tuple return type, tuple value, top-level tuple `let` | 3.3, 3.12, 6.10, 6.12, 7.1, 8.5 |
| 07_lists.ql | lambdas `s => s + 5`, `for (i, s)`, indexing, `?? 0` | 6.6, 6.18, 8.5 |
| 08_maps.ql | map literal, `[:]`, top-level `var` with `Map<String, Int>`, index assignment, `!= none` | 3.12, 4.4, 6.12, 7.1 |
| 09_strings.ql | `{{`, `"""` with trimming, raw `#"..."#`, `'A'`, holes with calls | lexical 8, lexical 9 |
| 10_guess_the_number.ql | `import std.random`, `repeat { }`, `match` with a block arm, `break` and `continue` inside `if` | 3.9, 6.16 |
| 11_structs.ql | struct with defaults and methods, `mut fn`, `static fn`, `self` | 3.3, 3.5 |
| 12_enums_and_match.ql | enum with and without fields, `match` as an `= expr` body, guard, `Rainy(_)`, range pattern, `6 or 7`, `_` | 3.6, 6.16, 8 |
| 13_optionals.ql | `String?`, multi-line list with trailing comma, `?.`, `?? return none` | 6.6, 6.10, 7.1, lexical 12.6 |
| 14_errors.ql | `enum AgeError: Error`, `throws AgeError`, `throw`, `?? throw`, `try` inside a call, `catch 0`, `catch err { }`, `catch []`, nested patterns `Err(NotANumber(text))` | 3.3, 3.6, 6.5, 6.9, 8.1 |
| 15_value_semantics.ql | `counter: mut Counter`, `bump(mut c)`, struct with a default, `= expr` body with no return type | 3.3, 6.10, 7.2 |
| 16_todo_app.ql | list patterns `["add", title]`, `["list"] or []`, `?? return none` inside a call argument in an arm, `throws` with no type, `match` on a top-level value, `try` statement | 3.3, 6.6, 8.1 |
| 17_traits.ql | trait with requirements and a default method, `struct Circle: Shape`, `List<any Shape>`, `is`, `extend Int`, format spec after a call `{self.area():.1}` | 3.7, 3.8, 6.4, 7.1, lexical 9.3 |
| 18_generics.ql | `fn largest<T: Compare>`, `<T: Compare and Show>`, `struct Stack<T>`, `List<Int>()`, `Stack<String>()`, `?? -1`, `mut fn push(item: T) = ...`, `mut fn pop() -> T? = ...` | 3.4, 6.11, 7.1 |
| 19_modules/main.ql | `import geometry.point.{Point, distance}` | 3.9 |
| 19_modules/geometry/point.ql | library file: `pub struct`, `pub` fields, `pub fn`, doc comments | 2.2, 3.1, 3.2 |
| 19_modules/geometry/shapes.ql | library file: `pub fn` method with a doc comment | 2.2, 3.5 |
| 20_testing.ql | `test "name" { }`, `assert` | 3.10, 4.9 |
| 21_library/src/lib.ql | library file: `pub static fn`, `?? return none`, `slice(0..<2)`, format spec `02x`, `pub let`, multi-line call with trailing comma, `div`, `test` in a library, code examples in doc comments (each parsed as an entry file) | 2.2, 3.12, 6.10, lexical 4.3, lexical 9.4 |
| 22_lambdas_and_chaining.ql | multi-line method chain, typed lambda, lambda with block body, function type `(Int) -> Int` | 6.18, 7.1, lexical 12.6 |
| 23_parallel.ql | `parallel for` as a value, `group { }`, `spawn call`, `spawn { }`, `channel<String>()`, top-level tuple `let` | 3.12, 6.9, 6.11, 6.17 |
| 24_sharing.ql | `spawn { }` inside `repeat` and `for`, `mut entries => ...`, `List<Handle<Room>>` (split `>>`), `Arena<Room>()`, index then member then call | 6.11, 6.18, 7.3 |
| 25_web_server.ql | `import http.{...}`, `@derive(Json)`, `match` on a tuple, tuple patterns `(Get, "/")`, block arm | 3.1, 6.16, 8.1 |
| 26_files_and_cleanup.ql | `throws` with no type, `-> Int throws`, `defer { }`, `try` statements | 3.3, 4.8 |
| 27_comptime.ql | `comptime let`, `comptime { }`, `assert` with a message, `comptime fn` with `T: type`, `(0..<16).map(...)` | 3.11, 3.12, 4.9, 7.1 |
| 28_c_interop/main.ql | `import c "sqlite3.h"`, `= unsafe { }` | 3.9, 6.17 |
| 29_embedded_blink/main.ql | `@entry` on its own line, `-> Never` | 3.1, 7.1 |
