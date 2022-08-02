mod guess_language;
mod interaction;
mod print;

use ast_grep_core::language::Language;
use ast_grep_core::{AstGrep, Matcher, Pattern};
use ast_grep_config::{from_yaml_string, Configs};
use clap::Parser;
use guess_language::{file_types, from_extension, SupportLang, config_file_type};
use ignore::{DirEntry, WalkBuilder, WalkParallel, WalkState};
use print::print_matches;
use std::fs::read_to_string;
use std::io::Result;
use std::path::{Path, PathBuf};


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
            match_one_file(path, lang, &pattern, &rewrite)
        });
        return Ok(());
    }
    run_walker_interactive(
        walker,
        |path| filter_file_interactive(path, lang, &pattern),
        |(grep, path)| {
            let matches = grep.root().find_all(&pattern);
            print_matches(matches, &path, &pattern, &rewrite);
            interaction::prompt("Confirm", "yn", Some('y')).expect("Error happened during prompt");
        },
    );
    Ok(())
}

fn run_with_config(args: Args) -> Result<()> {
    let configs = find_config(args.config);
    let threads = num_cpus::get().min(12);
    let walker = WalkBuilder::new(&args.path)
        .hidden(args.hidden)
        .threads(threads)
        .build_parallel();
    if !args.interactive {
        run_walker(walker, |path| {
            for config in &configs.configs {
                let lang = config.language;
                let matcher = config.get_matcher();
                if from_extension(path).filter(|&n| n == lang).is_none() {
                    continue;
                }
                match_one_file(path, lang, &matcher, &None)
            }
        });
    } else {
        run_walker_interactive(
            walker,
            |path| {
                for config in &configs.configs {
                    let lang = config.language;
                    let matcher = config.get_matcher();
                    if from_extension(path).filter(|&n| n == lang).is_none() {
                        continue;
                    }
                    let ret = filter_file_interactive(path, lang, &matcher);
                    if ret.is_some() {
                        return ret;
                    }
                }
                None
            },
            |(grep, path)| {
                for config in &configs.configs {
                    let matcher = config.get_matcher();
                    let matches = grep.root().find_all(&matcher);
                    print_matches(matches, &path, &matcher, &None);
                    interaction::prompt("Confirm", "yn", Some('y'))
                        .expect("Error happened during prompt");
                }
            },
        );
    }
    Ok(())
}

fn find_config(config: Option<String>) -> Configs {
    let config_file_or_dir = config.unwrap_or_else(find_default_config);
    let mut configs = vec![];
    let walker = WalkBuilder::new(&config_file_or_dir)
        .types(config_file_type())
        .build();
    for dir in walker {
        let config_file = dir.unwrap();
        if !config_file.file_type().unwrap().is_file() {
            continue;
        }
        let path = config_file.path();

        let yaml = read_to_string(path).unwrap();
        configs.extend(from_yaml_string(&yaml).unwrap());
    }
    Configs::new(configs)
}

fn find_default_config() -> String {
    "sgconfig.yml".to_string()
}

fn filter_file(entry: DirEntry) -> Option<DirEntry> {
    entry.file_type()?.is_file().then_some(entry)
}

fn run_walker(walker: WalkParallel, f: impl Fn(&Path) -> () + Sync) {
    interaction::run_walker(walker, |entry| {
        filter_file(entry).map(|e| f(e.path()));
        WalkState::Continue
    });
}

fn run_walker_interactive<T: Send>(
    walker: WalkParallel,
    producer: impl Fn(&Path) -> Option<T> + Sync,
    consumer: impl Fn(T) -> () + Send,
) {
    interaction::run_walker_interactive(
        walker,
        |entry| producer(filter_file(entry)?.path()),
        consumer,
    );
}

fn match_one_file(
    path: &Path,
    lang: SupportLang,
    pattern: &impl Matcher<SupportLang>,
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

fn filter_file_interactive(
    path: &Path,
    lang: SupportLang,
    pattern: &impl Matcher<SupportLang>,
) -> Option<(AstGrep<SupportLang>, PathBuf)> {
    let file_content = read_to_string(path)
        .map_err(|err| eprintln!("ERROR: {}", err))
        .ok()?;
    let grep = lang.new(file_content);
    let has_match = grep.root().find(&pattern).is_some();
    has_match.then_some((grep, path.to_path_buf()))
}
