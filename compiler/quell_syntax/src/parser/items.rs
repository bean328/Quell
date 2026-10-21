//! Files and items (spec/grammar.md, sections 2 and 3).

use quell_diagnostics::{Diagnostic, Edit, Span};

use super::exprs;
use super::fixes::{Case, field_name_for_type, indentation_at};
use super::habits;
use super::stmts;
use super::types::{self, MutRule};
use super::{Bracket, Ctx, Parser, codes};
use crate::SyntaxKind::{self, *};

/// `File = [ TopElement { NEWLINE TopElement } ] EOF` (2.1).
pub(crate) fn file(p: &mut Parser<'_>) {
    let marker = p.start();
    stmts::statement_list(p, None);
    p.bump_eof();
    p.complete(marker, FILE);
}

/// Where a declaration is: it decides which modifiers and members are allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Container {
    /// The top level of a file.
    File,
    /// Inside a block, where items are an error (4.2).
    Block,
    Struct,
    Enum,
    Trait,
    Extend,
}

impl Container {
    fn is_type(self) -> bool {
        matches!(self, Container::Struct | Container::Enum | Container::Trait | Container::Extend)
    }
}

/// Whether the current token starts an item. At the top level, `let`, `var` and `mut` do
/// (2.1); inside a block, `let` and `var` start statements and `mut` starts an item only
/// before `fn`.
pub(crate) fn at_item_start(p: &Parser<'_>, top: bool) -> bool {
    match p.current() {
        AT | PUB_KW | FN_KW | STATIC_KW | COMPTIME_KW | STRUCT_KW | ENUM_KW | TRAIT_KW
        | EXTEND_KW | IMPORT_KW => true,
        MUT_KW => top || p.nth(1) == FN_KW,
        LET_KW | VAR_KW => top,
        LOWER_IDENT if p.nth_text(0) == "test" && p.nth(1) == STRING_START => true,
        LOWER_IDENT => match habits::foreign_keyword(p) {
            Some(LET_KW) => top,
            Some(_) => true,
            None => habits::at_from_import(p),
        },
        _ => false,
    }
}

/// The `pub` and the function modifier before an item or member, in the order written.
#[derive(Clone, Copy, Debug, Default)]
struct Modifiers {
    public: Option<Span>,
    modifier: Option<(SyntaxKind, Span)>,
}

/// `Item = Attributes [ "pub" ] ItemKind` (3.1). Returns the kind of item.
pub(crate) fn item(p: &mut Parser<'_>, container: Container) -> SyntaxKind {
    let marker = p.start();
    let first = p.nth_span(0);
    attributes(p);
    let mods = modifiers(p);
    let keyword = p.nth_span(0);
    let foreign = habits::foreign_keyword(p);
    let kind = match p.current() {
        LOWER_IDENT if habits::at_from_import(p) && foreign.is_none() => {
            no_modifier(p, mods);
            not_public(p, mods, "`import`");
            habits::from_import(p);
            IMPORT_DECL
        }
        FN_KW | LOWER_IDENT if p.at(FN_KW) || foreign == Some(FN_KW) => {
            if let Some((kind @ (MUT_KW | STATIC_KW), span)) = mods.modifier {
                modifier_outside_type(p, kind, span);
            }
            fn_rest(p, container);
            FN_DECL
        }
        LET_KW | VAR_KW => {
            global_let(p, mods);
            GLOBAL_LET
        }
        LOWER_IDENT if foreign == Some(LET_KW) => {
            global_let(p, mods);
            GLOBAL_LET
        }
        L_BRACE if matches!(mods.modifier, Some((COMPTIME_KW, _))) => {
            not_public(p, mods, "a `comptime` block");
            stmts::body_block(p, "`comptime`", None);
            COMPTIME_BLOCK
        }
        STRUCT_KW => {
            no_modifier(p, mods);
            struct_decl(p);
            STRUCT_DECL
        }
        LOWER_IDENT if foreign == Some(STRUCT_KW) => {
            no_modifier(p, mods);
            struct_decl(p);
            STRUCT_DECL
        }
        ENUM_KW => {
            no_modifier(p, mods);
            enum_decl(p);
            ENUM_DECL
        }
        TRAIT_KW | LOWER_IDENT if p.at(TRAIT_KW) || foreign == Some(TRAIT_KW) => {
            no_modifier(p, mods);
            trait_decl(p);
            TRAIT_DECL
        }
        EXTEND_KW | LOWER_IDENT if p.at(EXTEND_KW) || foreign == Some(EXTEND_KW) => {
            no_modifier(p, mods);
            not_public(p, mods, "`extend`");
            extend_decl(p);
            EXTEND_DECL
        }
        IMPORT_KW => {
            no_modifier(p, mods);
            not_public(p, mods, "`import`");
            import_decl(p);
            IMPORT_DECL
        }
        LOWER_IDENT if p.nth_text(0) == "test" && p.nth(1) == STRING_START => {
            no_modifier(p, mods);
            not_public(p, mods, "a test");
            test_decl(p);
            TEST_DECL
        }
        _ => {
            let what = match mods.modifier {
                Some(_) => "`fn`",
                None => "an item, such as `fn` or `struct`",
            };
            p.error_expected(
                codes::EXPECTED_TOKEN,
                what,
                "an attribute or `pub` must be followed by the item it belongs to",
            );
            if p.consumed_since(&marker) {
                p.complete(marker, ERROR);
            } else {
                p.abandon(marker);
            }
            return ERROR;
        }
    };
    p.complete(marker, kind);
    if container == Container::Block {
        item_in_block(p, first.to(keyword), kind);
    }
    kind
}

