use ast_grep_core::AstGrep;
use std::fs::read_to_string;
use std::io::Result;
use clap::Parser;


#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
/**
 * TODO: add some description for ast-grep: sg
 * Example:
 * sg -e
 */
struct Args {
    /// AST pattern to match
    #[clap(short,long,value_parser)]
    pattern: String,

    /// String to replace the matched AST node
    #[clap(short, long, value_parser)]
    rewrite: Option<String>,

    /// A comma-delimited list of file extensions to process.
    #[clap(short, long)]
    extensions: Vec<String>,

    /// The path whose descendent files are to be explored.
    #[clap(value_parser, default_value=".")]
    path: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let pattern = args.pattern;
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
