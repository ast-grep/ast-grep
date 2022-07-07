use ast_grep_core::AstGrep;
use std::env::args;
use std::fs::read_to_string;
use std::io::Result;

fn main() -> Result<()> {
    let mut input = String::new();
    let mut pattern = String::new();
    for arg in args().skip(1) {
        if arg.starts_with("-e") {
            pattern = arg
                .split_once('=')
                .expect("invalid command line argument")
                .1
                .to_string();
        } else {
            input = read_to_string(arg)?;
        }
    }
    let grep = AstGrep::new(input);
    let matches = grep.root().find_all(&*pattern);
    if matches.is_empty() {
        println!("pattern not found!");
        return Ok(());
    }
    for e in matches {
        println!("{}", e.text());
    }
    Ok(())
}