/// `Attributes = { Attribute [ NEWLINE ] }` and
/// `Attribute = "@" IDENT { "." IDENT } [ "(" [ ArgList ] ")" ]` (3.1).
fn attributes(p: &mut Parser<'_>) {
    while p.at(AT) {
        let marker = p.start();
        p.bump();
        if p.current().is_ident() {
            p.bump();
            while p.at(DOT) && p.nth(1).is_ident() {
                p.bump();
                p.bump();
            }
        } else {
            p.error_expected(
                codes::EXPECTED_NAME,
                "the name of an attribute",
                "write a name after `@`, as in `@derive(Json)`",
            );
        }
        if p.at(L_PAREN) && !p.nth_line_break_before(0) {
            exprs::arg_list(p);
        }
        p.complete(marker, ATTRIBUTE);
        if p.at(NEWLINE) {
            p.bump();
        }
    }
}

/// Reads `pub` and a function modifier (`mut`, `static`, `comptime`), reporting the wrong
/// order and a second modifier (3.3).
fn modifiers(p: &mut Parser<'_>) -> Modifiers {
    let mut mods = Modifiers::default();
    loop {
        let span = p.nth_span(0);
        match p.current() {
            PUB_KW => {
                if mods.public.is_some() {
                    p.push_error(
                        Diagnostic::error("`pub` is written twice")
                            .with_code(codes::MODIFIER_ORDER)
                            .primary(span, "the second `pub`")
                            .suggest("remove it", with_space_after(p, span), ""),
                    );
                } else if let Some((kind, modifier)) = mods.modifier {
                    let word = kind.fixed_text().unwrap_or("");
                    p.push_error(
                        Diagnostic::error(format!("`pub` comes before `{word}`"))
                            .with_code(codes::MODIFIER_ORDER)
                            .primary(span, format!("this `pub` comes after `{word}`"))
                            .suggest_edits(
                                format!("write `pub {word}`"),
                                vec![
                                    Edit {
                                        span: Span::at(modifier.start),
                                        replacement: "pub ".into(),
                                    },
                                    Edit {
                                        span: with_space_after(p, span),
                                        replacement: String::new(),
                                    },
                                ],
                            ),
                    );
                }
                mods.public = Some(span);
            }
            MUT_KW | STATIC_KW | COMPTIME_KW => {
                let kind = p.current();
                if let Some((first, _)) = mods.modifier {
                    let a = first.fixed_text().unwrap_or("");
                    let b = kind.fixed_text().unwrap_or("");
                    p.push_error(
                        Diagnostic::error(
                            "a function can have only one of `mut`, `static` and `comptime`",
                        )
                        .with_code(codes::MODIFIER_ORDER)
                        .primary(span, format!("`{b}` after `{a}`"))
                        .suggest(
                            format!("remove `{b}`"),
                            with_space_after(p, span),
                            "",
                        ),
                    );
                } else {
                    mods.modifier = Some((kind, span));
                }
            }
            _ => break,
        }
        p.bump();
    }
    mods
}

/// `span` and the spaces after it, for fixes that remove a word.
pub(crate) fn with_space_after(p: &Parser<'_>, span: Span) -> Span {
    let rest = &p.text()[span.end as usize..];
    let spaces = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    Span::new(span.start, span.end + spaces as u32)
}

