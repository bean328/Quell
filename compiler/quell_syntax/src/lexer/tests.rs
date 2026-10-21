//! Tests for the lexer. The worked examples of spec/lexical.md, section 12.6 are in
//! `worked_examples`; every lexical error is in `every_error`.

use super::*;
use crate::SyntaxKind::*;

// ----------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------

/// A token's text with line breaks and tabs made visible.
fn show(text: &str) -> String {
    text.replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t")
}

/// The tokens the grammar reads, without `EOF`, as `KIND(text)` separated by spaces. Tokens
/// whose text is fixed by their kind are shown by their text alone.
fn toks(text: &str) -> String {
    let lexed = lex(text);
    check_invariants(&lexed);
    let mut out = Vec::new();
    for index in lexed.non_trivia() {
        let kind = lexed.kind(index);
        if kind == EOF {
            continue;
        }
        let text = lexed.text_of(index);
        if kind.fixed_text() == Some(text) && !kind.is_string_piece() {
            out.push(text.to_string());
        } else {
            out.push(format!("{kind:?}({})", show(text)));
        }
    }
    out.join(" ")
}

/// Every token, trivia included, as `KIND(text)`.
fn all_toks(text: &str) -> String {
    let lexed = lex(text);
    check_invariants(&lexed);
    (0..lexed.len())
        .map(|index| format!("{:?}({})", lexed.kind(index), show(lexed.text_of(index))))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The codes of the diagnostics for `text`, in order.
fn codes(text: &str) -> Vec<&'static str> {
    let lexed = lex(text);
    check_invariants(&lexed);
    lexed.diagnostics().iter().map(|d| d.code.unwrap_or("none")).collect()
}

/// Checks everything that must hold for every input:
/// - the tokens give back the text, start at 0, and never go backwards;
/// - the last token, and only the last, is an empty `EOF`;
/// - partners are symmetric and match kinds;
/// - every diagnostic has a code, a primary label and a fix, with spans inside the text.
fn check_invariants(lexed: &Lexed<'_>) {
    let text = lexed.text();
    let tokens = lexed.tokens();
    assert!(!tokens.is_empty());
    assert_eq!(tokens[0].start, 0, "the first token must start at 0");
    let joined: String = (0..lexed.len()).map(|index| lexed.text_of(index)).collect();
    assert_eq!(joined, text, "lexing must be lossless");
    for pair in tokens.windows(2) {
        assert!(pair[0].start <= pair[1].start, "tokens go backwards: {pair:?}");
    }
    let last = tokens.len() - 1;
    assert_eq!(tokens[last].kind, EOF);
    assert_eq!(tokens[last].start as usize, text.len());
    assert!(tokens[..last].iter().all(|t| t.kind != EOF), "EOF before the end");
    for (index, token) in tokens.iter().enumerate() {
        assert!(text.is_char_boundary(token.start as usize), "token inside a character");
        if token.kind.is_trivia() {
            assert!(!token.line_break_before, "trivia never has the line-break flag");
        }
        if let Some(partner) = lexed.partner(index) {
            assert_eq!(lexed.partner(partner), Some(index), "partners must be symmetric");
            let (open, close) = if index < partner { (index, partner) } else { (partner, index) };
            assert_eq!(
                lexed.kind(open).closing_partner(),
                Some(lexed.kind(close)),
                "partner kinds must match"
            );
        }
        if matches!(token.kind, STRING_START | HOLE_START) {
            assert!(lexed.partner(index).is_some(), "every string and hole is closed");
        }
    }
    for diagnostic in lexed.diagnostics() {
        let code = diagnostic.code.expect("every diagnostic has a code");
        assert!(code.starts_with("Q00"), "lexer codes are Q0001 to Q0099: {code}");
        assert!(diagnostic.labels.iter().any(|label| label.primary), "no primary label");
        assert!(!diagnostic.helps.is_empty(), "every diagnostic says how to fix it: {code}");
        let spans = diagnostic
            .labels
            .iter()
            .map(|label| label.span)
            .chain(diagnostic.helps.iter().flat_map(|help| help.edits.iter().map(|e| e.span)));
        for span in spans {
            assert!(span.start <= span.end && span.end as usize <= text.len(), "{code}");
            assert!(text.is_char_boundary(span.start as usize), "{code}");
            assert!(text.is_char_boundary(span.end as usize), "{code}");
        }
        let words = std::iter::once(&diagnostic.message)
            .chain(&diagnostic.notes)
            .chain(diagnostic.helps.iter().map(|help| &help.message))
            .chain(diagnostic.labels.iter().map(|label| &label.message));
        for words in words {
            assert!(!words.contains(['\u{2013}', '\u{2014}']), "no dashes in messages: {words}");
        }
    }
}

/// `text` with the edits of the first help that has edits applied.
fn apply_first_fix(text: &str, diagnostic: &Diagnostic) -> Option<String> {
    let help = diagnostic.helps.iter().find(|help| !help.edits.is_empty())?;
    let mut edits = help.edits.clone();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.span.start));
    let mut fixed = text.to_string();
    for edit in edits {
        fixed.replace_range(edit.span.start as usize..edit.span.end as usize, &edit.replacement);
    }
    Some(fixed)
}

/// A small deterministic random number generator (xorshift), so the robustness tests need no
/// dependency and always test the same inputs.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Every `.ql` file under examples/, with its path.
fn example_files() -> Vec<(String, String)> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples");
    let mut dirs = vec![std::path::PathBuf::from(root)];
    let mut files = Vec::new();
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).expect("the examples folder exists") {
            let path = entry.expect("a readable entry").path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|ext| ext == "ql") {
                let text = std::fs::read_to_string(&path).expect("a readable example");
                files.push((path.display().to_string(), text));
            }
        }
    }
    files.sort();
    files
}

// ----------------------------------------------------------------------
// The examples
// ----------------------------------------------------------------------

#[test]
fn every_example_lexes_with_no_diagnostics() {
    let files = example_files();
    assert_eq!(files.len(), 31, "spec/README.md says there are 31 .ql files");
    for (path, text) in &files {
        let lexed = lex(text);
        check_invariants(&lexed);
        let file = quell_diagnostics::SourceFile::new(path.clone(), text.clone());
        assert!(
            lexed.diagnostics().is_empty(),
            "{}",
            quell_diagnostics::render_all(lexed.diagnostics(), &file, Default::default())
        );
        for example in doc_examples(&lexed) {
            let inner = lex(&example.text);
            check_invariants(&inner);
            assert!(inner.diagnostics().is_empty(), "doc example in {path}: {:?}", example.text);
        }
    }
}

#[test]
fn examples_with_crlf_line_endings_lex_the_same() {
    for (path, text) in example_files() {
        let crlf = text.replace('\n', "\r\n");
        let lf_lexed = lex(&text);
        let crlf_lexed = lex(&crlf);
        check_invariants(&crlf_lexed);
        assert!(crlf_lexed.diagnostics().is_empty(), "{path}");
        let kinds = |lexed: &Lexed<'_>| lexed.tokens().iter().map(|t| t.kind).collect::<Vec<_>>();
        assert_eq!(kinds(&lf_lexed), kinds(&crlf_lexed), "{path}");
    }
}

// ----------------------------------------------------------------------
// Token classes
// ----------------------------------------------------------------------

#[test]
fn trivia_and_comments() {
    assert_eq!(
        all_toks("a \t b // note\n/// doc\n//// not doc\n/* x /* y */ z */c"),
        "LOWER_IDENT(a) WHITESPACE( \\t ) LOWER_IDENT(b) WHITESPACE( ) LINE_COMMENT(// note) \
         NEWLINE(\\n) DOC_COMMENT(/// doc) LINE_BREAK(\\n) LINE_COMMENT(//// not doc) \
         LINE_BREAK(\\n) BLOCK_COMMENT(/* x /* y */ z */) LOWER_IDENT(c) EOF()"
    );
    // A byte order mark at the very start is trivia; CR LF is one line break.
    assert_eq!(
        all_toks("\u{FEFF}a\r\nb"),
        "BYTE_ORDER_MARK(\u{FEFF}) LOWER_IDENT(a) NEWLINE(\\r\\n) LOWER_IDENT(b) EOF()"
    );
    // Quotes inside a block comment mean nothing.
    assert_eq!(toks("/* \"unclosed ' */ x"), "LOWER_IDENT(x)");
    assert_eq!(all_toks(""), "EOF()");
}

