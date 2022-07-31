mod guess_language;
mod interaction;

use ansi_term::Color::{Cyan, Green, Red};
use ansi_term::Style;
use ast_grep_core::language::Language;
use ast_grep_core::{Node, Pattern, Matcher};
use clap::Parser;
use guess_language::{SupportLang, file_types, from_extension};
use ignore::{WalkBuilder, WalkParallel, WalkState};
use similar::{ChangeTag, TextDiff};
use std::fmt::Display;
use std::fs::read_to_string;
use std::io::Result;
use std::path::Path;
use std::sync::mpsc;

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
    if !args.interactive {
        let rewrite = args.rewrite.map(|s| Pattern::new(s.as_ref(), lang));
        run_walker(walker, |path| {
            match_one_file(path, lang, &pattern, &rewrite);
        });
    } else {
        let (tx, rx) = mpsc::channel();
        let pat = pattern.clone();
        let l = lang.clone();
        std::thread::spawn(move || {
            walker.run(move || {
                let tx = tx.clone();
                let pattern = pat.clone();
                let lang = l.clone();
                Box::new(move |result| match result {
                    Ok(entry) => {
                        if let Some(file_type) = entry.file_type() {
                            if !file_type.is_file() {
                                return WalkState::Continue;
                            }
                            let path = entry.path();
                            let file_content = match read_to_string(path) {
                                Ok(content) => content,
                                _ => return WalkState::Continue,
                            };
                            let grep = lang.new(file_content);
                            let mut matches = grep.root().find_all(&pattern);
                            if matches.next().is_none() {
                                return WalkState::Continue;
                            }
                            drop(matches);
                            match tx.send((grep, path.to_path_buf())) {
                                Ok(_) => WalkState::Continue,
                                Err(_) => WalkState::Quit,
                            }
                        } else {
                            WalkState::Continue
                        }
                    }
                    Err(err) => {
                        eprintln!("ERROR: {}", err);
                        WalkState::Continue
                    }
                })
            });
        });
        let rewrite = args.rewrite.map(|s| Pattern::new(s.as_ref(), lang));
        while let Ok((grep, path)) = rx.recv() {
            interaction::clear();
            let matches = grep.root().find_all(&pattern);
            print_matches(matches, &path, &pattern, &rewrite);
            interaction::prompt("Confirm", "yn", Some('y'))
                .expect("Error happened during prompt");
        }
    }
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
    Ok(())
}

fn find_default_config() -> String {
    "sgconfig.yml".to_string()
}

fn run_walker(walker: WalkParallel, f: impl Fn(&Path) -> () + Sync) {
    walker.run(|| {
        Box::new(|result| match result {
            Ok(entry) => {
                if let Some(file_type) = entry.file_type() {
                    if !file_type.is_file() {
                        return WalkState::Continue;
                    }
                    let path = entry.path();
                    f(path);
                    WalkState::Continue
                } else {
                    WalkState::Continue
                }
            }
            Err(err) => {
                eprintln!("ERROR: {}", err);
                WalkState::Continue
            }
        })
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

fn print_matches<'a>(
    matches: impl Iterator<Item = Node<'a, SupportLang>>,
    path: &Path,
    pattern: &impl Matcher<SupportLang>,
    rewrite: &Option<Pattern<SupportLang>>,
) {
    let lock = std::io::stdout().lock(); // lock stdout to avoid interleaving output
    println!("{}", Cyan.italic().paint(format!("{}", path.display())));
    if let Some(rewrite) = rewrite {
        // TODO: actual matching happened in stdout lock, optimize it out
        for e in matches {
            let display = e.display_context();
            let old_str = format!(
                "{}{}{}\n",
                display.leading, display.matched, display.trailing
            );
            let new_str = format!(
                "{}{}{}\n",
                display.leading,
                e.replace(pattern, rewrite)
                    .unwrap()
                    .inserted_text,
                display.trailing
            );
            let base_line = display.start_line;
            print_diff(&old_str, &new_str, base_line);
        }
    } else {
        for e in matches {
            let display = e.display_context();
            let leading = display.leading;
            let trailing = display.trailing;
            let matched = display.matched;
            let highlighted = format!("{leading}{matched}{trailing}");
            let lines: Vec<_> = highlighted.lines().collect();
            let mut num = display.start_line;
            let width = (lines.len() + display.start_line)
                .to_string()
                .chars()
                .count();
            print!("{num:>width$}|"); // initial line num
            print_highlight(leading.lines(), Style::new().dimmed(), width, &mut num);
            print_highlight(matched.lines(), Style::new().bold(), width, &mut num);
            print_highlight(trailing.lines(), Style::new().dimmed(), width, &mut num);
            println!(); // end match new line
        }
    }
    drop(lock);
}

fn print_highlight<'a>(
    mut lines: impl Iterator<Item = &'a str>,
    style: Style,
    width: usize,
    num: &mut usize,
) {
    if let Some(line) = lines.next() {
        let line = style.paint(line);
        print!("{line}");
    }
    for line in lines {
        println!();
        *num += 1;
        let line = style.paint(line);
        print!("{num:>width$}|{line}");
    }
}

fn index_display(index: Option<usize>, style: Style) -> impl Display {
    let index_str = match index {
        None => String::from("    "),
        Some(idx) => format!("{:<4}", idx),
    };
    style.paint(index_str)
}

fn print_diff(old: &str, new: &str, base_line: usize) {
    let diff = TextDiff::from_lines(old, new);
    for (idx, group) in diff.grouped_ops(3).iter().enumerate() {
        if idx > 0 {
            println!("{:-^1$}", "-", 80);
        }
        for op in group {
            for change in diff.iter_inline_changes(op) {
                let (sign, s) = match change.tag() {
                    ChangeTag::Delete => ("-", Style::new().fg(Red)),
                    ChangeTag::Insert => ("+", Style::new().fg(Green)),
                    ChangeTag::Equal => (" ", Style::new().dimmed()),
                };
                print!(
                    "{}{}|{}",
                    index_display(change.old_index().map(|i| i + base_line), s),
                    index_display(change.new_index().map(|i| i + base_line), s),
                    s.paint(sign),
                );
                for (emphasized, value) in change.iter_strings_lossy() {
                    if emphasized {
                        print!("{}", s.underline().paint(value));
                    } else {
                        print!("{}", value);
                    }
                }
                if change.missing_newline() {
                    println!();
                }
            }
        }
    }
}