#[cold]
fn modifier_outside_type(p: &mut Parser<'_>, kind: SyntaxKind, span: Span) {
    let word = kind.fixed_text().unwrap_or("");
    let meaning = if kind == MUT_KW {
        "`mut fn` is a method that may change `self`"
    } else {
        "`static fn` belongs to a type, not to one value"
    };
    p.push_error(
        Diagnostic::error(format!("`{word} fn` only makes sense inside a type"))
            .with_code(codes::MODIFIER_OUTSIDE_TYPE)
            .primary(span, format!("`{word}` outside a `struct`, `enum`, `trait` or `extend`"))
            .note(meaning)
            .suggest(format!("remove `{word}`"), with_space_after(p, span), ""),
    );
}

/// Reports a function modifier before something that is not a function.
fn no_modifier(p: &mut Parser<'_>, mods: Modifiers) {
    if let Some((kind, span)) = mods.modifier {
        let word = kind.fixed_text().unwrap_or("");
        p.push_error(
            Diagnostic::error(format!("`{word}` only goes before `fn`"))
                .with_code(codes::UNEXPECTED_TOKEN)
                .primary(span, format!("`{word}` before something that is not a function"))
                .suggest(format!("remove `{word}`"), with_space_after(p, span), ""),
        );
    }
}

/// Reports `pub` where it is not allowed (3.2).
fn not_public(p: &mut Parser<'_>, mods: Modifiers, what: &str) {
    if let Some(span) = mods.public {
        pub_not_allowed(p, span, &format!("`pub` can't be used on {what}"), None);
    }
}

#[cold]
fn pub_not_allowed(p: &mut Parser<'_>, span: Span, message: &str, note: Option<&str>) {
    let mut diagnostic = Diagnostic::error(message.to_string())
        .with_code(codes::PUB_NOT_ALLOWED)
        .primary(span, "`pub` is not allowed here");
    if let Some(note) = note {
        diagnostic = diagnostic.note(note.to_string());
    }
    p.push_error(diagnostic.suggest("remove `pub`", with_space_after(p, span), ""));
}

#[cold]
fn item_in_block(p: &mut Parser<'_>, span: Span, kind: SyntaxKind) {
    let what = match kind {
        FN_DECL => "a function",
        STRUCT_DECL => "a struct",
        ENUM_DECL => "an enum",
        TRAIT_DECL => "a trait",
        EXTEND_DECL => "an `extend` block",
        IMPORT_DECL => "an import",
        TEST_DECL => "a test",
        COMPTIME_BLOCK => "a `comptime` block",
        GLOBAL_LET => "a `comptime let`",
        _ => "this",
    };
    p.push_error(
        Diagnostic::error(format!("{what} can't be declared inside a block"))
            .with_code(codes::ITEM_IN_BLOCK)
            .primary(span, "declared inside a block")
            .note("functions, types, imports, tests and `comptime` code live at the top level of a file")
            .help("declare this at the top level of the file, outside every function"),
    );
}

// ----------------------------------------------------------------------
// Functions (3.3)
// ----------------------------------------------------------------------

/// The rest of `FnDecl`, from `fn`.
pub(crate) fn fn_rest(p: &mut Parser<'_>, container: Container) {
    keyword(p, FN_KW);
    let what = if container.is_type() { "method names" } else { "function names" };
    declared_name(p, Case::Lower, what, "a function name");
    if p.at(LT) {
        types::generic_param_list(p);
    }
    if p.at(L_PAREN) {
        param_list(p);
    } else {
        p.error_missing_token("()", "`(` and the parameters");
    }
    if p.at(THIN_ARROW) {
        let marker = p.start();
        p.bump();
        types::type_(p, MutRule::Forbidden);
        p.complete(marker, RET_TYPE);
    }
    if p.at(THROWS_KW) {
        let marker = p.start();
        p.bump();
        // A type can't start with `{`, `=` or a line break (10.22).
        if p.current().is_ident() || matches!(p.current(), L_PAREN | MUT_KW) {
            types::type_(p, MutRule::Forbidden);
        }
        p.complete(marker, THROWS_CLAUSE);
    }
    fn_body(p, container);
}

/// The keyword that starts a declaration, or a word from another language that stands for it,
/// which is reported and read as the keyword (see `habits.rs`).
fn keyword(p: &mut Parser<'_>, kind: SyntaxKind) {
    if p.at(kind) {
        p.bump();
    } else {
        habits::foreign_word(p, kind);
    }
}

/// `FnBody = "=" Expr | Block`, required except in a trait.
fn fn_body(p: &mut Parser<'_>, container: Container) {
    if p.eat(EQ) {
        exprs::expr(p, Ctx::default());
        return;
    }
    if p.at_body_brace("the function's signature") {
        stmts::block(p);
        return;
    }
    if habits::colon_block(p, "the function's signature").is_some() {
        return;
    }
    if container == Container::Trait && !p.at(COLON) {
        return;
    }
    if p.speculating() {
        return;
    }
    if p.at(COLON) {
        habits::colon_instead_of_brace(p);
        return;
    }
    let at = Span::at(p.last_end());
    p.push_error(
        Diagnostic::error("this function needs a body")
            .with_code(codes::FUNCTION_WITHOUT_BODY)
            .primary(at, "the body goes here")
            .note("only a function in a `trait` can be written without a body")
            .suggest("add `{ }` or `= value`", at, " { }"),
    );
}

