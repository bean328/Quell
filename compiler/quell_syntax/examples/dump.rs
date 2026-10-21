// Temporary development tool: parse files and print diagnostics (and the tree with --tree).
fn main() {
    let mut tree = false;
    let mut library = false;
    for arg in std::env::args().skip(1) {
        if arg == "--tree" {
            tree = true;
            continue;
        }
        if arg == "--lib" {
            library = true;
            continue;
        }
        let text = if arg == "-" {
            let mut s = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut s).unwrap();
            s
        } else {
            std::fs::read_to_string(&arg).unwrap()
        };
        let role =
            if library { quell_syntax::FileRole::Library } else { quell_syntax::FileRole::Entry };
        let parse = quell_syntax::parse(&text, role);
        if tree {
            print!("{}", parse.debug_tree());
        }
        if std::env::var("SEXP").is_ok() {
            println!("{}", sexp(&parse.syntax()));
        }
        let file = quell_diagnostics::SourceFile::new(arg.clone(), text.clone());
        let rendered =
            quell_diagnostics::render_all(parse.diagnostics(), &file, Default::default());
        if !rendered.is_empty() {
            println!("== {arg}\n{rendered}");
        }
        assert_eq!(parse.syntax().to_string(), text, "lossless");
    }
}

fn sexp(node: &quell_syntax::SyntaxNode) -> String {
    let mut out = String::new();
    let mut first = true;
    for event in node.preorder_with_tokens() {
        match event {
            rowan::WalkEvent::Enter(rowan::NodeOrToken::Node(n)) => {
                if !first {
                    out.push(' ');
                }
                first = false;
                out.push_str(&format!("({:?}", n.kind()));
            }
            rowan::WalkEvent::Leave(rowan::NodeOrToken::Node(_)) => out.push(')'),
            rowan::WalkEvent::Enter(rowan::NodeOrToken::Token(t)) => {
                if t.kind().is_trivia() || t.kind() == quell_syntax::SyntaxKind::EOF {
                    continue;
                }
                out.push(' ');
                out.push_str(format!("{:?}", t.text()).replace("\"", "'").trim_matches('\''));
            }
            _ => {}
        }
    }
    out
}
