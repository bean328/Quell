//! The typed view of the syntax tree (docs/decisions/0005-front-end-architecture.md).
//!
//! Each type here wraps a [`SyntaxNode`] of one kind and has accessors for its parts, so later
//! stages don't work with raw nodes. The wrappers are cheap (a reference-counted pointer) and
//! never fail: on code with errors, an accessor returns `None` for a part that is missing.
//!
//! ```
//! use quell_syntax::ast::{Expr, Item};
//! use quell_syntax::{FileRole, parse};
//!
//! let parse = parse("fn square(x: Int) -> Int = x * x\n", FileRole::Library);
//! let Some(Item::Fn(function)) = parse.tree().items().next() else { panic!() };
//! assert_eq!(function.name().unwrap().text(), "square");
//! assert_eq!(function.params().count(), 1);
//! let Some(Expr::Binary(body)) = function.expr_body() else { panic!() };
//! assert_eq!(body.op().unwrap().text(), "*");
//! ```

use crate::SyntaxKind::{self, *};
use crate::{SyntaxNode, SyntaxToken};

/// A typed wrapper over a node of one kind (or, for the enums, one of a few kinds).
pub trait AstNode: Sized {
    /// Whether a node of this kind can be wrapped.
    fn can_cast(kind: SyntaxKind) -> bool;
    /// Wraps `node` if it has the right kind.
    fn cast(node: SyntaxNode) -> Option<Self>;
    /// The node itself.
    fn syntax(&self) -> &SyntaxNode;
}

/// The first child of `node` that is an `N`.
fn child<N: AstNode>(node: &SyntaxNode) -> Option<N> {
    node.children().find_map(N::cast)
}

/// The children of `node` that are `N`s.
fn children<N: AstNode>(node: &SyntaxNode) -> impl Iterator<Item = N> + use<N> {
    node.children().filter_map(N::cast)
}

/// The direct child tokens of `node` (no trivia).
fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> + use<> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
}

/// The first direct child token of `node` of this kind.
fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    tokens(node).find(|token| token.kind() == kind)
}

/// The name token that comes right after the token of kind `after` (an identifier, or a
/// keyword written where a name should be).
fn name_after(node: &SyntaxNode, after: SyntaxKind) -> Option<SyntaxToken> {
    let mut seen = false;
    for token in tokens(node) {
        if seen {
            let kind = token.kind();
            return (kind.is_ident() || kind.is_keyword()).then_some(token);
        }
        seen = token.kind() == after;
    }
    None
}

/// The first identifier token among the direct children of `node`.
fn first_ident(node: &SyntaxNode) -> Option<SyntaxToken> {
    tokens(node).find(|token| token.kind().is_ident())
}

macro_rules! ast_node {
    ($(#[$meta:meta])* $name:ident, $kind:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub struct $name {
            pub(crate) syntax: SyntaxNode,
        }

        impl AstNode for $name {
            fn can_cast(kind: SyntaxKind) -> bool {
                kind == $kind
            }

            fn cast(node: SyntaxNode) -> Option<Self> {
                Self::can_cast(node.kind()).then_some($name { syntax: node })
            }

            fn syntax(&self) -> &SyntaxNode {
                &self.syntax
            }
        }
    };
}

macro_rules! ast_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident($ty:ident)),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $(#[allow(missing_docs)] $variant($ty),)*
        }

        impl AstNode for $name {
            fn can_cast(kind: SyntaxKind) -> bool {
                $($ty::can_cast(kind))||*
            }

            fn cast(node: SyntaxNode) -> Option<Self> {
                $(if $ty::can_cast(node.kind()) {
                    return Some($name::$variant($ty { syntax: node }));
                })*
                None
            }

            fn syntax(&self) -> &SyntaxNode {
                match self {
                    $($name::$variant(it) => it.syntax(),)*
                }
            }
        }
    };
}

// ----------------------------------------------------------------------
// Files and items (grammar.md, 2 and 3)
// ----------------------------------------------------------------------

ast_node!(
    /// A whole file.
    SourceFile, FILE
);

impl SourceFile {
    /// The items of the file, in order.
    pub fn items(&self) -> impl Iterator<Item = Item> + use<> {
        children(&self.syntax)
    }

    /// The top-level statements of the file, in order (only an entry file should have them).
    pub fn statements(&self) -> impl Iterator<Item = Stmt> + use<> {
        children(&self.syntax)
    }
}

ast_enum!(
    /// An item: a declaration at the top level of a file.
    Item {
        Fn(FnDecl),
        Struct(StructDecl),
        Enum(EnumDecl),
        Trait(TraitDecl),
        Extend(ExtendDecl),
        Import(ImportDecl),
        Test(TestDecl),
        Comptime(ComptimeBlock),
        GlobalLet(GlobalLet),
    }
);

/// What items, fields, options and methods have in common.
pub trait HasDocs: AstNode {
    /// The doc comment lines right before the element (`///` included).
    fn doc_comments(&self) -> Vec<SyntaxToken> {
        self.syntax()
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| token.kind() == DOC_COMMENT)
            .collect()
    }

    /// The attributes before the element.
    fn attributes(&self) -> Vec<Attribute> {
        children(self.syntax()).collect()
    }

    /// Whether the element is marked `pub`.
    fn is_pub(&self) -> bool {
        token(self.syntax(), PUB_KW).is_some()
    }
}

impl HasDocs for Item {}
impl HasDocs for FnDecl {}
impl HasDocs for StructDecl {}
impl HasDocs for EnumDecl {}
impl HasDocs for TraitDecl {}
impl HasDocs for ExtendDecl {}
impl HasDocs for Field {}
impl HasDocs for EnumOption {}
impl HasDocs for GlobalLet {}

ast_node!(
    /// `@name` or `@name(args)`.
    Attribute, ATTRIBUTE
);

impl Attribute {
    /// The dotted name after `@`.
    pub fn path(&self) -> Vec<SyntaxToken> {
        tokens(&self.syntax).filter(|token| token.kind().is_ident()).collect()
    }

    /// The arguments, if there are parentheses.
    pub fn args(&self) -> Option<ArgList> {
        child(&self.syntax)
    }
}

ast_node!(
    /// A function or method.
    FnDecl, FN_DECL
);

/// The body of a function: a block, or `= expr`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FnBody {
    /// `{ ... }`.
    Block(BlockExpr),
    /// `= expr`.
    Expr(Expr),
}