/// A declared name at the current token, checked against the naming rule. Returns its span.
fn declared_name(p: &mut Parser<'_>, case: Case, what: &str, description: &str) -> Option<Span> {
    let span = p.nth_span(0);
    if p.current().is_ident() {
        p.check_name(span, case, what);
        p.bump();
        return Some(span);
    }
    if p.current().is_keyword() && !p.nth_line_break_before(0) {
        let word = p.nth_text(0).to_string();
        if !p.speculating() {
            p.push_error(
                Diagnostic::error(format!("`{word}` is a reserved word, so it can't be a name"))
                    .with_code(codes::EXPECTED_NAME)
                    .primary(span, format!("expected {description}"))
                    .suggest(
                        format!("choose another name, such as `{word}_`"),
                        span,
                        format!("{word}_"),
                    ),
            );
        }
        p.bump();
        return Some(span);
    }
    p.error_expected(codes::EXPECTED_NAME, description, "write a name here");
    None
}

/// `"(" [ ParamList ] ")"` with `Param = IDENT [ ":" ParamType ] [ "=" Expr ]`.
fn param_list(p: &mut Parser<'_>) {
    let marker = p.start();
    let open = p.pos();
    p.bump();
    let saved = p.bracket;
    p.bracket = Bracket::Other;
    exprs::delimited(p, open, "a parameter", param);
    p.bracket = saved;
    p.close_bracket(open, "the parameter list");
    p.complete(marker, PARAM_LIST);
}

fn param(p: &mut Parser<'_>) -> bool {
    if p.at(SELF_KW) {
        let before = p.last_token_span();
        let before_kind = p.last_kind();
        habits::self_parameter(p, before, before_kind);
        return true;
    }
    if !(p.current().is_ident() || p.current().is_keyword()) {
        p.error_expected(
            codes::EXPECTED_NAME,
            "a parameter name",
            "write the parameter's name, as in `amount: Int`",
        );
        return false;
    }
    if at_type_before_name(p, &[COMMA, R_PAREN, EQ]) {
        let marker = p.start();
        type_before_name(p);
        p.complete(marker, PARAM);
        return true;
    }
    let marker = p.start();
    let name = p.nth_span(0);
    let is_keyword = p.current().is_keyword();
    if is_keyword {
        declared_name(p, Case::Lower, "parameter names", "a parameter name");
    } else {
        p.bump();
    }
    let mut holds_type = false;
    if p.eat(COLON) || type_without_colon(p) {
        let ty = types::type_(p, MutRule::Allowed);
        holds_type = ty.is_some_and(|ty| ty.kind == TYPE_TYPE);
    }
    if p.eat(EQ) {
        exprs::expr(p, Ctx::default());
    }
    p.complete(marker, PARAM);
    if !is_keyword {
        if holds_type {
            p.check_name(name, Case::Upper, "parameters that hold a type");
        } else {
            p.check_name(name, Case::Lower, "parameter names");
        }
    }
    true
}

// ----------------------------------------------------------------------
// Structs, enums, traits and extend (3.5 to 3.8)
// ----------------------------------------------------------------------

fn struct_decl(p: &mut Parser<'_>) {
    keyword(p, STRUCT_KW);
    declared_name(p, Case::Upper, "struct names", "the struct's name");
    if p.at(LT) {
        types::generic_param_list(p);
    }
    conformance(p);
    member_list(p, Container::Struct, "the struct's name");
}

fn enum_decl(p: &mut Parser<'_>) {
    p.bump();
    declared_name(p, Case::Upper, "enum names", "the enum's name");
    if p.at(LT) {
        types::generic_param_list(p);
    }
    conformance(p);
    member_list(p, Container::Enum, "the enum's name");
}

fn trait_decl(p: &mut Parser<'_>) {
    keyword(p, TRAIT_KW);
    declared_name(p, Case::Upper, "trait names", "the trait's name");
    if p.at(LT) {
        types::generic_param_list(p);
    }
    member_list(p, Container::Trait, "the trait's name");
}

