use ast_grep_core::AstGrep;
use std::fs::read_to_string;
use std::io::Result;
use clap::Parser;
use std::path::Path;
use ignore::WalkBuilder;


#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
/**
 * TODO: add some description for ast-grep: sg
 * Example:
 * sg -p ""
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

    /// Include hidden files in search
    #[clap(short,long, parse(from_flag))]
    hidden: bool,

    /// The path whose descendent files are to be explored.
    #[clap(value_parser, default_value=".")]
    path: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let pattern = args.pattern;

    for result in WalkBuilder::new(&args.path).hidden(args.hidden).build() {
        match result {
            Ok(entry) => {
                if let Some(file_type) = entry.file_type() {
                    if !file_type.is_file() {
                        continue;
                    }
                    let path = entry.path();
                    let file_content = read_to_string(&path)?;
                    match_one_file(path, file_content, &pattern, args.rewrite.as_ref());
                }
            },
            Err(err) => eprintln!("ERROR: {}", err),

        }
    }
    Ok(())
}


fn match_one_file(path: &Path, file_content: String, pattern: &str, rewrite: Option<&String> ) {
    let grep = AstGrep::new(file_content);
    println!("{}", path.display());
    let mut matches = grep.root().find_all(pattern).peekable();
    if matches.peek().is_none() {
        println!("pattern not found!");
        return
    }
    if let Some(rewrite) = rewrite {
        for mut e in matches {
            println!("------------------");
            println!("{}", e.replace(&pattern, rewrite).unwrap().inserted_text);
        }
    } else {
        for e in matches {
            println!("------------------");
            println!("{}", grep.display_context(e));
        }
    }
}
