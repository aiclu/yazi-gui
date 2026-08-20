//! 验证 syntect 的主题名、语法查找和高亮输出是否正常。

use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

fn main() {
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    println!(
        "theme count: {}\nthemes: {:?}",
        ts.themes.len(),
        ts.themes.keys().collect::<Vec<_>>()
    );
    println!(
        "base16-ocean.dark exists: {}",
        ts.themes.contains_key("base16-ocean.dark")
    );

    let syntax = ss.find_syntax_by_extension("rs");
    println!("rust syntax found: {:?}", syntax.map(|s| s.name.clone()));

    if let (Some(syntax), Some(theme)) = (syntax, ts.themes.get("base16-ocean.dark")) {
        let mut h = HighlightLines::new(syntax, theme);
        let code = "fn main() {\n    let x = 42; // comment\n    println!(\"{}\", x);\n}\n";
        for line in LinesWithEndings::from(code) {
            let ranges = h.highlight_line(line, &ss).unwrap();
            for (style, seg) in ranges {
                if !seg.trim().is_empty() {
                    println!("  {:?} -> {:?}", style.foreground, seg);
                }
            }
        }
    }
}