fn extend_decl(p: &mut Parser<'_>) {
    keyword(p, EXTEND_KW);
    if p.current().is_ident() {
        types::type_path(p);
    } else {
        p.error_expected(
            codes::EXPECTED_TYPE,
            "the type to extend",
            "write the type's name, as in `extend Int { ... }`",
        );
    }
    conformance(p);
    member_list(p, Container::Extend, "the type's name");
}

/// `Conformance = ":" TypePath { "and" TypePath }`.
fn conformance(p: &mut Parser<'_>) {
    // A `:` at the end of the line starts a block written the Python way (`habits.rs`).
    if !p.at(COLON) || matches!(p.nth(1), NEWLINE | EOF) {
        return;
    }
    let marker = p.start();
    p.bump();
    loop {
        if p.current().is_ident() {
            types::type_path(p);
        } else {
            p.error_expected(
                codes::EXPECTED_TYPE,
                "a trait name",
                "write the trait's name, as in `struct Circle: Shape`",
            );
            break;
        }
        if !p.eat(AND_KW) {
            break;
        }
    }
    p.complete(marker, CONFORMANCE);
}

/// The `{ }` body of a struct, enum, trait or extend, with members separated by line breaks
/// (and commas after fields and options).
fn member_list(p: &mut Parser<'_>, container: Container, after: &str) {
    let colon = !p.at_body_brace(after) && habits::colon_members(p, after);
    if !colon && !p.at(L_BRACE) {
        if !p.speculating() {
            if let Some(diagnostic) = p.expected(codes::EXPECTED_TOKEN, "`{`") {
                let at = Span::at(p.last_end());
                p.push_error(diagnostic.suggest("add the body", at, " {}"));
            }
        }
        return;
    }
    let marker = p.start();
    let open = p.pos();
    p.bump();
    let saved = p.bracket;
    p.bracket = Bracket::Block;
    let close = p.list_end(open);
    loop {
        while p.at(NEWLINE) && !stmts::at_list_close(p, close) {
            p.bump();
        }
        if stmts::at_list_close(p, close) {
            break;
        }
        let start = p.pos();
        let kind = member(p, container);
        if p.at(COMMA) {
            let comma = p.nth_span(0);
            if kind == FN_DECL {
                comma_after_method(p, comma);
            }
            p.bump();
            continue;
        }
        if p.at(NEWLINE) || stmts::at_list_close(p, close) || p.at_soft_break() {
            continue;
        }
        if p.pos() == start {
            p.skip_to_safe_point();
            if p.pos() == start {
                p.bump_error();
            }
            continue;
        }
        if at_member_start(p, container) {
            missing_member_separator(p, kind);
            continue;
        }
        if !p.already_reported_here() {
            p.error_expected(
                codes::UNEXPECTED_TOKEN,
                "a line break or `}`",
                "put each member on its own line",
            );
        }
        p.skip_to_safe_point();
    }
    p.bracket = saved;
    if !colon {
        p.close_bracket(open, "the body");
    }
    p.complete(marker, MEMBER_LIST);
}

fn at_member_start(p: &Parser<'_>, container: Container) -> bool {
    match p.current() {
        AT | PUB_KW | FN_KW | MUT_KW | STATIC_KW | COMPTIME_KW => true,
        kind => kind.is_ident() && container != Container::Trait,
    }
}

#[cold]
fn comma_after_method(p: &mut Parser<'_>, comma: Span) {
    let indent = indentation_at(p.text(), comma.start).to_string();
    let rest = &p.text()[comma.end as usize..];
    let spaces = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    p.push_error(
        Diagnostic::error("a comma can't follow a method")
            .with_code(codes::COMMA_AFTER_METHOD)
            .primary(comma, "this comma comes after a method")
            .note("only fields and enum options can be separated with commas")
            .suggest(
                "put the next member on a new line",
                Span::new(comma.start, comma.end + spaces as u32),
                format!("\n{indent}"),
            ),
    );
}

#[cold]
fn missing_member_separator(p: &mut Parser<'_>, previous: SyntaxKind) {
    if p.already_reported_here() || p.speculating() {
        return;
    }
    let span = p.nth_span(0);
    let between = stmts::spaces_before(p, span.start);
    let diagnostic = Diagnostic::error("two members on one line need a separator")
        .with_code(codes::MISSING_SEPARATOR)
        .primary(span, "this member starts on the same line as the one before it");
    let diagnostic = if previous == FN_DECL {
        let indent = indentation_at(p.text(), between.start).to_string();
        diagnostic.suggest("put the next member on a new line", between, format!("\n{indent}"))
    } else {
        diagnostic.suggest("separate members on one line with `,`", between, ", ")
    };
    p.push_error(diagnostic);
}