#[test]
fn identifiers() {
    assert_eq!(
        toks("score Player T _unused __ _x1 Self \u{C9}cole \u{E9}cole \u{5929}\u{6C14} \u{1C5}x"),
        "LOWER_IDENT(score) UPPER_IDENT(Player) UPPER_IDENT(T) LOWER_IDENT(_unused) \
         LOWER_IDENT(__) LOWER_IDENT(_x1) UPPER_IDENT(Self) UPPER_IDENT(\u{C9}cole) \
         LOWER_IDENT(\u{E9}cole) LOWER_IDENT(\u{5929}\u{6C14}) UPPER_IDENT(\u{1C5}x)"
    );
    // A lone `_` is the wildcard.
    assert_eq!(toks("_"), "_");
    // The contextual keywords are identifiers.
    assert_eq!(
        toks("any type parallel test c"),
        "LOWER_IDENT(any) LOWER_IDENT(type) LOWER_IDENT(parallel) LOWER_IDENT(test) LOWER_IDENT(c)"
    );
    assert!(is_upper_name("Player") && !is_upper_name("player") && !is_upper_name("_P"));
}

#[test]
fn keywords() {
    let words = [
        "and", "assert", "break", "catch", "comptime", "continue", "defer", "div", "else", "enum",
        "extend", "false", "fn", "for", "group", "if", "import", "in", "is", "let", "match", "mut",
        "none", "not", "or", "pub", "repeat", "return", "self", "spawn", "static", "struct",
        "throw", "throws", "trait", "true", "try", "unsafe", "var", "while",
    ];
    let text = words.join(" ");
    let lexed = lex(&text);
    let kinds: Vec<SyntaxKind> = lexed.non_trivia().map(|i| lexed.kind(i)).collect();
    assert_eq!(kinds.len(), 41);
    for (word, kind) in words.iter().zip(&kinds) {
        assert!(kind.is_keyword(), "{word}");
        assert_eq!(kind.fixed_text(), Some(*word));
    }
    // Keywords are case-sensitive and whole words.
    assert_eq!(
        toks("If iff Self selfish"),
        "UPPER_IDENT(If) LOWER_IDENT(iff) UPPER_IDENT(Self) LOWER_IDENT(selfish)"
    );
    // After `.` and `?.` the lexer still produces the keyword token (the parser reads it as a
    // name, spec 6.1).
    assert_eq!(
        toks("c.div(7, 2) x?.in"),
        "LOWER_IDENT(c) . div ( INT(7) , INT(2) ) LOWER_IDENT(x) ?. in"
    );
}

#[test]
fn numbers() {
    assert_eq!(
        toks("0 42 1_000_000 0xff 0xFF 0b1010 0o17 1.52 0.5 8.0 2.5e-3 1e9 6.02E23 1.05 1e+5"),
        "INT(0) INT(42) INT(1_000_000) INT(0xff) INT(0xFF) INT(0b1010) INT(0o17) FLOAT(1.52) \
         FLOAT(0.5) FLOAT(8.0) FLOAT(2.5e-3) FLOAT(1e9) FLOAT(6.02E23) FLOAT(1.05) FLOAT(1e+5)"
    );
    // The table of spec/lexical.md, 7.3 (the dot rule).
    assert_eq!(toks("1.52"), "FLOAT(1.52)");
    assert_eq!(toks("4.is_even()"), "INT(4) . LOWER_IDENT(is_even) ( )");
    assert_eq!(toks("0..<3"), "INT(0) ..< INT(3)");
    assert_eq!(toks("1..=10"), "INT(1) ..= INT(10)");
    assert_eq!(toks("0.5..<1.5"), "FLOAT(0.5) ..< FLOAT(1.5)");
    assert_eq!(toks("1.5.round()"), "FLOAT(1.5) . LOWER_IDENT(round) ( )");
    assert_eq!(toks("2.e"), "INT(2) . LOWER_IDENT(e)");
    // Number literals are never negative.
    assert_eq!(toks("-5"), "- INT(5)");
    // `_1` is an identifier.
    assert_eq!(toks("_1"), "LOWER_IDENT(_1)");
    // A number can't hold a large value in a token kind, but its text has the exact value.
    let big = "123456789012345678901234567890123456789012345678901234567890";
    assert_eq!(int_value(big).map(|v| v.digits), Some(big.to_string()));
}