impl FnDecl {
    /// The function's name.
    pub fn name(&self) -> Option<SyntaxToken> {
        name_after(&self.syntax, FN_KW)
    }

    /// `mut`, `static` or `comptime`, if there is one.
    pub fn modifier(&self) -> Option<SyntaxToken> {
        tokens(&self.syntax).find(|token| matches!(token.kind(), MUT_KW | STATIC_KW | COMPTIME_KW))
    }

    /// The generic parameters, if any.
    pub fn generic_params(&self) -> Option<GenericParamList> {
        child(&self.syntax)
    }

    /// The parameters.
    pub fn params(&self) -> impl Iterator<Item = Param> + use<> {
        child::<ParamList>(&self.syntax).into_iter().flat_map(|list| children(&list.syntax))
    }

    /// The type after `->`.
    pub fn return_type(&self) -> Option<Type> {
        child::<RetType>(&self.syntax).and_then(|ret| child(&ret.syntax))
    }

    /// The `throws` clause.
    pub fn throws(&self) -> Option<ThrowsClause> {
        child(&self.syntax)
    }

    /// The body, or `None` for a requirement in a trait.
    pub fn body(&self) -> Option<FnBody> {
        if token(&self.syntax, EQ).is_some() {
            return self.expr_body().map(FnBody::Expr);
        }
        child(&self.syntax).map(FnBody::Block)
    }

    /// The expression of an `= expr` body.
    pub fn expr_body(&self) -> Option<Expr> {
        token(&self.syntax, EQ)?;
        children::<Expr>(&self.syntax).last()
    }
}

ast_node!(
    /// The parameter list of a function.
    ParamList, PARAM_LIST
);

ast_node!(
    /// One parameter: `name: Type = default`.
    Param, PARAM
);

impl Param {
    /// The parameter's name.
    pub fn name(&self) -> Option<SyntaxToken> {
        tokens(&self.syntax)
            .next()
            .filter(|token| token.kind().is_ident() || token.kind().is_keyword())
    }

    /// The type (a `mut` type for a parameter the function may change).
    pub fn ty(&self) -> Option<Type> {
        child(&self.syntax)
    }

    /// The default value.
    pub fn default_value(&self) -> Option<Expr> {
        token(&self.syntax, EQ)?;
        child(&self.syntax)
    }
}

ast_node!(
    /// `-> Type`.
    RetType, RET_TYPE
);

ast_node!(
    /// `throws` with an optional error type.
    ThrowsClause, THROWS_CLAUSE
);

impl ThrowsClause {
    /// The error type; `None` means any error.
    pub fn error_type(&self) -> Option<Type> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `<T, U: Compare>` in a declaration.
    GenericParamList, GENERIC_PARAM_LIST
);

impl GenericParamList {
    /// The parameters.
    pub fn params(&self) -> impl Iterator<Item = GenericParam> + use<> {
        children(&self.syntax)
    }
}

ast_node!(
    /// One generic parameter with its requirements.
    GenericParam, GENERIC_PARAM
);

impl GenericParam {
    /// The parameter's name.
    pub fn name(&self) -> Option<SyntaxToken> {
        first_ident(&self.syntax)
    }

    /// The requirements after `:`, joined by `and`.
    pub fn requirements(&self) -> impl Iterator<Item = Type> + use<> {
        children(&self.syntax)
    }
}

ast_node!(
    /// `struct Name { ... }`.
    StructDecl, STRUCT_DECL
);

impl StructDecl {
    /// The struct's name.
    pub fn name(&self) -> Option<SyntaxToken> {
        name_after(&self.syntax, STRUCT_KW)
    }

    /// The generic parameters, if any.
    pub fn generic_params(&self) -> Option<GenericParamList> {
        child(&self.syntax)
    }

    /// The traits after `:`.
    pub fn conformances(&self) -> impl Iterator<Item = Type> + use<> {
        child::<Conformance>(&self.syntax).into_iter().flat_map(|it| children(&it.syntax))
    }

    /// The fields.
    pub fn fields(&self) -> impl Iterator<Item = Field> + use<> {
        members(&self.syntax)
    }

    /// The methods.
    pub fn methods(&self) -> impl Iterator<Item = FnDecl> + use<> {
        members(&self.syntax)
    }
}

/// The members of a struct, enum, trait or extend body that are `N`s.
fn members<N: AstNode>(node: &SyntaxNode) -> impl Iterator<Item = N> + use<N> {
    child::<MemberList>(node).into_iter().flat_map(|list| children(&list.syntax))
}

ast_node!(
    /// `: Trait and Other`.
    Conformance, CONFORMANCE
);

ast_node!(
    /// The `{ }` body of a struct, enum, trait or extend.
    MemberList, MEMBER_LIST
);

ast_node!(
    /// A struct field: `name: Type = default`.
    Field, FIELD
);

impl Field {
    /// The field's name.
    pub fn name(&self) -> Option<SyntaxToken> {
        tokens(&self.syntax).find(|token| {
            token.kind().is_ident() || token.kind().is_keyword() && token.kind() != PUB_KW
        })
    }

    /// The field's type.
    pub fn ty(&self) -> Option<Type> {
        child(&self.syntax)
    }

    /// The default value.
    pub fn default_value(&self) -> Option<Expr> {
        token(&self.syntax, EQ)?;
        child(&self.syntax)
    }
}

ast_node!(
    /// `enum Name { ... }`.
    EnumDecl, ENUM_DECL
);

impl EnumDecl {
    /// The enum's name.
    pub fn name(&self) -> Option<SyntaxToken> {
        name_after(&self.syntax, ENUM_KW)
    }

    /// The traits after `:`.
    pub fn conformances(&self) -> impl Iterator<Item = Type> + use<> {
        child::<Conformance>(&self.syntax).into_iter().flat_map(|it| children(&it.syntax))
    }

    /// The options.
    pub fn options(&self) -> impl Iterator<Item = EnumOption> + use<> {
        members(&self.syntax)
    }

    /// The methods.
    pub fn methods(&self) -> impl Iterator<Item = FnDecl> + use<> {
        members(&self.syntax)
    }
}

ast_node!(
    /// An enum option: `Rainy(mm: Float)`.
    EnumOption, ENUM_OPTION
);