/// One member: `Attributes [ "pub" ] ( Field | FnDecl )` in a struct, an option or a method
/// in an enum, a function in a trait or extend. Returns the member's kind.
fn member(p: &mut Parser<'_>, container: Container) -> SyntaxKind {
    let marker = p.start();
    attributes(p);
    let mods = modifiers(p);
    let foreign_fn = habits::foreign_keyword(p) == Some(FN_KW);
    let kind = match p.current() {
        FN_KW | LOWER_IDENT if p.at(FN_KW) || foreign_fn => {
            if container == Container::Trait {
                if let Some(span) = mods.public {
                    pub_not_allowed(
                        p,
                        span,
                        "`pub` can't be used on a trait member",
                        Some("trait members share the trait's visibility"),
                    );
                }
            }
            fn_rest(p, container);
            FN_DECL
        }
        kind if kind.is_ident() && container == Container::Enum => {
            no_modifier(p, mods);
            if let Some(span) = mods.public {
                pub_not_allowed(
                    p,
                    span,
                    "`pub` can't be used on an enum option",
                    Some("enum options share the enum's visibility"),
                );
            }
            enum_option(p);
            ENUM_OPTION
        }
        kind if kind.is_ident() => {
            no_modifier(p, mods);
            if matches!(container, Container::Trait | Container::Extend) {
                field_not_allowed(p, container);
            }
            field(p);
            FIELD
        }
        _ => {
            let what = match container {
                Container::Enum => "an enum option or a method",
                Container::Trait | Container::Extend => "a method (`fn`)",
                _ => "a field or a method",
            };
            p.error_expected(
                codes::UNEXPECTED_TOKEN,
                what,
                "each member is a field (`name: Type`) or a method (`fn name() { ... }`)",
            );
            if p.consumed_since(&marker) {
                p.complete(marker, ERROR);
            } else {
                p.abandon(marker);
            }
            return ERROR;
        }
    };
    p.complete(marker, kind);
    kind
}

#[cold]
fn field_not_allowed(p: &mut Parser<'_>, container: Container) {
    let span = p.nth_span(0);
    let message = if container == Container::Trait {
        "a trait holds only functions"
    } else {
        "an `extend` block can add methods, not fields"
    };
    p.push_error(
        Diagnostic::error(message)
            .with_code(codes::MEMBER_NOT_ALLOWED)
            .primary(span, "this is a field")
            .help("declare fields in the `struct` itself, or turn this into a method: `fn name() -> Type`"),
    );
}

/// `Field = LOWER_IDENT ":" Type [ "=" Expr ]`.
fn field(p: &mut Parser<'_>) {
    if at_type_before_name(p, &[COMMA, NEWLINE, R_BRACE, EQ, EOF]) {
        type_before_name(p);
        return;
    }
    declared_name(p, Case::Lower, "field names", "a field name");
    if p.eat(COLON) || type_without_colon(p) {
        types::type_(p, MutRule::Forbidden);
    } else if let Some(diagnostic) = p.expected(codes::EXPECTED_TOKEN, "`:` and the field's type") {
        let at = Span::at(p.last_end());
        p.push_error(diagnostic.note("every field has a type").suggest(
            "add the type",
            at,
            ": Type",
        ));
    }
    if p.eat(EQ) {
        exprs::expr(p, Ctx::default());
    }
}

/// Whether the current token is a type name followed by a lowercase name and then one of
/// `ends`, as in `Int amount` (the order of C, Java and C#).
fn at_type_before_name(p: &Parser<'_>, ends: &[SyntaxKind]) -> bool {
    p.at(UPPER_IDENT)
        && p.nth(1) == LOWER_IDENT
        && !p.nth_line_break_before(1)
        && ends.contains(&p.nth(2))
}

/// `Int amount`: reported, with the fix `amount: Int`. The type goes into an error node, then
/// come the name and any default value.
fn type_before_name(p: &mut Parser<'_>) {
    let ty = p.nth_span(0);
    let name = p.nth_span(1);
    p.bump_error();
    p.bump();
    if p.eat(EQ) {
        exprs::expr(p, Ctx::default());
    }
    if p.speculating() {
        return;
    }
    let ty_text = p.text_of(ty).to_string();
    let name_text = p.text_of(name).to_string();
    p.push_error(
        Diagnostic::error("Quell writes the name before the type")
            .with_code(codes::TYPE_BEFORE_NAME)
            .primary(ty, "the type comes first here")
            .note("a name and its type are written `name: Type`, as in `amount: Int`")
            .suggest(
                format!("write `{name_text}: {ty_text}`"),
                ty.to(name),
                format!("{name_text}: {ty_text}"),
            ),
    );
}