#[test]
fn characters() {
    assert_eq!(
        toks(r#"'A' '\u{E9}' '\n' '\'' '\u{1F600}' '{' '\"' '\0' 'é'"#),
        r#"CHAR('A') CHAR('\u{E9}') CHAR('\n') CHAR('\'') CHAR('\u{1F600}') CHAR('{') CHAR('\"') CHAR('\0') CHAR('é')"#
    );
    assert_eq!(char_value("'\u{1F600}'"), Some('\u{1F600}'));
    assert_eq!(char_value(r"'\u{E9}'"), Some('\u{E9}'));
}

#[test]
fn normal_strings() {
    assert_eq!(toks(r#""""#), r#"STRING_START(") STRING_END(")"#);
    assert_eq!(
        toks(r#""Hi, {name}!""#),
        r#"STRING_START(") STRING_TEXT(Hi, ) HOLE_START({) LOWER_IDENT(name) HOLE_END(}) STRING_TEXT(!) STRING_END(")"#
    );
    assert_eq!(
        toks(r#""a\n\t\\\"\'\0\u{E9}b""#),
        r#"STRING_START(") STRING_TEXT(a) STRING_ESCAPE(\n) STRING_ESCAPE(\t) STRING_ESCAPE(\\) STRING_ESCAPE(\") STRING_ESCAPE(\') STRING_ESCAPE(\0) STRING_ESCAPE(\u{E9}) STRING_TEXT(b) STRING_END(")"#
    );
    // `{{` and `}}` are literal braces, checked before holes (spec 9.1).
    assert_eq!(
        toks(r#""{{name}}""#),
        r#"STRING_START(") STRING_ESCAPE({{) STRING_TEXT(name) STRING_ESCAPE(}}) STRING_END(")"#
    );
    // The example of spec 9.3: text, a hole, text, a hole and text.
    assert_eq!(
        toks(r#""{{ if ok { 1 } else { 2 } }}""#),
        r#"STRING_START(") STRING_ESCAPE({{) STRING_TEXT( if ok ) HOLE_START({) INT(1) HOLE_END(}) STRING_TEXT( else ) HOLE_START({) INT(2) HOLE_END(}) STRING_TEXT( ) STRING_ESCAPE(}}) STRING_END(")"#
    );
    // `#` in a normal string is just text.
    assert_eq!(
        toks(r##""#{self.r:02x}""##),
        r#"STRING_START(") STRING_TEXT(#) HOLE_START({) self . LOWER_IDENT(r) FORMAT_SPEC(:02x) HOLE_END(}) STRING_END(")"#
    );
    let lexed = lex(r#""caf\u{E9} {{ok}}\n""#);
    assert_eq!(lexed.plain_string_value(0).as_deref(), Some("caf\u{E9} {ok}\n"));
    assert!(lexed.is_plain_string(0));
    let lexed = lex(r#""a {b}""#);
    assert!(!lexed.is_plain_string(0));
    assert_eq!(lexed.plain_string_value(0), None);
}

#[test]
fn holes_and_format_specs() {
    // The table of spec/lexical.md, 9.3.
    assert_eq!(
        toks(r#""{price:.2}""#),
        r#"STRING_START(") HOLE_START({) LOWER_IDENT(price) FORMAT_SPEC(:.2) HOLE_END(}) STRING_END(")"#
    );
    assert_eq!(
        toks(r#""{self.area():.1}""#),
        r#"STRING_START(") HOLE_START({) self . LOWER_IDENT(area) ( ) FORMAT_SPEC(:.1) HOLE_END(}) STRING_END(")"#
    );
    assert_eq!(
        toks(r#""{greet(name: "Ava")}""#),
        r#"STRING_START(") HOLE_START({) LOWER_IDENT(greet) ( LOWER_IDENT(name) : STRING_START(") STRING_TEXT(Ava) STRING_END(") ) HOLE_END(}) STRING_END(")"#
    );
    assert_eq!(
        toks(r#""{["a": 1]["a"] ?? 0}""#),
        r#"STRING_START(") HOLE_START({) [ STRING_START(") STRING_TEXT(a) STRING_END(") : INT(1) ] [ STRING_START(") STRING_TEXT(a) STRING_END(") ] ?? INT(0) HOLE_END(}) STRING_END(")"#
    );
    // Braces inside a hole count too (spec README, decision 6).
    assert_eq!(
        toks(r#""{match n { 0 => "zero", _ => "many" }}""#),
        r#"STRING_START(") HOLE_START({) match LOWER_IDENT(n) { INT(0) => STRING_START(") STRING_TEXT(zero) STRING_END(") , _ => STRING_START(") STRING_TEXT(many) STRING_END(") } HOLE_END(}) STRING_END(")"#
    );
    // A hole whose expression is a block needs a space after the hole's `{`.
    assert_eq!(
        toks(r#""{ {x} }""#),
        r#"STRING_START(") HOLE_START({) { LOWER_IDENT(x) } HOLE_END(}) STRING_END(")"#
    );
    // A string with holes inside a hole.
    assert_eq!(
        toks(r#""{f("{x}")}""#),
        r#"STRING_START(") HOLE_START({) LOWER_IDENT(f) ( STRING_START(") HOLE_START({) LOWER_IDENT(x) HOLE_END(}) STRING_END(") ) HOLE_END(}) STRING_END(")"#
    );
    // The hole's tokens keep their spaces as trivia, and the partners of a hole match.
    let lexed = lex(r#""{ a + b }""#);
    assert_eq!(lexed.partner(1), Some(lexed.len() - 3));
    assert_eq!(lexed.kind(lexed.len() - 3), HOLE_END);
    assert_eq!(parse_format_spec(".2").and_then(|spec| spec.precision), Some(2));
}

#[test]
fn multi_line_strings() {
    let text = "let poem = \"\"\"\n    Roses are red,\n      Quell is {x}.\n\n    \"\"\"\n";
    assert_eq!(
        toks(text),
        r#"let LOWER_IDENT(poem) = STRING_START("""\n) STRING_INDENT(    ) STRING_TEXT(Roses are red,) STRING_LINE_BREAK(\n) STRING_INDENT(    ) STRING_TEXT(  Quell is ) HOLE_START({) LOWER_IDENT(x) HOLE_END(}) STRING_TEXT(.) STRING_LINE_BREAK(\n) STRING_END(\n    """)"#
    );
    // Example 09: the value has no indentation and no final line break.
    let lexed = lex("let poem = \"\"\"\n    Roses are red,\n    Quell is fast.\n    \"\"\"");
    let start = lexed.non_trivia().find(|&i| lexed.kind(i) == STRING_START).unwrap();
    assert_eq!(lexed.plain_string_value(start).as_deref(), Some("Roses are red,\nQuell is fast."));
    // CR LF in the file becomes `\n` in the value; spaces after `"""` are allowed.
    let lexed = lex("x = \"\"\"  \r\n  a\r\n\r\n  b \r\n  \"\"\"");
    assert!(lexed.diagnostics().is_empty());
    let start = lexed.non_trivia().find(|&i| lexed.kind(i) == STRING_START).unwrap();
    assert_eq!(lexed.plain_string_value(start).as_deref(), Some("a\n\nb "));
    // A blank line that is shorter than the prefix is just an empty line (rule 4); a blank
    // line longer than the prefix keeps the rest.
    let lexed = lex("x = \"\"\"\n    a\n  \n      \n    \"\"\"");
    assert!(lexed.diagnostics().is_empty());
    let start = lexed.non_trivia().find(|&i| lexed.kind(i) == STRING_START).unwrap();
    assert_eq!(lexed.plain_string_value(start).as_deref(), Some("a\n\n  "));
    // Tabs and spaces are matched one by one.
    let lexed = lex("x = \"\"\"\n\t a\n\t \"\"\"");
    assert!(lexed.diagnostics().is_empty());
    // `"` and `""` may appear in the content, and `\"""` is an escaped quote.
    let lexed = lex("x = \"\"\"\n  say \"hi\" or \"\" or \\\"\"\"\n  \"\"\"");
    assert!(lexed.diagnostics().is_empty(), "{:?}", lexed.diagnostics());
    let start = lexed.non_trivia().find(|&i| lexed.kind(i) == STRING_START).unwrap();
    assert_eq!(lexed.plain_string_value(start).as_deref(), Some("say \"hi\" or \"\" or \"\"\""));
    // An empty multi-line string.
    let lexed = lex("x = \"\"\"\n\"\"\"");
    assert!(lexed.diagnostics().is_empty());
    let start = lexed.non_trivia().find(|&i| lexed.kind(i) == STRING_START).unwrap();
    assert_eq!(lexed.plain_string_value(start).as_deref(), Some(""));
}

#[test]
fn raw_strings() {
    assert_eq!(
        toks(r##"#"{"name": "Ava"}"#"##),
        r##"STRING_START(#") STRING_TEXT({"name": "Ava"}) STRING_END("#)"##
    );
    assert_eq!(
        toks(r###"##"a raw string can contain "# like this"##"###),
        r###"STRING_START(##") STRING_TEXT(a raw string can contain "# like this) STRING_END("##)"###
    );
    // No escapes and no holes in a raw string.
    assert_eq!(
        toks(r##"#"\n {x} }"#"##),
        r##"STRING_START(#") STRING_TEXT(\n {x} }) STRING_END("#)"##
    );
    // `#"""` followed by text is a one-line raw string whose text starts with `""`.
    let lexed = lex(r##"#"""hi"""#"##);
    assert!(lexed.diagnostics().is_empty());
    assert_eq!(lexed.plain_string_value(0).as_deref(), Some("\"\"hi\"\""));
    // A multi-line raw string.
    let lexed = lex("x = ##\"\"\"\n    {raw} \\n \"\"\"#\n    \"\"\"##");
    assert!(lexed.diagnostics().is_empty(), "{:?}", lexed.diagnostics());
    let start = lexed.non_trivia().find(|&i| lexed.kind(i) == STRING_START).unwrap();
    assert_eq!(lexed.plain_string_value(start).as_deref(), Some("{raw} \\n \"\"\"#"));
}

#[test]
fn operators_and_punctuation() {
    let all = "+ - * / % == != < <= > >= & | ^ ~ << >> = += -= *= /= %= &= |= ^= <<= >>= \
               ..< ..= . ?. ?? ? => -> @ , : ( ) [ ] { } _";
    let lexed = lex(all);
    check_invariants(&lexed);
    assert!(lexed.diagnostics().is_empty());
    for index in lexed.non_trivia() {
        let kind = lexed.kind(index);
        if kind != EOF {
            assert_eq!(kind.fixed_text(), Some(lexed.text_of(index)), "{kind:?}");
        }
    }
    assert_eq!(lexed.non_trivia().count(), 47);
    // Maximal munch.
    assert_eq!(toks("a<=b"), "LOWER_IDENT(a) <= LOWER_IDENT(b)");
    assert_eq!(toks("x>>=1"), "LOWER_IDENT(x) >>= INT(1)");
    assert_eq!(toks("a<<=b"), "LOWER_IDENT(a) <<= LOWER_IDENT(b)");
    assert_eq!(
        toks("List<List<Int>>"),
        "UPPER_IDENT(List) < UPPER_IDENT(List) < UPPER_IDENT(Int) >>"
    );
    assert_eq!(
        toks("x: List<Int>= []"),
        "LOWER_IDENT(x) : UPPER_IDENT(List) < UPPER_IDENT(Int) >= [ ]"
    );
    assert_eq!(
        toks("a??b?.c?d"),
        "LOWER_IDENT(a) ?? LOWER_IDENT(b) ?. LOWER_IDENT(c) ? LOWER_IDENT(d)"
    );
    // `::`, `++` and `--` are two tokens each; the parser explains them (grammar.md, 11).
    assert_eq!(
        toks("A::B x++ y--"),
        "UPPER_IDENT(A) : : UPPER_IDENT(B) LOWER_IDENT(x) + + LOWER_IDENT(y) - -"
    );
    assert_eq!(toks("a->b=>c"), "LOWER_IDENT(a) -> LOWER_IDENT(b) => LOWER_IDENT(c)");
}

#[test]
fn bracket_partners() {
    // The parser looks past the matching `)` to find a lambda (grammar.md, 6.1).
    let lexed = lex("f((a, b) => a + b, [1, {2}])");
    check_invariants(&lexed);
    let find = |kind: SyntaxKind, nth: usize| {
        (0..lexed.len()).filter(|&i| lexed.kind(i) == kind).nth(nth).unwrap()
    };
    let inner_open = find(L_PAREN, 1);
    let inner_close = lexed.partner(inner_open).unwrap();
    assert_eq!(lexed.kind(lexed.next_non_trivia(inner_close + 1)), FAT_ARROW);
    assert_eq!(lexed.partner(find(L_PAREN, 0)), Some(find(R_PAREN, 1)));
    assert_eq!(lexed.partner(find(L_BRACK, 0)), Some(find(R_BRACK, 0)));
    assert_eq!(lexed.partner(find(L_BRACE, 0)), Some(find(R_BRACE, 0)));
    assert_eq!(lexed.partner(find(COMMA, 0)), None);
    // A closer that doesn't match pops back to its opener (spec 12.2).
    let lexed = lex("{ ( }");
    assert_eq!(lexed.partner(0), Some(4));
    assert_eq!(lexed.partner(2), None);
    assert_eq!(codes("{ ( }"), ["Q0061"]);
}

// ----------------------------------------------------------------------
// The newline rules (spec/lexical.md, section 12)
// ----------------------------------------------------------------------

/// Checks a worked example written as in spec/lexical.md, 12.6: `⏎` after a line break that
/// becomes a `NEWLINE` token and `·` after one that is removed. Lines that hold only spaces
/// get no marker, and neither does a line whose break is inside a token (a multi-line string).
#[track_caller]
fn check_marked(marked: &str) {
    let source = marked.replace(['\u{23CE}', '\u{B7}'], "");
    let lexed = lex(&source);
    check_invariants(&lexed);
    // Only brackets left open at the end of a short example are allowed.
    for diagnostic in lexed.diagnostics() {
        assert_eq!(diagnostic.code, Some("Q0061"), "{}", diagnostic.message);
    }
    let tokens = lexed.tokens();
    let mut rebuilt = String::new();
    let mut line_start = 0;
    for (at, _) in source.match_indices('\n') {
        let break_start = if at > 0 && source.as_bytes()[at - 1] == b'\r' { at - 1 } else { at };
        let line = &source[line_start..break_start];
        let index = tokens.partition_point(|t| t.start as usize <= break_start) - 1;
        let marker = match tokens[index].kind {
            _ if line.trim_matches([' ', '\t']).is_empty() => "",
            NEWLINE => "\u{23CE}",
            LINE_BREAK => "\u{B7}",
            _ => "",
        };
        rebuilt.push_str(line);
        rebuilt.push_str(marker);
        rebuilt.push_str(&source[break_start..at + 1]);
        line_start = at + 1;
    }
    rebuilt.push_str(&source[line_start..]);
    assert_eq!(rebuilt, marked);
}

#[test]
fn worked_examples() {
    // 12.2: the innermost bracket decides.
    check_marked(
        "items.each(x => {·
    print(x)          // a NEWLINE ends this line: the innermost bracket is `{` ⏎
    print(x * 2)⏎
})",
    );
    // A method chain (example 22).
    check_marked(
        "let top_titles = movies·
    .filter(m => m.rating >= 8.0)·
    .sort_by(m => m.rating, descending: true)·
    .map(m => \"{m.title} ({m.year})\")⏎

for title in top_titles {·
    print(title)⏎
}",
    );
    check_marked(
        "let f = x => x * 2·
    .to_string()",
    );
    check_marked(
        "let name = user.nickname ?? user.name·
    .upper()",
    );
    // A multi-line list with a trailing comma (examples 13 and 22).
    check_marked(
        "let movies = [·
    Movie(title: \"Spirited Away\", year: 2001, rating: 8.6),·
    Movie(title: \"Up\", year: 2009, rating: 8.3),·
]⏎
print(movies)",
    );
    // A multi-line call (example 21).
    check_marked(
        "pub fn mix(a: Color, b: Color) -> Color = Color(·
    r: average(a.r, b.r),·
    g: average(a.g, b.g),·
    b: average(a.b, b.b),·
)⏎
next()",
    );
    // An import list split over lines, with and without a trailing comma.
    check_marked(
        "import http.{·
    Server,·
    Request,·
    Response⏎
}⏎
next()",
    );
    check_marked(
        "import http.{·
    Server,·
    Response,·
}⏎
next()",
    );
    // A struct with fields on separate lines and on one line.
    check_marked(
        "struct Movie {·
    title: String⏎
    year: Int, rating: Float⏎
}⏎
next()",
    );
    // A line ending with an operator, with a comment after it.
    check_marked(
        "let total = first_price +       // `+` is in the continue-after set ·
    second_price⏎
next()",
    );
    // The trap: an operator at the start of a line.
    check_marked(
        "let total = first_price⏎
    - discount",
    );
    check_marked(
        "fn score() -> Int {·
    compute_base(x)⏎
        - penalty⏎
}",
    );
    check_marked(
        "fn sign(x: Int) -> Int {·
    if x >= 0 { return 1 }⏎
    -1⏎
}⏎

fn neg_total(xs: List<Int>) -> Int {·
    let total = xs.sum()⏎
    -total⏎
}",
    );
    // `if` and `else` on separate lines.
    check_marked(
        "if temperature > 30 {·
    print(\"It's hot!\")⏎
}·
else {·
    print(\"Stay inside.\")⏎
}⏎
next()",
    );
    // A long condition.
    check_marked(
        "if temperature > 15·
    and not raining {·
    go()⏎
}",
    );
    // Fallbacks on their own line.
    check_marked(
        "let name = find_user(42)·
    ?.name·
    ?? \"guest\"⏎
let n = parse_age(text)·
    catch 0⏎
next()",
    );
    // Generic types and optionals at the end of a line.
    check_marked(
        "struct User {·
    name: String⏎
    email: String?        // `?` is not an operator: the field ends ⏎
    tags: List<String>    // a generic `>`: the field ends ⏎
}⏎
next()",
    );
    // A comparison `>` at the end of a line: the lexer keeps the NEWLINE (12.5).
    check_marked(
        "if score >⏎
    best {",
    );
    // `return` at the end of a line.
    check_marked(
        "if done {·
    return⏎
    cleanup()⏎
}",
    );
    // A keyword used as a member name at the end of a line.
    check_marked(
        "let d = c.div⏎
print(d)⏎
let n = rec.not⏎
-1",
    );
    // Match arms, including a negative number.
    check_marked(
        "match n {·
    0 => \"zero\"⏎
    -1 => \"minus one\"⏎
    1..=9 => \"small\"⏎
    _ => \"big\"⏎
}",
    );
    // A brace on the next line.
    check_marked(
        "if ready⏎
{",
    );
}

#[test]
fn block_comment_that_spans_lines_counts_as_a_line_break() {
    let text = "let a = 1 /* a comment\n             that spans two lines */ let b = 2";
    let lexed = lex(text);
    check_invariants(&lexed);
    let kinds: Vec<SyntaxKind> = lexed.non_trivia().map(|i| lexed.kind(i)).collect();
    assert_eq!(kinds, [LET_KW, LOWER_IDENT, EQ, INT, NEWLINE, LET_KW, LOWER_IDENT, EQ, INT, EOF]);
    let newline = lexed.non_trivia().find(|&i| lexed.kind(i) == NEWLINE).unwrap();
    assert_eq!(lexed.text_of(newline), "");
    assert_eq!(lexed.kind(newline - 1), BLOCK_COMMENT);
    // A block comment on one line counts as whitespace.
    assert_eq!(
        toks("let a = 1 /* note */ let b = 2"),
        "let LOWER_IDENT(a) = INT(1) let LOWER_IDENT(b) = INT(2)"
    );
}

#[test]
fn continue_after_set() {
    // Every token of the continue-after set joins the next line (12.3).
    for op in [
        "+", "-", "*", "/", "%", "div", "==", "!=", "<", "<=", ">=", "&", "|", "^", "~", "<<",
        "and", "or", "not", "..<", "..=", "??", "is", "catch", "try", "spawn", "=", "+=", "-=",
        "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>=", ".", "?.", "=>", "->", ",", "(", "[",
        "{",
    ] {
        let text = format!("a {op}\nb");
        let lexed = lex(&text);
        assert!(lexed.non_trivia().all(|i| lexed.kind(i) != NEWLINE), "{op}");
        let line_break = (0..lexed.len()).find(|&i| lexed.kind(i) == LINE_BREAK);
        assert!(line_break.is_some(), "{op}");
    }
    // These are not in the set: the line ends.
    for op in [">", ">>", "?", ")", "]", "return", "throw", "break", "continue", "x", "1", "\"s\""]
    {
        let text = format!("a {op}\nb");
        let lexed = lex(&text);
        assert!(lexed.non_trivia().any(|i| lexed.kind(i) == NEWLINE), "{op}");
    }
    // A keyword after `.` or `?.` is a member name, never an operator.
    for member in ["div", "is", "and", "or", "not", "catch", "try", "spawn"] {
        for dot in [".", "?."] {
            let text = format!("let d = c{dot}{member}\nprint(d)");
            assert_eq!(
                lex(&text).non_trivia().filter(|&i| lex(&text).kind(i) == NEWLINE).count(),
                1,
                "{dot}{member}"
            );
        }
    }
    // `let text = try` followed by the call on the next line is one statement.
    assert!(!toks("let text = try\n    fs.read_text(path)").contains("NEWLINE"));
    assert!(!toks("let x = a div\n    b").contains("NEWLINE"));
    // The helpful-error tokens act like the words they stand for.
    assert!(!toks("a &&\nb").contains("NEWLINE"));
    assert!(!toks("a ||\nb").contains("NEWLINE"));
    assert!(!toks("x = !\nb").contains("NEWLINE"));
    assert!(!toks("0 ..\n9").contains("NEWLINE"));
}

#[test]
fn continue_before_set() {
    for op in [".", "?.", "else", "catch", "??", "and", "or", "&&", "||"] {
        let text = format!("a\n{op} b");
        assert!(!toks(&text).contains("NEWLINE"), "{op}");
    }
    // Every other token starts a new statement, including `-` and `+` (12.4).
    for op in ["-", "+", "*", "(", "[", "{", "not", "!", "=", ".."] {
        let text = format!("a\n{op} b");
        assert!(toks(&text).contains("NEWLINE"), "{op}");
    }
}

#[test]
fn newline_details() {
    // Several line breaks and comment lines give one NEWLINE, at the first line break.
    assert_eq!(
        all_toks("a\n\n  // c\n\nb"),
        "LOWER_IDENT(a) NEWLINE(\\n) LINE_BREAK(\\n) WHITESPACE(  ) LINE_COMMENT(// c) \
         LINE_BREAK(\\n) LINE_BREAK(\\n) LOWER_IDENT(b) EOF()"
    );
    // A file never starts or ends with NEWLINE.
    assert_eq!(toks("\n\n a \n\n"), "LOWER_IDENT(a)");
    // Inside ( ) and [ ] line breaks are removed, even right before the closer.
    assert_eq!(
        toks("f(a\n, b\n)\n[1\n2\n]"),
        "LOWER_IDENT(f) ( LOWER_IDENT(a) , LOWER_IDENT(b) ) NEWLINE(\\n) [ INT(1) INT(2) ]"
    );
    // A `{` inside ( ) makes line breaks count again until its `}`.
    assert_eq!(
        toks("f({\na\nb\n}\n)"),
        "LOWER_IDENT(f) ( { LOWER_IDENT(a) NEWLINE(\\n) LOWER_IDENT(b) NEWLINE(\\n) } )"
    );
    // A `(` inside { } removes them again.
    assert_eq!(toks("{\n(a\nb)\n}"), "{ ( LOWER_IDENT(a) LOWER_IDENT(b) ) NEWLINE(\\n) }");
    // The line breaks inside a multi-line string are part of its tokens.
    let lexed = lex("x = \"\"\"\n  a\n  \"\"\"\ny");
    let newlines = lexed.non_trivia().filter(|&i| lexed.kind(i) == NEWLINE).count();
    assert_eq!(newlines, 1);
    // `;` counts as a line break (and is reported).
    assert_eq!(toks("a; b"), "LOWER_IDENT(a) NEWLINE() LOWER_IDENT(b)");
    assert_eq!(toks("f(a; b)"), "LOWER_IDENT(f) ( LOWER_IDENT(a) LOWER_IDENT(b) )");
    // The line-break flag is kept inside brackets too, for the `{` rule of 12.6.
    let lexed = lex("(if ready\n{ 1 })");
    assert!(lexed.non_trivia().all(|i| lexed.kind(i) != NEWLINE));
    let brace = (0..lexed.len()).find(|&i| lexed.kind(i) == L_BRACE).unwrap();
    assert!(lexed.line_break_before(brace));
    let ready = (0..lexed.len()).find(|&i| lexed.text_of(i) == "ready").unwrap();
    assert!(!lexed.line_break_before(ready));
    // `>` and `>>` leave their NEWLINE to the parser; `>=` and `>>=` continue.
    assert!(GT.newline_left_to_parser() && GT_GT.newline_left_to_parser());
    assert!(toks("a >>\nb").contains("NEWLINE"));
    assert!(!toks("a >=\nb").contains("NEWLINE"));
    assert!(!toks("a >>=\nb").contains("NEWLINE"));
    // A carriage return on its own is reported and counts as a line break.
    assert_eq!(toks("a\rb"), "LOWER_IDENT(a) NEWLINE(\\r) LOWER_IDENT(b)");
}

// ----------------------------------------------------------------------
// Errors (spec/lexical.md, sections 2 to 11, and 12.2 for brackets)
// ----------------------------------------------------------------------

/// One case for every lexical error: the source and the codes it must produce.
const ERROR_CASES: &[(&str, &[&str])] = &[
    // Source text (section 2)
    ("a \u{FEFF}b", &["Q0002"]),
    ("// \u{FEFF}", &["Q0002"]),
    ("\"\u{FEFF}\"", &["Q0002"]),
    ("a\0b", &["Q0003"]),
    ("// a\0b", &["Q0003"]),
    ("/* \0 */", &["Q0003"]),
    ("\"\0\"", &["Q0003"]),
    ("a\u{1}b", &["Q0004"]),
    ("a\u{7F}b", &["Q0004"]),
    ("\"a\u{1B}b\"", &["Q0035"]),
    ("#\"a\u{1B}b\"#", &["Q0035"]),
    ("'\u{7}'", &["Q0035"]),
    // Whitespace and line breaks (section 3)
    ("a\rb", &["Q0005"]),
    ("// a\rb", &["Q0005"]),
    ("/* a\rb */", &["Q0005"]),
    ("\"a\rb\"", &["Q0005"]),
    ("a\u{A0}b", &["Q0006"]),
    ("a\u{200B}b", &["Q0006"]),
    ("a\u{2028}b", &["Q0006"]),
    ("a\u{3000}b", &["Q0006"]),
    // Comments (section 4)
    ("/* never closed", &["Q0007"]),
    ("/* a /* b */", &["Q0007"]),
    ("/// ```\n/// let x = 1\nfn f() {}", &["Q0008"]),
    ("/// ```text\n/// a", &["Q0008"]),
    // Names (section 5)
    ("let cafe\u{301} = 1", &["Q0009"]),
    // Numbers (section 7)
    ("0X10", &["Q0010"]),
    ("0B1", &["Q0010"]),
    ("007", &["Q0011"]),
    ("00.5", &["Q0011"]),
    ("0x", &["Q0012"]),
    ("0b", &["Q0012"]),
    ("0o", &["Q0012"]),
    ("0b102", &["Q0013"]),
    ("0o8", &["Q0013"]),
    ("1__000", &["Q0014"]),
    ("1_", &["Q0014"]),
    ("0x_ff", &["Q0014"]),
    ("1.5_", &["Q0014"]),
    ("3px", &["Q0015"]),
    ("10ms", &["Q0015"]),
    ("0x1g", &["Q0015"]),
    ("2fast", &["Q0015"]),
    ("1.5e", &["Q0015"]),
    // Characters (section 8)
    ("''", &["Q0020"]),
    ("'ab'", &["Q0021"]),
    ("'e\u{301}'", &["Q0021"]),
    ("'a", &["Q0022"]),
    ("'", &["Q0022"]),
    ("'\n'", &["Q0022", "Q0022"]),
    // Strings (section 9)
    (r#""\q""#, &["Q0030"]),
    ("\"\\", &["Q0034", "Q0030"]),
    (r#""\{""#, &["Q0031"]),
    (r#"'\{'"#, &["Q0031"]),
    (r#""\u{}""#, &["Q0032"]),
    (r#""\uE9""#, &["Q0032"]),
    (r#""\u{E9""#, &["Q0032"]),
    (r#""\u{1234567}""#, &["Q0032"]),
    (r#""\u{xyz}""#, &["Q0032"]),
    (r#""\u{D800}""#, &["Q0033"]),
    (r#""\u{110000}""#, &["Q0033"]),
    ("\"never closed", &["Q0034"]),
    ("\"two\nlines\"", &["Q0034", "Q0034"]),
    ("#\"raw", &["Q0034"]),
    ("x = \"\"\"\n  never closed", &["Q0034"]),
    (r#""a}b""#, &["Q0036"]),
    (r#""{}""#, &["Q0037"]),
    (r#""{ }""#, &["Q0037"]),
    (r#""{:.2}""#, &["Q0038"]),
    (r#"print("Use { to start a block")"#, &["Q0039"]),
    (r#""{x:.2""#, &["Q0039"]),
    (r#""{x // note}""#, &["Q0040"]),
    (r#""{x /* note */}""#, &["Q0040"]),
    (r#""{x:}""#, &["Q0041"]),
    (r#""{x: .2}""#, &["Q0041"]),
    (r#""{x:.2f}""#, &["Q0041"]),
    (r#""{"a": 1}""#, &["Q0041"]),
    ("\"\"\"text\"\"\"", &["Q0042"]),
    ("x = \"\"\"text\n  more\n  \"\"\"", &["Q0042"]),
    ("x = \"\"\"\n  text\"\"\"", &["Q0043"]),
    ("x = \"\"\"\n  a\n b\n  \"\"\"", &["Q0044"]),
    ("x = \"\"\"\n  a\n\tb\n  \"\"\"", &["Q0044"]),
    // Tokens that exist only to give a helpful error (section 11)
    ("a && b", &["Q0050"]),
    ("a || b", &["Q0051"]),
    ("!done", &["Q0052"]),
    ("0..10", &["Q0053"]),
    ("0...10", &["Q0053"]),
    ("x = 1;", &["Q0054"]),
    ("a; b", &["Q0054"]),
    ("a */ b", &["Q0055"]),
    ("# comment", &["Q0056"]),
    ("##x", &["Q0056"]),
    ("$x", &["Q0057"]),
    ("`x`", &["Q0057", "Q0057"]),
    ("a \\ b", &["Q0057"]),
    ("price = \u{20AC}5", &["Q0057"]),
    ("\u{201C}hi\u{201D}", &["Q0058", "Q0058"]),
    ("a \u{2212} b", &["Q0058"]),
    // Brackets (section 12.2)
    ("a)", &["Q0060"]),
    ("]", &["Q0060"]),
    ("\"{x)}\"", &["Q0060"]),
    ("(a", &["Q0061"]),
    ("{ [ }", &["Q0061"]),
    ("\"{f(x}\"", &["Q0061"]),
];

#[test]
fn every_error() {
    for &(text, expected) in ERROR_CASES {
        assert_eq!(codes(text), expected, "{text:?}");
    }
    assert!(decode_source(b"ok").is_ok());
    let error = decode_source(b"let x = \xFF").unwrap_err();
    assert_eq!(error.code, Some("Q0001"));
    assert_eq!(error.main_label().map(|label| label.span), Some(quell_diagnostics::Span::at(8)));
    let error = decode_source(b"\xFF\xFEa\0").unwrap_err();
    assert!(error.notes.iter().any(|note| note.contains("UTF-16")));
}

#[test]
fn many_unclosed_holes_on_one_line() {
    // Each hole that is never closed makes the lexer read the rest of the line again, up to a
    // limit per line. Past it, holes are closed where the line ends, with the same error.
    let text = "\"{".repeat(300);
    let found = codes(&text);
    assert!(found.len() >= 300);
    assert!(found.iter().all(|code| ["Q0034", "Q0039", "Q0061"].contains(code)));
    // The limit is per line: the next line is lexed normally.
    let text = format!("{}\nx = \"{{y}}\"", "\"{".repeat(300));
    let lexed = lex(&text);
    let last_line = text.rfind('\n').unwrap() as u32;
    assert!(lexed.diagnostics().iter().all(|d| d.main_label().unwrap().span.start < last_line));
    let y = (0..lexed.len()).rev().find(|&i| lexed.text_of(i) == "y").unwrap();
    assert_eq!(lexed.kind(y - 1), HOLE_START);
}

#[test]
fn every_code_is_tested_and_documented() {
    let source = include_str!("codes.rs");
    let registry = include_str!("../../../../docs/errors.md");
    let mut tested: Vec<&str> =
        ERROR_CASES.iter().flat_map(|(_, codes)| codes.iter().copied()).collect();
    // Q0001 is tested through `decode_source`, and Q0070 needs a 4 GiB file.
    tested.extend(["Q0001", "Q0070"]);
    let defined: Vec<&str> =
        source.match_indices("\"Q").map(|(at, _)| &source[at + 1..at + 6]).collect();
    assert!(defined.len() >= 40);
    for code in &defined {
        assert!(tested.contains(code), "{code} has no test");
        assert!(registry.contains(&format!("| `{code}` |")), "{code} is not in docs/errors.md");
        assert_eq!(defined.iter().filter(|c| *c == code).count(), 1, "{code} is used twice");
    }
    for code in tested {
        assert!(defined.contains(&code), "{code} is not defined");
    }
}

/// Every lexer error message, rendered as the user sees it. Review changes to the snapshot like
/// code (docs/decisions/0005-front-end-architecture.md).
#[test]
fn error_messages() {
    let mut out = String::new();
    for &(text, _) in ERROR_CASES {
        let lexed = lex(text);
        let file = quell_diagnostics::SourceFile::new("example.ql", text);
        out.push_str(&format!("=== {}\n", text.escape_debug()));
        out.push_str(&quell_diagnostics::render_all(
            lexed.diagnostics(),
            &file,
            Default::default(),
        ));
        out.push('\n');
    }
    // Show invisible characters as Unicode control pictures (one column each, so the markers
    // stay aligned), and keep the snapshot plain text.
    let visible: String = out
        .chars()
        .map(|c| match c {
            '\n' => c,
            '\0'..='\u{1F}' => char::from_u32(0x2400 + c as u32).unwrap_or(c),
            '\u{7F}' => '\u{2421}',
            '\u{FEFF}' | '\u{200B}' | '\u{2028}' => '\u{2422}',
            _ => c,
        })
        .collect();
    insta::assert_snapshot!(visible);
}

#[test]
fn fixes_remove_the_error() {
    // Applying the first fix with edits makes that error go away. Some errors have only advice
    // (a code point that is not a character, a prefix with no digits, ...).
    for &(text, expected) in ERROR_CASES {
        let lexed = lex(text);
        let first = &lexed.diagnostics()[0];
        let Some(fixed) = apply_first_fix(text, first) else { continue };
        let before = expected.iter().filter(|code| Some(**code) == first.code).count();
        let after = codes(&fixed).iter().filter(|code| Some(**code) == first.code).count();
        assert!(after < before, "{text:?} fixed as {fixed:?} still has {:?}", first.code);
    }
}

#[test]
fn recovery() {
    // The helpful-error tokens recover as the token the user meant.
    assert_eq!(AMP_AMP.recovered(), AND_KW);
    assert_eq!(PIPE_PIPE.recovered(), OR_KW);
    assert_eq!(BANG.recovered(), NOT_KW);
    assert_eq!(DOT_DOT.recovered(), DOT_DOT_LT);
    assert_eq!(DOT_DOT_DOT.recovered(), DOT_DOT_LT);
    // Skipped characters are trivia, so the parser sees the code around them.
    assert_eq!(toks("$x + `y"), "LOWER_IDENT(x) + LOWER_IDENT(y)");
    // An unclosed string ends at the end of its line.
    assert_eq!(
        toks("print(\"oops)\nnext()"),
        r#"LOWER_IDENT(print) ( STRING_START(") STRING_TEXT(oops)) STRING_END() LOWER_IDENT(next) ( )"#
    );
    // A hole that is never closed is read again as text, so the string and the call still end
    // where they appear to (spec 9.3).
    assert_eq!(
        toks("print(\"Use { to start a block\")"),
        r#"LOWER_IDENT(print) ( STRING_START(") STRING_TEXT(Use { to start a block) STRING_END(") )"#
    );
    // After an error in a number, the whole number is one token.
    assert_eq!(toks("3px + 1"), "INT(3px) + INT(1)");
    // A string with errors still has its partner.
    let lexed = lex("\"a\\q {}\"");
    assert_eq!(lexed.partner(0), Some(lexed.len() - 2));
}

#[test]
fn messages_follow_the_spec() {
    let message = |text: &str| lex(text).diagnostics()[0].message.clone();
    assert!(message("a && b").contains("the word `and`"));
    assert!(message("a || b").contains("the word `or`"));
    assert!(message("!a").contains("the word `not`"));
    assert_eq!(message("0..9"), "a range must say whether it includes the end");
    assert_eq!(message("a;"), "Quell doesn't use semicolons");
    assert_eq!(message("*/"), "`*/` without a matching `/*`");
    assert_eq!(message("''"), "a character literal needs exactly one character");
    assert_eq!(message("007"), "numbers can't start with `0`");
    assert_eq!(message("3px"), "a number can't be directly followed by a letter");
    assert!(message("print(\"{ oops\")").starts_with("this `{` starts a value inside the string"));
    assert!(message("x = \"\"\"\n  a\n b\n  \"\"\"").starts_with("this line is less indented"));
    assert!(message("a\u{A0}b").contains("non-breaking space"));
    // The fixes are exact edits where the spec gives one.
    let first_fix = |text: &str| apply_first_fix(text, &lex(text).diagnostics()[0]).unwrap();
    assert_eq!(first_fix("a&&b"), "a and b");
    assert_eq!(first_fix("if !done {"), "if not done {");
    assert_eq!(first_fix("0..10"), "0..<10");
    assert_eq!(first_fix("x = 1;"), "x = 1");
    assert_eq!(first_fix("x = 1; y = 2"), "x = 1\ny = 2");
    assert_eq!(first_fix("    x = 1; y = 2"), "    x = 1\n    y = 2");
    assert_eq!(first_fix("007"), "7");
    assert_eq!(first_fix("0X10"), "0x10");
    assert_eq!(first_fix("'ab'"), "\"ab\"");
    assert_eq!(first_fix(r"'{\u{E9}}'"), r#""{{\u{E9}}}""#);
    assert_eq!(first_fix("\"a}b\""), "\"a}}b\"");
    assert_eq!(first_fix("\"{}\""), "\"{{}}\"");
    assert_eq!(first_fix("\"\\{\""), "\"{{\"");
    assert_eq!(first_fix("\"never closed"), "\"never closed\"");
    assert_eq!(first_fix("\"\"\"text\"\"\""), "\"text\"");
    assert_eq!(first_fix("let cafe\u{301} = 1"), "let caf\u{E9} = 1");
    assert_eq!(first_fix("a\u{A0}b"), "a b");
    assert_eq!(first_fix("\u{201C}hi\""), "\"hi\"");
    // The JSON hint of spec 9.3.
    let lexed = lex(r#""{"a": 1}""#);
    assert!(lexed.diagnostics()[0].helps.iter().any(|help| help.message.contains("raw string")));
}

#[test]
fn diagnostics_are_in_source_order() {
    let lexed = lex("(\n$ a && b\n'ab'");
    let starts: Vec<u32> =
        lexed.diagnostics().iter().map(|d| d.main_label().unwrap().span.start).collect();
    let mut sorted = starts.clone();
    sorted.sort();
    assert_eq!(starts, sorted);
    assert_eq!(starts.len(), 4);
}

// ----------------------------------------------------------------------
// The public API
// ----------------------------------------------------------------------

#[test]
fn navigation_helpers() {
    let lexed = lex("a  // c\n  .b");
    let a = 0;
    let dot = lexed.next_non_trivia(a + 1);
    assert_eq!(lexed.kind(dot), DOT);
    assert_eq!(lexed.prev_non_trivia(dot), Some(a));
    assert_eq!(lexed.prev_non_trivia(a), None);
    assert_eq!(lexed.kind(lexed.next_non_trivia(1000)), EOF);
    assert_eq!(lexed.kind(1000), EOF);
    assert_eq!(lexed.indentation_of_line(lexed.tokens()[dot].start), "  ");
    assert_eq!(lexed.indentation_of_line(0), "");
    assert!(!lexed.is_empty() && !lexed.has_errors());
}

#[test]
fn doc_comment_examples() {
    let text = "/// Adds.\n///\n/// ```\n/// assert add(1, 2) == 3\n///assert add(0, 0) == 0\n/// ```\n/// ```text\n/// not code\n/// ```\n/// ```quell\n/// x\n/// ```\nfn add(a: Int, b: Int) -> Int = a + b\n";
    let lexed = lex(text);
    assert!(lexed.diagnostics().is_empty());
    let examples = doc_examples(&lexed);
    assert_eq!(examples.len(), 2);
    assert_eq!(examples[0].text, "assert add(1, 2) == 3\nassert add(0, 0) == 0");
    assert_eq!(examples[1].text, "x");
    // Offsets map back to the file.
    let second_line = examples[0].text.find("assert add(0").unwrap() as u32;
    let in_file = examples[0].file_offset(second_line) as usize;
    assert!(text[in_file..].starts_with("assert add(0, 0)"));
    let first = examples[0].file_offset(0) as usize;
    assert!(text[first..].starts_with("assert add(1, 2)"));
    // A blank line ends a run of doc comments, so its fence is never closed.
    assert_eq!(codes("/// ```\n\n/// ```\nfn f() {}"), ["Q0008", "Q0008"]);
}

// ----------------------------------------------------------------------
// Losslessness and robustness
// ----------------------------------------------------------------------

/// Inputs made to be awkward. Each must lex without panicking and give back its text.
const AWKWARD: &[&str] = &[
    "",
    " ",
    "\n",
    "\r",
    "\r\n",
    "\r\r\n\n\r",
    "\u{FEFF}",
    "\u{FEFF}\u{FEFF}",
    "\0",
    "\t\t\t",
    "\"",
    "\"\"",
    "\"\"\"",
    "\"\"\"\"",
    "\"\"\"\"\"\"",
    "\"\"\"\n",
    "\"\"\"\n\"",
    "\"\"\"\n  \"\"\"\"\"\"",
    "#",
    "#\"",
    "##\"\"#",
    "#\"\"\"",
    "#\"\"\"\n\"\"\"",
    "'",
    "''",
    "'''",
    "'\\",
    "'\\u{",
    "\\",
    "\"\\",
    "\"\\u",
    "\"\\u{",
    "\"\\u{12345678901234567890}\"",
    "{",
    "}",
    "\"{",
    "\"}",
    "\"{{",
    "\"}}",
    "\"{\"",
    "\"{\"}\"",
    "\"{:",
    "\"{x:",
    "\"{x:}",
    "\"{/*",
    "\"{//",
    "\"{\n}\"",
    "\"{\"\"\"\n\"\"\"}\"",
    "\"{#\"",
    "\"{'}'}\"",
    "/*",
    "/*/",
    "/**/",
    "/* /* */",
    "*/",
    "///",
    "/// ```",
    "0x",
    "0b2",
    "1.",
    "1.e",
    "1e",
    "1e+",
    "1__",
    "0_",
    "9999999999999999999999999999999999999999999",
    "..",
    "...",
    "....",
    "&&&",
    "|||",
    "!!",
    ";;;",
    "(((",
    ")))",
    "([{)]}",
    "\u{301}",
    "e\u{301}\u{301}\u{301}",
    "\u{1F600}\u{1F600}",
    "\u{A0}\u{2028}\u{2029}\u{85}",
    "a\u{200D}b",
    "\u{D55C}\u{AE00}",
    "\u{1112}\u{1161}\u{11AB}",
    "x\u{1112}\u{1161}\u{11AB}",
    "\"a\"#",
    "r#\"x\"#",
    "\"{\"{\"{\"{\"{",
    "\"{ (\"{ [\"{ {",
    "\"\"\"\n  {x\n  \"\"\"",
    "x = \"\"\"\n  a {\"\"\"\n  \"\"\"",
];

#[test]
fn lossless_on_awkward_input() {
    for text in AWKWARD {
        let lexed = lex(text);
        check_invariants(&lexed);
    }
    // Every prefix and every suffix of a sample that uses every feature.
    let sample = "\u{FEFF}/// Doc\n/// ```\n/// x\n/// ```\nfn f<T>(a: List<T>) -> String {\r\n    let s = \"a {b:.2} {{c}} \\u{E9}\" + #\"raw\"# + '\\n'\n    let m = \"\"\"\n      x {y}\n      \"\"\"\n    /* a /* b */ */ return s ?? m.div // c\n    0x_1g && !x .. 1__0; $ \u{A0} caf\u{E9}\u{301}\n}\n";
    let boundaries = (0..=sample.len()).filter(|&at| sample.is_char_boundary(at));
    for at in boundaries {
        check_invariants(&lex(&sample[..at]));
        check_invariants(&lex(&sample[at..]));
    }
}

#[test]
fn lossless_on_random_input() {
    // Random text made of the characters that matter most to the lexer.
    let alphabet: Vec<char> =
        "\"'#{}()[]\\:;/*.<>=!&|?-+_ \t\n\r0xebua1Z\u{FEFF}\u{301}\u{A0}\u{E9}$`\0"
            .chars()
            .collect();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..3000 {
        let len = rng.below(40);
        let text: String = (0..len).map(|_| alphabet[rng.below(alphabet.len())]).collect();
        check_invariants(&lex(&text));
    }
}

#[test]
fn broken_examples_never_crash() {
    // Thousands of broken variations of the examples: a piece deleted, duplicated or moved.
    let files = example_files();
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    for round in 0..1500 {
        let (_, text) = &files[round % files.len()];
        let mut mutated = text.clone();
        for _ in 0..1 + rng.below(3) {
            let a = rng.below(mutated.len() + 1);
            let b = (a + rng.below(12)).min(mutated.len());
            if !mutated.is_char_boundary(a) || !mutated.is_char_boundary(b) {
                continue;
            }
            let piece = mutated[a..b].to_string();
            match rng.below(3) {
                0 => mutated.replace_range(a..b, ""),
                1 => mutated.insert_str(a, &piece),
                _ => {
                    mutated.replace_range(a..b, "");
                    let mut at = rng.below(mutated.len() + 1);
                    while !mutated.is_char_boundary(at) {
                        at -= 1;
                    }
                    mutated.insert_str(at, &piece);
                }
            }
        }
        check_invariants(&lex(&mutated));
    }
}

#[test]
fn pathological_input_is_linear() {
    // Each of these is about 200 KB. A quadratic step would take minutes here; the lexer takes
    // a few milliseconds per input.
    let size = 200_000;
    let inputs = [
        "(".repeat(size),
        ")".repeat(size),
        "{".repeat(size),
        "\"{".repeat(size / 2),
        "\"{(".repeat(size / 3),
        "\"{x /*".repeat(size / 6),
        "/*".repeat(size / 2),
        "a;".repeat(size / 2),
        " ;".repeat(size / 2),
        "'a".repeat(size / 2),
        "'".repeat(size),
        "#".repeat(size),
        format!("{}\"{}", "#".repeat(size / 2), "\"#".repeat(size / 4)),
        "\"\"\"\n".repeat(size / 4),
        format!("x = \"\"\"\n{}  \"\"\"", " a\n".repeat(size / 3)),
        "e\u{301}".repeat(size / 3),
        "x\u{301}\u{302}\u{303}\u{304}".repeat(size / 10),
        "1_".repeat(size / 2),
        "\"\\u{".repeat(size / 4),
        "/// ```\n".repeat(size / 8),
        "\"{\"{\"{\"{\n".repeat(size / 9),
        "a.div\n".repeat(size / 6),
    ];
    for text in &inputs {
        let started = std::time::Instant::now();
        let lexed = lex(text);
        assert_eq!(lexed.text().len(), text.len());
        let joined: usize = (0..lexed.len()).map(|index| lexed.text_of(index).len()).sum();
        assert_eq!(joined, text.len());
        // A generous bound that only a quadratic step could break, even in a debug build.
        assert!(started.elapsed().as_secs() < 20, "{:?}...", &text[..20]);
    }
}

#[test]
fn deep_nesting_does_not_overflow_the_stack() {
    let depth = 100_000;
    let nested_strings = "\"{".repeat(depth) + &"}\"".repeat(depth);
    check_invariants(&lex(&nested_strings));
    let nested_brackets = "([{".repeat(depth) + &"}])".repeat(depth);
    let lexed = lex(&nested_brackets);
    check_invariants(&lexed);
    assert!(lexed.diagnostics().is_empty());
    assert_eq!(lexed.partner(0), Some(lexed.len() - 2));
    let nested_comments = "/*".repeat(depth) + &"*/".repeat(depth);
    let lexed = lex(&nested_comments);
    assert!(lexed.diagnostics().is_empty());
    assert_eq!(lexed.len(), 2);
}
