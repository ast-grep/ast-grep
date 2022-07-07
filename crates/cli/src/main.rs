use ast_grep_core::AstGrep;
use std::fs::read_to_string;
use std::io::Result;
use clap::Parser;


#[derive(Parser, Debug)]
struct Args {
    #[clap(value_parser)]
    path: String,
    #[clap(short, long, value_parser)]
    expression: String,
    #[clap(short, long, value_parser)]
    rewrite: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let pattern = args.expression;
    let input = read_to_string(&args.path)?;
    let grep = AstGrep::new(input);
    let matches = grep.root().find_all(&*pattern);
    if matches.is_empty() {
        println!("pattern not found!");
        return Ok(());
    }
    println!("{}", args.path);
    if let Some(rewrite) = args.rewrite {
        for mut e in matches {
            println!("------------------");
            println!("{}", e.replace(&pattern, &rewrite).unwrap().inserted_text);
        }
    } else {
        for e in matches {
            println!("------------------");
            println!("{}", e.text());
        }
    }
    Ok(())
}
