fn main() {
    let text = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    let parse = quell_syntax::parse(&text, quell_syntax::FileRole::Entry);
    for d in parse.diagnostics() {
        for h in &d.helps {
            for e in &h.edits {
                println!("{:?} {:?} {:?}", d.code, e.span, e.replacement);
            }
        }
    }
}