impl EnumOption {
    /// The option's name.
    pub fn name(&self) -> Option<SyntaxToken> {
        first_ident(&self.syntax)
    }

    /// The option's fields.
    pub fn fields(&self) -> impl Iterator<Item = OptionField> + use<> {
        child::<OptionFieldList>(&self.syntax).into_iter().flat_map(|list| children(&list.syntax))
    }
}

ast_node!(
    /// The `( )` of an option's fields.
    OptionFieldList, OPTION_FIELD_LIST
);

ast_node!(
    /// One field of an enum option.
    OptionField, OPTION_FIELD
);

impl OptionField {
    /// The field's name.
    pub fn name(&self) -> Option<SyntaxToken> {
        token(&self.syntax, COLON)?;
        first_ident(&self.syntax)
    }

    /// The field's type.
    pub fn ty(&self) -> Option<Type> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `trait Name { ... }`.
    TraitDecl, TRAIT_DECL
);

impl TraitDecl {
    /// The trait's name.
    pub fn name(&self) -> Option<SyntaxToken> {
        name_after(&self.syntax, TRAIT_KW)
    }

    /// The requirements and default methods.
    pub fn methods(&self) -> impl Iterator<Item = FnDecl> + use<> {
        members(&self.syntax)
    }
}

ast_node!(
    /// `extend Type { ... }`.
    ExtendDecl, EXTEND_DECL
);

impl ExtendDecl {
    /// The type that is extended.
    pub fn ty(&self) -> Option<Type> {
        child(&self.syntax)
    }

    /// The traits after `:`.
    pub fn conformances(&self) -> impl Iterator<Item = Type> + use<> {
        child::<Conformance>(&self.syntax).into_iter().flat_map(|it| children(&it.syntax))
    }

    /// The methods.
    pub fn methods(&self) -> impl Iterator<Item = FnDecl> + use<> {
        members(&self.syntax)
    }
}

ast_node!(
    /// `import std.math.{pi}` or `import c "header.h"`.
    ImportDecl, IMPORT_DECL
);

impl ImportDecl {
    /// The module path, as its names: `["std", "math"]`.
    pub fn path(&self) -> Vec<SyntaxToken> {
        child::<ModulePath>(&self.syntax)
            .map(|path| tokens(&path.syntax).filter(|token| token.kind().is_ident()).collect())
            .unwrap_or_default()
    }

    /// The names in `{ }`, if there is a list.
    pub fn names(&self) -> Option<Vec<SyntaxToken>> {
        let list = child::<ImportList>(&self.syntax)?;
        Some(tokens(&list.syntax).filter(|token| token.kind().is_ident()).collect())
    }

    /// The header of `import c "header.h"`.
    pub fn c_header(&self) -> Option<StringLiteral> {
        child(&self.syntax)
    }
}

ast_node!(
    /// The dotted path of an import.
    ModulePath, MODULE_PATH
);

ast_node!(
    /// The `{ }` list of an import.
    ImportList, IMPORT_LIST
);

ast_node!(
    /// `test "name" { ... }`.
    TestDecl, TEST_DECL
);

impl TestDecl {
    /// The test's name.
    pub fn name(&self) -> Option<StringLiteral> {
        child(&self.syntax)
    }

    /// The test's body.
    pub fn body(&self) -> Option<BlockExpr> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `comptime { ... }`.
    ComptimeBlock, COMPTIME_BLOCK
);

impl ComptimeBlock {
    /// The block.
    pub fn body(&self) -> Option<BlockExpr> {
        child(&self.syntax)
    }
}

ast_node!(
    /// A `let` or `var` at the top level of a file.
    GlobalLet, GLOBAL_LET
);

impl GlobalLet {
    /// Whether this is a `var`.
    pub fn is_var(&self) -> bool {
        token(&self.syntax, VAR_KW).is_some()
    }

    /// Whether this is a `comptime let`.
    pub fn is_comptime(&self) -> bool {
        token(&self.syntax, COMPTIME_KW).is_some()
    }

    /// The pattern: a name or a tuple of names.
    pub fn pattern(&self) -> Option<Pattern> {
        child(&self.syntax)
    }

    /// The type annotation.
    pub fn ty(&self) -> Option<Type> {
        child(&self.syntax)
    }