/// After a declared name, a type on the same line with no `:` before it, as in `amount Int`
/// (from Go). Reports the missing `:` with a fix that adds it, and returns whether a type
/// follows, so the caller reads it.
pub(crate) fn type_without_colon(p: &mut Parser<'_>) -> bool {
    if !p.current().is_ident() || p.nth_line_break_before(0) {
        return false;
    }
    let ends = matches!(p.nth(1), COMMA | R_PAREN | R_BRACE | EQ | QUESTION | LT | NEWLINE | EOF);
    if !(p.at(UPPER_IDENT) || ends) {
        return false;
    }
    if let Some(diagnostic) = p.expected(codes::EXPECTED_TOKEN, "`:` before the type") {
        let at = Span::at(p.last_end());
        p.push_error(diagnostic.suggest("add `:`", at, ":"));
    }
    true
}

/// `EnumOption = UPPER_IDENT [ "(" OptionField { "," OptionField } [ "," ] ")" ]`.
fn enum_option(p: &mut Parser<'_>) {
    declared_name(p, Case::Upper, "enum options", "an option name");
    if !p.at(L_PAREN) {
        return;
    }
    let marker = p.start();
    let open = p.pos();
    let open_span = p.nth_span(0);
    p.bump();
    if p.partner_of(open) == Some(p.pos()) {
        let close = p.nth_span(0);
        if !p.speculating() {
            p.push_error(
                Diagnostic::error("an option with no fields has no parentheses")
                    .with_code(codes::EMPTY_OPTION_PARENTHESES)
                    .primary(open_span.to(close), "empty parentheses")
                    .suggest("remove the empty parentheses", open_span.to(close), ""),
            );
        }
    }
    let saved = p.bracket;
    p.bracket = Bracket::Other;
    let mut unnamed = Vec::new();
    exprs::delimited(p, open, "an option field", |p| option_field(p, &mut unnamed));
    p.bracket = saved;
    p.close_bracket(open, "the option's fields");
    p.complete(marker, OPTION_FIELD_LIST);
    unnamed_option_fields(p, &unnamed);
}

/// One error for all the fields of an option that have a type but no name (3.6), with a fix
/// that names each one after its type.
#[cold]
fn unnamed_option_fields(p: &mut Parser<'_>, unnamed: &[Span]) {
    let (Some(&first), false) = (unnamed.first(), p.speculating()) else { return };
    let mut used = Vec::new();
    let edits = unnamed
        .iter()
        .map(|&ty| {
            let base = field_name_for_type(p.text_of(ty));
            // `Pair(Int, Int)` gets `int` and `int2`.
            let mut name = base.clone();
            let mut number = 1;
            while used.contains(&name) {
                number += 1;
                name = format!("{base}{number}");
            }
            used.push(name.clone());
            Edit { span: Span::at(ty.start), replacement: format!("{name}: ") }
        })
        .collect::<Vec<_>>();
    let example = format!("{}{}", edits[0].replacement, p.text_of(first));
    let (message, label) = if unnamed.len() == 1 {
        ("this field of an enum option needs a name", "this field has a type but no name")
    } else {
        ("these fields of an enum option need names", "these fields have a type but no name")
    };
    let mut diagnostic = Diagnostic::error(message)
        .with_code(codes::UNNAMED_OPTION_FIELD)
        .primary(first.to(*unnamed.last().unwrap_or(&first)), label)
        .note("options are built and matched by field name, as in `Circle(radius: 1.0)`");
    for &other in &unnamed[1..] {
        diagnostic = diagnostic.secondary(other, "no name");
    }
    p.push_error(diagnostic.suggest_edits(format!("name each field, for example `{example}`"), edits));
}

/// `OptionField = LOWER_IDENT ":" Type`. A field with a type and no name is read, and its
/// type's span is added to `unnamed`.
fn option_field(p: &mut Parser<'_>, unnamed: &mut Vec<Span>) -> bool {
    let marker = p.start();
    if p.current().is_ident() && p.nth(1) == COLON {
        declared_name(p, Case::Lower, "field names", "a field name");
        p.bump();
        types::type_(p, MutRule::Forbidden);
        p.complete(marker, OPTION_FIELD);
        return true;
    }
    match types::type_(p, MutRule::Forbidden) {
        Some(ty) => {
            unnamed.push(ty.span());
            p.complete(marker, OPTION_FIELD);
            true
        }
        None => {
            p.abandon(marker);
            false
        }
    }
}

// ----------------------------------------------------------------------
// Imports, tests, compile-time items and top-level let (3.9 to 3.12)
// ----------------------------------------------------------------------

