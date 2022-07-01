use std::env::args;
use std::fs::read_to_string;
use std::io::Result;
use semgrep_rs::Semgrep;

fn main() -> Result<()> {
    let mut input = String::new();
    let mut pattern = String::new();
    for arg in args().skip(1) {
        if arg.starts_with("-e") {
            pattern = arg.split('=').skip(1).next().unwrap().to_string();
        } else {
            input = read_to_string(arg)?;
        }
    }
    let grep = Semgrep::new(input);
    if let Some(e) = grep.root().find(&*pattern) {
        println!("pattern found!");
        dbg!(e.text());
    } else {
        println!("pattern not found!");
    }
    Ok(())
}
