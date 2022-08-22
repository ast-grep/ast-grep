use std::fs::read_to_string;
use std::io::Result;
use std::path::{Path, PathBuf};

use ast_grep_config::{from_yaml_string, Configs, RuleConfig};
use ast_grep_core::language::Language;
use ast_grep_core::{AstGrep, Matcher, Pattern};
use clap::Args;
use ignore::{DirEntry, WalkBuilder, WalkParallel, WalkState};

use crate::guess_language::{config_file_type, file_types, from_extension, SupportLang};
use crate::print::{print_matches, ColorArg, ErrorReporter, ReportStyle, SimpleFile};
use crate::{interaction, Args as PatternArg};

#[derive(Args)]
pub struct ScanArg {
    /// Path to ast-grep config, either YAML or folder of YAMLs
    #[clap(short, long)]
    config: Option<String>,

    /// Include hidden files in search
    #[clap(short, long, parse(from_flag))]
    hidden: bool,

    #[clap(short, long, parse(from_flag))]
    interactive: bool,

    #[clap(long, default_value = "auto")]
    color: ColorArg,

    #[clap(long, default_value = "rich")]
    report_style: ReportStyle,

    /// The path whose descendent files are to be explored.
    #[clap(value_parser, default_value = ".")]
    path: String,
}

// Every run will include Search or Replace
// Search or Replace by arguments `pattern` and `rewrite` passed from CLI
pub fn run_with_pattern(args: PatternArg) -> Result<()> {
    let pattern = args.pattern;
    let threads = num_cpus::get().min(12);
    let lang = args.lang;
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
        |(grep, path)| run_one_interaction(&path, &grep, &pattern, &rewrite),
    );
    Ok(())
}

pub fn run_with_config(args: ScanArg) -> Result<()> {
    let configs = find_config(args.config);
    let threads = num_cpus::get().min(12);
    let walker = WalkBuilder::new(&args.path)
        .hidden(args.hidden)
        .threads(threads)
        .build_parallel();
    let reporter = ErrorReporter::new(args.color.into(), args.report_style);
    if !args.interactive {
        run_walker(walker, |path| {
            for config in &configs.configs {
                let lang = config.language;
                if from_extension(path).filter(|&n| n == lang).is_none() {
                    continue;
                }
                match_rule_on_file(path, lang, config, &reporter)
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
                    if from_extension(&path)
                        .filter(|&n| n == config.language)
                        .is_none()
                    {
                        continue;
                    }
                    let matcher = config.get_matcher();
                    let fixer = config.get_fixer();
                    run_one_interaction(&path, &grep, matcher, &fixer);
                }
            },
        );
    }
    Ok(())
}

fn run_one_interaction<M: Matcher<SupportLang>>(
    path: &PathBuf,
    grep: &AstGrep<SupportLang>,
    matcher: M,
    rewrite: &Option<Pattern<SupportLang>>,
) {
    let mut matches = grep.root().find_all(&matcher).peekable();
    if matches.peek().is_none() {
        return;
    }
    print_matches(matches, path, &matcher, rewrite);
    let rewrite = match rewrite {
        Some(r) => r,
        None => {
            interaction::prompt("Next", "", Some('\n')).unwrap();
            return;
        }
    };
    let response = interaction::prompt("Accept change? (Yes[y], No[n], All[a])", "yna", Some('y'))
        .expect("Error happened during prompt");
    match response {
        'y' => {
            let new_content = apply_rewrite(grep, &matcher, rewrite);
            std::fs::write(&path, new_content).expect("write file content failed");
        }
        'a' => (),
        _ => (),
    }
}

fn apply_rewrite<M: Matcher<SupportLang>>(
    grep: &AstGrep<SupportLang>,
    matcher: M,
    rewrite: &Pattern<SupportLang>,
) -> String {
    let root = grep.root();
    let edits = root.replace_all(matcher, rewrite);
    let mut new_content = String::new();
    let mut start = 0;
    for edit in edits {
        new_content.push_str(&grep.source()[start..edit.position]);
        new_content.push_str(&edit.inserted_text);
        start = edit.position + edit.deleted_length;
    }
    new_content
}

fn find_config(config: Option<String>) -> Configs<SupportLang> {
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

fn run_walker(walker: WalkParallel, f: impl Fn(&Path) + Sync) {
    interaction::run_walker(walker, |entry| {
        if let Some(e) = filter_file(entry) {
            f(e.path());
        }
        WalkState::Continue
    });
}

fn run_walker_interactive<T: Send>(
    walker: WalkParallel,
    producer: impl Fn(&Path) -> Option<T> + Sync,
    consumer: impl Fn(T) + Send,
) {
    interaction::run_walker_interactive(
        walker,
        |entry| producer(filter_file(entry)?.path()),
        consumer,
    );
}

fn match_rule_on_file(
    path: &Path,
    lang: SupportLang,
    rule: &RuleConfig<SupportLang>,
    reporter: &ErrorReporter,
) {
    let matcher = rule.get_matcher();
    let file_content = match read_to_string(&path) {
        Ok(content) => content,
        _ => return,
    };
    let grep = lang.ast_grep(&file_content);
    let mut matches = grep.root().find_all(matcher).peekable();
    if matches.peek().is_none() {
        return;
    }
    let file = SimpleFile::new(path.to_string_lossy(), &file_content);
    reporter.print_rule(matches, file, rule);
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
    let grep = lang.ast_grep(file_content);
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
    let grep = lang.ast_grep(file_content);
    let has_match = grep.root().find(&pattern).is_some();
    has_match.then_some((grep, path.to_path_buf()))
}