    /// The starting value.
    pub fn value(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

// ----------------------------------------------------------------------
// Blocks and statements (grammar.md, 4)
// ----------------------------------------------------------------------

ast_node!(
    /// `{ statements }`, as a body or as a value.
    BlockExpr, BLOCK
);

impl BlockExpr {
    /// The statements, in order.
    pub fn statements(&self) -> impl Iterator<Item = Stmt> + use<> {
        children(&self.syntax)
    }

    /// The block's value: the expression of its last statement, if that is an expression
    /// statement (4.1).
    pub fn value(&self) -> Option<Expr> {
        match self.statements().last()? {
            Stmt::Expr(statement) => statement.expr(),
            _ => None,
        }
    }
}

ast_enum!(
    /// A statement.
    Stmt {
        Let(LetStmt),
        Assign(AssignStmt),
        For(ForStmt),
        While(WhileStmt),
        Repeat(RepeatStmt),
        Defer(DeferStmt),
        Assert(AssertStmt),
        Expr(ExprStmt),
    }
);

ast_node!(
    /// `let` or `var` in a block.
    LetStmt, LET_STMT
);

impl LetStmt {
    /// Whether this is a `var`.
    pub fn is_var(&self) -> bool {
        token(&self.syntax, VAR_KW).is_some()
    }

    /// The pattern.
    pub fn pattern(&self) -> Option<Pattern> {
        child(&self.syntax)
    }

    /// The type annotation.
    pub fn ty(&self) -> Option<Type> {
        child(&self.syntax)
    }

    /// The starting value.
    pub fn value(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `place = value`, or a compound assignment.
    AssignStmt, ASSIGN_STMT
);

impl AssignStmt {
    /// The place that is changed.
    pub fn place(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    /// The operator: `=`, `+=`, ...
    pub fn op(&self) -> Option<SyntaxToken> {
        tokens(&self.syntax).next()
    }

    /// The new value.
    pub fn value(&self) -> Option<Expr> {
        children::<Expr>(&self.syntax).nth(1)
    }
}

ast_node!(
    /// `for pattern in items { ... }`.
    ForStmt, FOR_STMT
);

impl ForStmt {
    /// The loop's pattern.
    pub fn pattern(&self) -> Option<Pattern> {
        child(&self.syntax)
    }

    /// The items looped over.
    pub fn iterable(&self) -> Option<Expr> {
        children::<Expr>(&self.syntax).find(|expr| !matches!(expr, Expr::Block(_)))
    }

    /// The body.
    pub fn body(&self) -> Option<BlockExpr> {
        children(&self.syntax).last()
    }
}

ast_node!(
    /// `while condition { ... }`.
    WhileStmt, WHILE_STMT
);

impl WhileStmt {
    /// The condition.
    pub fn condition(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    /// The body.
    pub fn body(&self) -> Option<BlockExpr> {
        children(&self.syntax).last()
    }
}

ast_node!(
    /// `repeat count { ... }` or `repeat { ... }`.
    RepeatStmt, REPEAT_STMT
);

impl RepeatStmt {
    /// The count, if there is one.
    pub fn count(&self) -> Option<Expr> {
        let exprs: Vec<Expr> = children(&self.syntax).collect();
        (exprs.len() == 2).then(|| exprs[0].clone())
    }

    /// The body.
    pub fn body(&self) -> Option<BlockExpr> {
        children(&self.syntax).last()
    }
}

ast_node!(
    /// `defer { ... }`.
    DeferStmt, DEFER_STMT
);

impl DeferStmt {
    /// The block that runs when the enclosing block ends.
    pub fn body(&self) -> Option<BlockExpr> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `assert condition, message`.
    AssertStmt, ASSERT_STMT
);

impl AssertStmt {
    /// The condition.
    pub fn condition(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    /// The message after the comma.
    pub fn message(&self) -> Option<Expr> {
        children::<Expr>(&self.syntax).nth(1)
    }
}

ast_node!(
    /// An expression used as a statement.
    ExprStmt, EXPR_STMT
);

impl ExprStmt {
    /// The expression.
    pub fn expr(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

// ----------------------------------------------------------------------
// Expressions (grammar.md, 6)
// ----------------------------------------------------------------------

ast_enum!(
    /// An expression.
    Expr {
        Literal(Literal),
        String(StringLiteral),
        Name(NameExpr),
        SelfExpr(SelfExpr),
        Paren(ParenExpr),
        Tuple(TupleExpr),
        List(ListExpr),
        Map(MapExpr),
        Block(BlockExpr),
        If(IfExpr),
        Match(MatchExpr),
        ParallelFor(ParallelForExpr),
        Group(GroupExpr),
        Unsafe(UnsafeExpr),
        Spawn(SpawnExpr),
        Prefix(PrefixExpr),
        Try(TryExpr),
        Binary(BinaryExpr),
        Range(RangeExpr),
        Is(IsExpr),
        Catch(CatchExpr),
        Call(CallExpr),
        Member(MemberExpr),
        Index(IndexExpr),
        Lambda(LambdaExpr),
        Return(ReturnExpr),
        Throw(ThrowExpr),
        Break(BreakExpr),
        Continue(ContinueExpr),
    }
);

ast_node!(
    /// A number, a character, `true`, `false` or `none`.
    Literal, LITERAL
);

impl Literal {
    /// The literal's token.
    pub fn token(&self) -> Option<SyntaxToken> {
        tokens(&self.syntax).next()
    }
}

ast_node!(
    /// A string literal, with its holes.
    StringLiteral, STRING_LITERAL
);

impl StringLiteral {
    /// The holes (interpolated values), in order.
    pub fn holes(&self) -> impl Iterator<Item = Hole> + use<> {
        children(&self.syntax)
    }

    /// Whether the string has no holes (a `PLAIN_STRING`).
    pub fn is_plain(&self) -> bool {
        self.holes().next().is_none()
    }

    /// The value of a plain string with valid escapes.
    pub fn plain_value(&self) -> Option<String> {
        let mut value = String::new();
        for element in self.syntax.children_with_tokens() {
            let token = element.into_token()?;
            match token.kind() {
                STRING_TEXT => value.push_str(token.text()),
                STRING_ESCAPE => value.push(crate::escape_value(token.text())?),
                STRING_LINE_BREAK => value.push('\n'),
                _ => {}
            }
        }
        Some(value)
    }
}

ast_node!(
    /// One hole of a string: `{value:spec}`.
    Hole, HOLE
);

impl Hole {
    /// The value.
    pub fn expr(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    /// The format spec token (`:` and the spec), if there is one.
    pub fn format_spec(&self) -> Option<SyntaxToken> {
        token(&self.syntax, FORMAT_SPEC)
    }
}

ast_node!(
    /// A name, with optional generic arguments.
    NameExpr, NAME_EXPR
);

impl NameExpr {
    /// The name.
    pub fn name(&self) -> Option<SyntaxToken> {
        first_ident(&self.syntax)
    }

    /// The generic arguments, as in `List<Int>`.
    pub fn generic_args(&self) -> Option<GenericArgList> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `self`.
    SelfExpr, SELF_EXPR
);

ast_node!(
    /// `(expr)`.
    ParenExpr, PAREN_EXPR
);

impl ParenExpr {
    /// The expression inside.
    pub fn expr(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `(a, b)`.
    TupleExpr, TUPLE_EXPR
);

impl TupleExpr {
    /// The elements.
    pub fn elements(&self) -> impl Iterator<Item = Expr> + use<> {
        children(&self.syntax)
    }
}

ast_node!(
    /// `[a, b]`.
    ListExpr, LIST_EXPR
);

impl ListExpr {
    /// The elements.
    pub fn elements(&self) -> impl Iterator<Item = Expr> + use<> {
        children(&self.syntax)
    }
}

ast_node!(
    /// `[key: value]` or `[:]`.
    MapExpr, MAP_EXPR
);

impl MapExpr {
    /// The entries.
    pub fn entries(&self) -> impl Iterator<Item = MapEntry> + use<> {
        children(&self.syntax)
    }
}

ast_node!(
    /// `key: value`.
    MapEntry, MAP_ENTRY
);

impl MapEntry {
    /// The key.
    pub fn key(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    /// The value.
    pub fn value(&self) -> Option<Expr> {
        children::<Expr>(&self.syntax).nth(1)
    }
}

ast_node!(
    /// `if condition { ... } else ...`.
    IfExpr, IF_EXPR
);

/// The `else` branch of an `if`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ElseBranch {
    /// `else { ... }`.
    Block(BlockExpr),
    /// `else if ...`.
    If(IfExpr),
}

impl IfExpr {
    /// The condition.
    pub fn condition(&self) -> Option<Expr> {
        child(&self.syntax).filter(|expr: &Expr| !matches!(expr, Expr::Block(_) | Expr::If(_)))
    }

    /// The block run when the condition holds.
    pub fn then_branch(&self) -> Option<BlockExpr> {
        self.blocks_before_else().last()
    }

    fn blocks_before_else(&self) -> impl Iterator<Item = BlockExpr> + use<> {
        let else_start = token(&self.syntax, ELSE_KW).map(|token| token.text_range().start());
        self.syntax
            .children()
            .take_while(move |node| {
                else_start.is_none_or(|start| node.text_range().start() < start)
            })
            .filter_map(BlockExpr::cast)
    }

    /// The `else` branch.
    pub fn else_branch(&self) -> Option<ElseBranch> {
        let start = token(&self.syntax, ELSE_KW)?.text_range().start();
        let node = self.syntax.children().find(|node| node.text_range().start() > start)?;
        if let Some(block) = BlockExpr::cast(node.clone()) {
            return Some(ElseBranch::Block(block));
        }
        IfExpr::cast(node).map(ElseBranch::If)
    }
}

ast_node!(
    /// `match value { arms }`.
    MatchExpr, MATCH_EXPR
);

impl MatchExpr {
    /// The value that is matched.
    pub fn scrutinee(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    /// The arms.
    pub fn arms(&self) -> impl Iterator<Item = MatchArm> + use<> {
        child::<MatchArmList>(&self.syntax).into_iter().flat_map(|list| children(&list.syntax))
    }
}

ast_node!(
    /// The `{ }` of a `match`.
    MatchArmList, MATCH_ARM_LIST
);

ast_node!(
    /// `pattern if guard => value`.
    MatchArm, MATCH_ARM
);

impl MatchArm {
    /// The pattern.
    pub fn pattern(&self) -> Option<Pattern> {
        child(&self.syntax)
    }

    /// The guard's condition.
    pub fn guard(&self) -> Option<Expr> {
        child::<MatchGuard>(&self.syntax).and_then(|guard| child(&guard.syntax))
    }

    /// The value after `=>`.
    pub fn body(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `if guard` in a `match` arm.
    MatchGuard, MATCH_GUARD
);

ast_node!(
    /// `parallel for pattern in items { ... }`.
    ParallelForExpr, PARALLEL_FOR_EXPR
);

impl ParallelForExpr {
    /// The loop's pattern.
    pub fn pattern(&self) -> Option<Pattern> {
        child(&self.syntax)
    }

    /// The items.
    pub fn iterable(&self) -> Option<Expr> {
        children::<Expr>(&self.syntax).find(|expr| !matches!(expr, Expr::Block(_)))
    }

    /// The body.
    pub fn body(&self) -> Option<BlockExpr> {
        children(&self.syntax).last()
    }
}

ast_node!(
    /// `group { ... }`.
    GroupExpr, GROUP_EXPR
);

impl GroupExpr {
    /// The block.
    pub fn body(&self) -> Option<BlockExpr> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `unsafe { ... }`.
    UnsafeExpr, UNSAFE_EXPR
);

impl UnsafeExpr {
    /// The block.
    pub fn body(&self) -> Option<BlockExpr> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `spawn { ... }` or `spawn call()`.
    SpawnExpr, SPAWN_EXPR
);

impl SpawnExpr {
    /// The block or the call that the task runs.
    pub fn task(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `-x`, `~x` or `not x`.
    PrefixExpr, PREFIX_EXPR
);

impl PrefixExpr {
    /// The operator.
    pub fn op(&self) -> Option<SyntaxToken> {
        tokens(&self.syntax).next()
    }

    /// The operator's kind, with `!` read as `not`.
    pub fn op_kind(&self) -> Option<SyntaxKind> {
        self.op().map(|op| op.kind().recovered())
    }

    /// The operand.
    pub fn operand(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `try x`.
    TryExpr, TRY_EXPR
);

impl TryExpr {
    /// The operand.
    pub fn operand(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `lhs op rhs`: arithmetic, bitwise, comparison, `and`, `or` and `??`.
    BinaryExpr, BINARY_EXPR
);

impl BinaryExpr {
    /// The left operand.
    pub fn lhs(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    /// The operator.
    pub fn op(&self) -> Option<SyntaxToken> {
        tokens(&self.syntax).find(|token| token.kind() != NEWLINE)
    }

    /// The operator's kind, with `&&` read as `and` and `||` as `or`.
    pub fn op_kind(&self) -> Option<SyntaxKind> {
        self.op().map(|op| op.kind().recovered())
    }

    /// The right operand.
    pub fn rhs(&self) -> Option<Expr> {
        children::<Expr>(&self.syntax).nth(1)
    }
}

ast_node!(
    /// `start..<end` or `start..=end`.
    RangeExpr, RANGE_EXPR
);

impl RangeExpr {
    /// The start.
    pub fn start(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    /// The end.
    pub fn end(&self) -> Option<Expr> {
        children::<Expr>(&self.syntax).nth(1)
    }

    /// Whether the end is included (`..=`).
    pub fn is_inclusive(&self) -> bool {
        token(&self.syntax, DOT_DOT_EQ).is_some()
    }
}

ast_node!(
    /// `value is Type`.
    IsExpr, IS_EXPR
);

impl IsExpr {
    /// The value tested.
    pub fn value(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    /// The type.
    pub fn ty(&self) -> Option<Type> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `value catch fallback` or `value catch name { ... }`.
    CatchExpr, CATCH_EXPR
);

impl CatchExpr {
    /// The value that might fail.
    pub fn value(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    /// The name the error is bound to, in the binding form.
    pub fn binding(&self) -> Option<SyntaxToken> {
        name_after(&self.syntax, CATCH_KW).or_else(|| {
            let mut seen = false;
            tokens(&self.syntax).find(|token| {
                let found = seen && token.kind() == UNDERSCORE;
                seen |= token.kind() == CATCH_KW;
                found
            })
        })
    }

    /// The block of the binding form.
    pub fn handler(&self) -> Option<BlockExpr> {
        self.binding()?;
        children(&self.syntax).last()
    }

    /// The fallback value (or jump) of the fallback form.
    pub fn fallback(&self) -> Option<Expr> {
        if self.binding().is_some() {
            return None;
        }
        children::<Expr>(&self.syntax).nth(1)
    }
}

ast_node!(
    /// `callee(args)`.
    CallExpr, CALL_EXPR
);

impl CallExpr {
    /// What is called.
    pub fn callee(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    /// The arguments.
    pub fn args(&self) -> impl Iterator<Item = Arg> + use<> {
        child::<ArgList>(&self.syntax).into_iter().flat_map(|list| children(&list.syntax))
    }
}

ast_node!(
    /// The `( )` of a call or an attribute.
    ArgList, ARG_LIST
);

impl ArgList {
    /// The arguments.
    pub fn args(&self) -> impl Iterator<Item = Arg> + use<> {
        children(&self.syntax)
    }
}

ast_node!(
    /// One argument: `name: value`, `mut place`, or a value.
    Arg, ARG
);

impl Arg {
    /// The argument's name, for a named argument.
    pub fn name(&self) -> Option<SyntaxToken> {
        let mut tokens = tokens(&self.syntax);
        let first = tokens.next()?;
        (first.kind().is_ident() && tokens.next().is_some_and(|t| matches!(t.kind(), COLON | EQ)))
            .then_some(first)
    }

    /// Whether this is a `mut` argument (the function may change the place).
    pub fn is_mut(&self) -> bool {
        token(&self.syntax, MUT_KW).is_some()
    }

    /// The value.
    pub fn value(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `receiver.name` or `receiver?.name`.
    MemberExpr, MEMBER_EXPR
);

impl MemberExpr {
    /// The value before the `.`.
    pub fn receiver(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    /// The member's name (a keyword is a name here).
    pub fn name(&self) -> Option<SyntaxToken> {
        name_after(&self.syntax, DOT).or_else(|| name_after(&self.syntax, QUESTION_DOT))
    }

    /// Whether this is `?.`.
    pub fn is_optional(&self) -> bool {
        token(&self.syntax, QUESTION_DOT).is_some()
    }

    /// The generic arguments after the name.
    pub fn generic_args(&self) -> Option<GenericArgList> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `receiver[index]`.
    IndexExpr, INDEX_EXPR
);

impl IndexExpr {
    /// The value indexed.
    pub fn receiver(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    /// The index.
    pub fn index(&self) -> Option<Expr> {
        children::<Expr>(&self.syntax).nth(1)
    }
}

ast_node!(
    /// `params => body`.
    LambdaExpr, LAMBDA_EXPR
);

impl LambdaExpr {
    /// The parameters.
    pub fn params(&self) -> Vec<LambdaParam> {
        match child::<LambdaParamList>(&self.syntax) {
            Some(list) => children(&list.syntax).collect(),
            None => children(&self.syntax).collect(),
        }
    }

    /// The body.
    pub fn body(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

ast_node!(
    /// The `( )` of a lambda's parameters.
    LambdaParamList, LAMBDA_PARAM_LIST
);

ast_node!(
    /// One lambda parameter.
    LambdaParam, LAMBDA_PARAM
);

impl LambdaParam {
    /// The name (`None` for `_`).
    pub fn name(&self) -> Option<SyntaxToken> {
        first_ident(&self.syntax)
    }

    /// Whether the lambda may change this parameter (`mut x =>`).
    pub fn is_mut(&self) -> bool {
        token(&self.syntax, MUT_KW).is_some()
    }

    /// The type annotation.
    pub fn ty(&self) -> Option<Type> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `return` with an optional value.
    ReturnExpr, RETURN_EXPR
);

impl ReturnExpr {
    /// The value returned.
    pub fn value(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `throw value`.
    ThrowExpr, THROW_EXPR
);

impl ThrowExpr {
    /// The error thrown.
    pub fn value(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `break`.
    BreakExpr, BREAK_EXPR
);

ast_node!(
    /// `continue`.
    ContinueExpr, CONTINUE_EXPR
);

ast_node!(
    /// `<Type, ...>` in an expression or a type.
    GenericArgList, GENERIC_ARG_LIST
);

impl GenericArgList {
    /// The type arguments.
    pub fn types(&self) -> impl Iterator<Item = Type> + use<> {
        children(&self.syntax)
    }
}

// ----------------------------------------------------------------------
// Types (grammar.md, 7)
// ----------------------------------------------------------------------

ast_enum!(
    /// A type.
    Type {
        Path(PathType),
        Any(AnyType),
        TypeType(TypeType),
        Optional(OptionalType),
        Paren(ParenType),
        Tuple(TupleType),
        Fn(FnType),
        Mut(MutType),
    }
);

ast_node!(
    /// A type name or path, with optional generic arguments.
    PathType, PATH_TYPE
);

impl PathType {
    /// The names of the path: `["http", "Request"]`.
    pub fn segments(&self) -> Vec<SyntaxToken> {
        tokens(&self.syntax).filter(|token| token.kind().is_ident()).collect()
    }

    /// The generic arguments.
    pub fn generic_args(&self) -> Option<GenericArgList> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `any Trait`.
    AnyType, ANY_TYPE
);

impl AnyType {
    /// The trait.
    pub fn trait_type(&self) -> Option<PathType> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `type`, the type of types.
    TypeType, TYPE_TYPE
);

ast_node!(
    /// `Type?`.
    OptionalType, OPTIONAL_TYPE
);

impl OptionalType {
    /// The type that might be missing.
    pub fn inner(&self) -> Option<Type> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `(Type)`.
    ParenType, PAREN_TYPE
);

impl ParenType {
    /// The type inside.
    pub fn inner(&self) -> Option<Type> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `(Int, String)`.
    TupleType, TUPLE_TYPE
);

impl TupleType {
    /// The element types.
    pub fn elements(&self) -> impl Iterator<Item = Type> + use<> {
        children(&self.syntax)
    }
}

ast_node!(
    /// `(Int) -> Int`.
    FnType, FN_TYPE
);

impl FnType {
    /// The parameter types.
    pub fn params(&self) -> Vec<Type> {
        let arrow = token(&self.syntax, THIN_ARROW).map(|token| token.text_range().start());
        children::<Type>(&self.syntax)
            .filter(|ty| arrow.is_none_or(|arrow| ty.syntax().text_range().start() < arrow))
            .collect()
    }

    /// The result type.
    pub fn result(&self) -> Option<Type> {
        let arrow = token(&self.syntax, THIN_ARROW)?.text_range().start();
        children::<Type>(&self.syntax).find(|ty| ty.syntax().text_range().start() > arrow)
    }
}

ast_node!(
    /// `mut Type`, in a parameter's type.
    MutType, MUT_TYPE
);

impl MutType {
    /// The type after `mut`.
    pub fn inner(&self) -> Option<Type> {
        child(&self.syntax)
    }
}

// ----------------------------------------------------------------------
// Patterns (grammar.md, 8)
// ----------------------------------------------------------------------

ast_enum!(
    /// A pattern.
    Pattern {
        Wildcard(WildcardPat),
        Bind(BindPat),
        Literal(LiteralPat),
        Range(RangePat),
        Option(OptionPat),
        Paren(ParenPat),
        Tuple(TuplePat),
        List(ListPat),
        Or(OrPat),
    }
);

ast_node!(
    /// `_`.
    WildcardPat, WILDCARD_PAT
);

ast_node!(
    /// A new name.
    BindPat, BIND_PAT
);

impl BindPat {
    /// The name.
    pub fn name(&self) -> Option<SyntaxToken> {
        first_ident(&self.syntax)
    }
}

ast_node!(
    /// A literal pattern: `6`, `-1`, `"add"`, `'a'`, `true`, `none`.
    LiteralPat, LITERAL_PAT
);

impl LiteralPat {
    /// Whether the number is negative (written with `-`).
    pub fn is_negative(&self) -> bool {
        token(&self.syntax, MINUS).is_some()
    }

    /// The literal's token (`None` for a string, see [`LiteralPat::string`]).
    pub fn token(&self) -> Option<SyntaxToken> {
        tokens(&self.syntax).find(|token| token.kind() != MINUS)
    }

    /// The string of a string pattern.
    pub fn string(&self) -> Option<StringLiteral> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `1..=5`.
    RangePat, RANGE_PAT
);

impl RangePat {
    /// The lower bound.
    pub fn start(&self) -> Option<LiteralPat> {
        child(&self.syntax)
    }

    /// The upper bound.
    pub fn end(&self) -> Option<LiteralPat> {
        children(&self.syntax).nth(1)
    }

    /// Whether the end is included (`..=`).
    pub fn is_inclusive(&self) -> bool {
        token(&self.syntax, DOT_DOT_EQ).is_some()
    }
}

ast_node!(
    /// An enum option, bare or qualified, with an optional payload: `Rainy(mm)`.
    OptionPat, OPTION_PAT
);

/// One entry of an option pattern's payload.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PayloadItem {
    /// A pattern matched by position.
    Positional(Pattern),
    /// `field: pattern`.
    Named(PayloadField),
}

impl OptionPat {
    /// The names of the path: `["Weather", "Rainy"]`.
    pub fn path(&self) -> Vec<SyntaxToken> {
        tokens(&self.syntax).filter(|token| token.kind().is_ident()).collect()
    }

    /// Whether the option has `( )`.
    pub fn has_payload(&self) -> bool {
        child::<PayloadList>(&self.syntax).is_some()
    }

    /// The payload entries.
    pub fn payload(&self) -> Vec<PayloadItem> {
        let Some(list) = child::<PayloadList>(&self.syntax) else { return Vec::new() };
        list.syntax
            .children()
            .filter_map(|node| match PayloadField::cast(node.clone()) {
                Some(field) => Some(PayloadItem::Named(field)),
                None => Pattern::cast(node).map(PayloadItem::Positional),
            })
            .collect()
    }
}

ast_node!(
    /// The `( )` of an option pattern.
    PayloadList, PAYLOAD_LIST
);

ast_node!(
    /// `field: pattern` in an option pattern.
    PayloadField, PAYLOAD_FIELD
);

impl PayloadField {
    /// The field's name.
    pub fn name(&self) -> Option<SyntaxToken> {
        first_ident(&self.syntax)
    }

    /// The pattern.
    pub fn pattern(&self) -> Option<Pattern> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `(pattern)`.
    ParenPat, PAREN_PAT
);

impl ParenPat {
    /// The pattern inside.
    pub fn inner(&self) -> Option<Pattern> {
        child(&self.syntax)
    }
}

ast_node!(
    /// `(a, b)`.
    TuplePat, TUPLE_PAT
);

impl TuplePat {
    /// The element patterns.
    pub fn elements(&self) -> impl Iterator<Item = Pattern> + use<> {
        children(&self.syntax)
    }
}

ast_node!(
    /// `[a, b]`.
    ListPat, LIST_PAT
);

impl ListPat {
    /// The element patterns.
    pub fn elements(&self) -> impl Iterator<Item = Pattern> + use<> {
        children(&self.syntax)
    }
}

ast_node!(
    /// `a or b`.
    OrPat, OR_PAT
);

impl OrPat {
    /// The alternatives.
    pub fn alternatives(&self) -> impl Iterator<Item = Pattern> + use<> {
        children(&self.syntax)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileRole, parse};

    fn file(text: &str) -> SourceFile {
        let parse = parse(text, FileRole::Entry);
        assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
        parse.tree()
    }

    fn expr(text: &str) -> Expr {
        let file = file(text);
        let Some(Stmt::Expr(statement)) = file.statements().next() else { panic!("{text}") };
        statement.expr().expect("an expression")
    }

    #[test]
    fn items() {
        let file = file(
            "/// Docs.\n@derive(Json)\npub struct Point<T>: Show and Compare {\n    pub x: Float = 0.0\n    fn len() -> Float = 1.0\n}\n\
             enum Weather { Sunny, Rainy(mm: Float) }\n\
             trait Shape {\n    fn area() -> Float\n}\n\
             extend Int: Show { fn show() -> String = \"\" }\n\
             import std.math.{pi, sqrt}\nimport c \"sqlite3.h\"\n\
             test \"works\" { assert true }\n\
             comptime { }\ncomptime let squares = 1\nvar total: Int = 0\n",
        );
        let items: Vec<Item> = file.items().collect();
        assert_eq!(items.len(), 10);
        let Item::Struct(point) = &items[0] else { panic!() };
        assert_eq!(point.name().unwrap().text(), "Point");
        assert_eq!(point.doc_comments().len(), 1);
        assert_eq!(point.attributes()[0].path()[0].text(), "derive");
        assert!(point.is_pub());
        assert_eq!(point.generic_params().unwrap().params().count(), 1);
        assert_eq!(point.conformances().count(), 2);
        let field = point.fields().next().unwrap();
        assert_eq!(field.name().unwrap().text(), "x");
        assert!(field.is_pub() && field.default_value().is_some());
        assert!(matches!(field.ty(), Some(Type::Path(_))));
        let method = point.methods().next().unwrap();
        assert_eq!(method.name().unwrap().text(), "len");
        assert!(matches!(method.body(), Some(FnBody::Expr(_))));
        let Item::Enum(weather) = &items[1] else { panic!() };
        let options: Vec<EnumOption> = weather.options().collect();
        assert_eq!(options.len(), 2);
        assert_eq!(options[1].fields().next().unwrap().name().unwrap().text(), "mm");
        let Item::Trait(shape) = &items[2] else { panic!() };
        assert!(shape.methods().next().unwrap().body().is_none());
        let Item::Extend(extend) = &items[3] else { panic!() };
        assert!(matches!(extend.ty(), Some(Type::Path(_))));
        let Item::Import(import) = &items[4] else { panic!() };
        assert_eq!(import.path().len(), 2);
        assert_eq!(import.names().unwrap().len(), 2);
        let Item::Import(c) = &items[5] else { panic!() };
        assert_eq!(c.c_header().unwrap().plain_value().as_deref(), Some("sqlite3.h"));
        let Item::Test(test) = &items[6] else { panic!() };
        assert_eq!(test.name().unwrap().plain_value().as_deref(), Some("works"));
        assert!(matches!(test.body().unwrap().statements().next(), Some(Stmt::Assert(_))));
        assert!(matches!(items[7], Item::Comptime(_)));
        let Item::GlobalLet(squares) = &items[8] else { panic!() };
        assert!(squares.is_comptime() && !squares.is_var());
        let Item::GlobalLet(total) = &items[9] else { panic!() };
        assert!(total.is_var() && total.ty().is_some() && total.value().is_some());
    }

    #[test]
    fn functions() {
        let file = file(
            "fn f<T: Compare>(a: Int, b: mut List<T> = []) -> Int throws AgeError {\n    a\n}\n",
        );
        let Some(Item::Fn(f)) = file.items().next() else { panic!() };
        let params: Vec<Param> = f.params().collect();
        assert_eq!(params[1].name().unwrap().text(), "b");
        assert!(matches!(params[1].ty(), Some(Type::Mut(_))));
        assert!(params[1].default_value().is_some());
        assert!(matches!(f.return_type(), Some(Type::Path(_))));
        assert!(f.throws().unwrap().error_type().is_some());
        let Some(FnBody::Block(body)) = f.body() else { panic!() };
        assert!(matches!(body.value(), Some(Expr::Name(_))));
    }

    #[test]
    fn expressions() {
        let Expr::Binary(sum) = expr("a + b * c") else { panic!() };
        assert_eq!(sum.op_kind(), Some(PLUS));
        assert!(matches!(sum.rhs(), Some(Expr::Binary(_))));
        let Expr::Call(call) = expr("greet(\"Sam\", greeting: \"Hi\", mut x)") else { panic!() };
        let args: Vec<Arg> = call.args().collect();
        assert_eq!(args[1].name().unwrap().text(), "greeting");
        assert!(args[2].is_mut());
        let Expr::Member(member) = expr("maybe?.parse<Int>") else { panic!() };
        assert!(member.is_optional() && member.generic_args().is_some());
        assert_eq!(member.name().unwrap().text(), "parse");
        let Expr::Catch(catch) = expr("load() catch err { 0 }") else { panic!() };
        assert_eq!(catch.binding().unwrap().text(), "err");
        assert!(catch.handler().is_some() && catch.fallback().is_none());
        let Expr::Catch(catch) = expr("load() catch 0") else { panic!() };
        assert!(catch.binding().is_none() && catch.fallback().is_some());
        let Expr::If(chain) = expr("if a { 1 } else if b { 2 } else { 3 }") else { panic!() };
        assert!(chain.condition().is_some() && chain.then_branch().is_some());
        let Some(ElseBranch::If(inner)) = chain.else_branch() else { panic!() };
        assert!(matches!(inner.else_branch(), Some(ElseBranch::Block(_))));
        let Expr::Match(m) = expr("match x { Rainy(mm) if mm > 1 => 1, _ => 2 }") else { panic!() };
        let arms: Vec<MatchArm> = m.arms().collect();
        assert!(arms[0].guard().is_some() && arms[0].body().is_some());
        let Some(Pattern::Option(option)) = arms[0].pattern() else { panic!() };
        assert_eq!(option.payload().len(), 1);
        let Expr::Lambda(lambda) = expr("(mut a, b: Int) => a + b") else { panic!() };
        let params = lambda.params();
        assert!(params[0].is_mut() && params[1].ty().is_some());
        let Expr::String(string) = expr("\"a {b:.2} c\"") else { panic!() };
        let hole = string.holes().next().unwrap();
        assert_eq!(hole.format_spec().unwrap().text(), ":.2");
        assert!(!string.is_plain());
        let Expr::Map(map) = expr("[\"a\": 1, \"b\": 2]") else { panic!() };
        assert_eq!(map.entries().count(), 2);
        let Expr::Range(range) = expr("1..=10") else { panic!() };
        assert!(range.is_inclusive() && range.end().is_some());
    }

    #[test]
    fn types_and_patterns() {
        let file = file("let f: ((Int, String) -> Bool)? = none\nlet (a, _) = pair\n");
        let mut items = file.items();
        let Some(Item::GlobalLet(f)) = items.next() else { panic!() };
        let Some(Type::Optional(optional)) = f.ty() else { panic!() };
        let Some(Type::Paren(paren)) = optional.inner() else { panic!() };
        let Some(Type::Fn(function)) = paren.inner() else { panic!() };
        assert_eq!(function.params().len(), 2);
        assert!(matches!(function.result(), Some(Type::Path(_))));
        let Some(Item::GlobalLet(pair)) = items.next() else { panic!() };
        let Some(Pattern::Tuple(tuple)) = pair.pattern() else { panic!() };
        assert_eq!(tuple.elements().count(), 2);
    }
}
