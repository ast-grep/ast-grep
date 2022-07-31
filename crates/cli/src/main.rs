mod guess_language;
mod interaction;
mod print;

use ast_grep_core::language::Language;
use ast_grep_core::Pattern;
use clap::Parser;
use guess_language::{SupportLang, file_types, from_extension};
use ignore::{WalkBuilder, WalkParallel, WalkState, DirEntry};
use std::fs::read_to_string;
use std::io::Result;
use std::path::Path;
use print::print_matches;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
/**
 * TODO: add some description for ast-grep: sg
 * Example:
 * sg -p ""
 */
struct Args {
    /// AST pattern to match
    #[clap(short, long, requires("lang"))]
    pattern: Option<String>,

    /// String to replace the matched AST node
    #[clap(short, long)]
    rewrite: Option<String>,

    /// A comma-delimited list of file extensions to process.
    #[clap(short, long)]
    extensions: Vec<String>,

    /// The language of the pattern query
    #[clap(short, long)]
    lang: Option<SupportLang>,

    /// Path to ast-grep config, either YAML or folder of YAMLs
    #[clap(short, long, conflicts_with("pattern"))]
    config: Option<String>,

    /// Include hidden files in search
    #[clap(short, long, parse(from_flag))]
    hidden: bool,

    #[clap(short, long, parse(from_flag))]
    interactive: bool,

    /// Print query pattern's tree-sitter AST
    #[clap(long, parse(from_flag))]
    debug_query: bool,

    /// The path whose descendent files are to be explored.
    #[clap(value_parser, default_value = ".")]
    path: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.pattern.is_some() {
        run_with_pattern(args)
    } else {
        run_with_config(args)
    }
}

// Every run will include Search or Replace
// Search or Replace by arguments `pattern` and `rewrite` passed from CLI
fn run_with_pattern(args: Args) -> Result<()> {
    let pattern = args.pattern.unwrap();
    let threads = num_cpus::get().min(12);
    let lang = args.lang.unwrap();
    let pattern = Pattern::new(&pattern, lang);
    if args.debug_query {
        println!("Pattern TreeSitter {:?}", pattern);
    }
    let walker = WalkBuilder::new(&args.path)
        .hidden(args.hidden)
        .threads(threads)
        .types(file_types(&lang))
        .build_parallel();
    let rewrite = args.rewrite.map(|s| Pattern::new(s.as_ref(), lang));
    if !args.interactive {
        run_walker(walker, |path| {
            match_one_file(path, lang, &pattern, &rewrite);
        });
        return Ok(());
    }
    interaction::run_walker_interactive(
        walker,
        |entry| {
            let entry = filter_file(entry)?;
            let path = entry.path();
            let file_content = read_to_string(path).map_err(
                |err| eprintln!("ERROR: {}", err)
            ).ok()?;
            let grep = lang.new(file_content);
            let has_match = grep.root().find(&pattern).is_some();
            has_match.then_some((grep, path.to_path_buf()))
        },
        |(grep, path)| {
            let matches = grep.root().find_all(&pattern);
            print_matches(matches, &path, &pattern, &rewrite);
            interaction::prompt("Confirm", "yn", Some('y'))
                .expect("Error happened during prompt");
        },
    );
    Ok(())
}

fn run_with_config(args: Args) -> Result<()> {
    use ast_grep_config::{from_yaml_string};
    let config_file = args.config.unwrap_or_else(find_default_config);
    let yaml = read_to_string(config_file)?;
    let config = from_yaml_string(&yaml).unwrap();
    let threads = num_cpus::get().min(12);
    let walker = WalkBuilder::new(&args.path)
        .hidden(args.hidden)
        .threads(threads)
        .build_parallel();
    let lang = config.language;
    if !args.interactive {
        run_walker(walker, |path| {
            if from_extension(path).filter(|&n| n == lang).is_none() {
                return;
            }
            let file_content = match read_to_string(&path) {
                Ok(content) => content,
                _ => return,
            };
            let grep = lang.new(file_content);
            let mut matches = grep.root().find_all(&config).peekable();
            if matches.peek().is_none() {
                return;
            }
            print_matches(matches, path, &config, &None);
        });
    } else {
        interaction::run_walker_interactive(
            walker,
            |entry| {
                let entry = filter_file(entry)?;
                let path = entry.path();
                if from_extension(path).filter(|&n| n == lang).is_none() {
                    return None;
                }
                let file_content = read_to_string(path).map_err(
                    |err| eprintln!("ERROR: {}", err)
                ).ok()?;
                let grep = lang.new(file_content);
                let has_match = grep.root().find(&config).is_some();
                has_match.then_some((grep, path.to_path_buf()))
            },
            |(grep, path)| {
                let matches = grep.root().find_all(&config);
                print_matches(matches, &path, &config, &None);
                interaction::prompt("Confirm", "yn", Some('y'))
                    .expect("Error happened during prompt");
            },
        );
    }
    Ok(())
}

fn find_default_config() -> String {
    "sgconfig.yml".to_string()
}

fn filter_file(
    entry: DirEntry,
) -> Option<DirEntry> {
    entry.file_type()?.is_file().then_some(entry)
}

fn run_walker(walker: WalkParallel, f: impl Fn(&Path) -> () + Sync) {
    interaction::run_walker(walker, |entry| {
        filter_file(entry).map(|e| f(e.path()));
        WalkState::Continue
    });
}

fn match_one_file(
    path: &Path,
    lang: SupportLang,
    pattern: &Pattern<SupportLang>,
    rewrite: &Option<Pattern<SupportLang>>,
) {
    let file_content = match read_to_string(&path) {
        Ok(content) => content,
        _ => return,
    };
    let grep = lang.new(file_content);
    let mut matches = grep.root().find_all(pattern).peekable();
    if matches.peek().is_none() {
        return;
    }
    print_matches(matches, path, pattern, rewrite);
}