/// `ImportDecl = "import" ( ModulePath [ "." "{" ImportList "}" ] | "c" PLAIN_STRING )`.
fn import_decl(p: &mut Parser<'_>) {
    p.bump();
    if p.at(LOWER_IDENT) && p.nth_text(0) == "c" && p.nth(1) == STRING_START {
        p.bump();
        let string = exprs::string_literal(p);
        exprs::require_plain(p, &string, "the name of a C header");
        return;
    }
    let path = p.start();
    if p.current().is_ident() {
        p.bump();
    } else {
        p.error_expected(
            codes::EXPECTED_NAME,
            "a module name",
            "write the module to import, as in `import std.math`",
        );
        p.abandon(path);
        return;
    }
    loop {
        if p.at(DOT) && p.nth(1).is_ident() {
            p.bump();
            p.bump();
        } else if p.at(COLON) && p.nth(1) == COLON && p.nth_is_joined(1) && p.nth(2).is_ident() {
            exprs::double_colon(p);
            p.bump();
        } else {
            break;
        }
    }
    p.complete(path, MODULE_PATH);
    if p.at(DOT) && p.nth(1) == L_BRACE {
        p.bump();
        import_list(p);
    } else if p.at(DOT) {
        p.bump();
        p.error_expected(
            codes::EXPECTED_NAME,
            "a module name or `{`",
            "after `.`, write a module name or a list of names in `{ }`",
        );
    }
}

/// `"{" ImportList "}"` with `ImportList = IDENT { "," IDENT } [ "," ] [ NEWLINE ]`.
fn import_list(p: &mut Parser<'_>) {
    let marker = p.start();
    let open = p.pos();
    p.bump();
    let close = p.list_end(open);
    let mut empty = true;
    loop {
        if p.at(NEWLINE) {
            if stmts::at_list_close_at(p, close, p.pos() + 1) {
                p.bump();
                break;
            }
            if !empty {
                exprs::forgotten_comma_error(p);
            }
            p.bump();
            continue;
        }
        if stmts::at_list_close(p, close) {
            break;
        }
        let start = p.pos();
        if p.current().is_ident() {
            p.bump();
            empty = false;
        } else {
            p.error_expected(
                codes::EXPECTED_NAME,
                "a name to import",
                "list the names to import, separated by commas",
            );
            p.skip_to_safe_point();
            if p.pos() == start && !stmts::at_list_close(p, close) && !p.at(NEWLINE) {
                p.bump_error();
            }
            if p.pos() == start {
                break;
            }
            continue;
        }
        if p.eat(COMMA) || p.at(NEWLINE) || stmts::at_list_close(p, close) {
            continue;
        }
        if p.current().is_ident() {
            exprs::missing_comma(p);
            continue;
        }
        p.error_expected(codes::EXPECTED_TOKEN, "`,` or `}`", "separate the names with commas");
        p.skip_to_safe_point();
    }
    if empty && !p.speculating() && close.is_some() && !p.already_reported_here() {
        p.error_expected(
            codes::EXPECTED_NAME,
            "a name to import",
            "list the names to import, as in `import std.math.{pi}`",
        );
    }
    p.close_bracket(open, "the list of names");
    p.complete(marker, IMPORT_LIST);
}

/// `TestDecl = "test" PLAIN_STRING Block`.
fn test_decl(p: &mut Parser<'_>) {
    p.bump(); // `test`
    let string = exprs::string_literal(p);
    exprs::require_plain(p, &string, "a test name");
    stmts::body_block(p, "the test's name", None);
}

/// `GlobalLet = [ "comptime" ] ( "let" | "var" ) BindingPattern [ ":" Type ] "=" Expr`.
fn global_let(p: &mut Parser<'_>, mods: Modifiers) {
    let keyword = p.current();
    let keyword_span = p.nth_span(0);
    if keyword == VAR_KW {
        if let Some(span) = mods.public {
            pub_not_allowed(
                p,
                span,
                "`pub` can't be used on a top-level `var`",
                Some("a value that other files can see must not change; use `pub let`"),
            );
        }
    }
    match mods.modifier {
        Some((COMPTIME_KW, _)) if keyword == VAR_KW => {
            p.push_error(
                Diagnostic::error("`comptime var` is not allowed")
                    .with_code(codes::COMPTIME_VAR)
                    .primary(keyword_span, "a compile-time value can't change")
                    .suggest("write `comptime let`", keyword_span, "let"),
            );
        }
        Some((kind @ (MUT_KW | STATIC_KW), span)) => {
            no_modifier(p, Modifiers { public: None, modifier: Some((kind, span)) });
        }
        _ => {}
    }
    stmts::let_rest(p);
}
