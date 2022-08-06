use std::fmt::Display;
use std::path::Path;
use std::borrow::Cow;

use ansi_term::{
    Color::{Cyan, Green, Red},
    Style,
};
use codespan_reporting::diagnostic::{self, Diagnostic, Label};
use codespan_reporting::term::termcolor::{StandardStream, ColorChoice};
use codespan_reporting::term;
use similar::{ChangeTag, TextDiff};

use ast_grep_core::{Matcher, NodeMatch, Pattern};
use ast_grep_config::{AstGrepRuleConfig, Severity};

pub use codespan_reporting::{
    files::SimpleFile,
    term::ColorArg,
};

use crate::guess_language::SupportLang;

pub struct ErrorReporter {
    writer: StandardStream,
    config: term::Config,
}

impl ErrorReporter {
    pub fn new(
        color: ColorChoice,
    ) -> Self {
        Self {
            writer: StandardStream::stdout(color),
            config: term::Config::default(),
        }
    }

    pub fn print_rule<'a>(
        &self,
        matches: impl Iterator<Item = NodeMatch<'a, SupportLang>>,
        file: SimpleFile<Cow<str>, &String>,
        rule: &AstGrepRuleConfig,
    ) {
        let config = &self.config;
        let writer = &self.writer;
        let serverity = match rule.severity {
            Severity::Error => diagnostic::Severity::Error,
            Severity::Warning => diagnostic::Severity::Warning,
            Severity::Info => diagnostic::Severity::Note,
        };
        for m in matches{
            let range = m.inner.start_byte()..m.inner.end_byte();
            let diagnostic = Diagnostic::new(serverity)
                .with_code(&rule.id)
                .with_message(&rule.message)
                .with_notes(rule.note.iter().cloned().collect())
                .with_labels(vec![
                    Label::primary((), range),
                ]);
            term::emit(&mut writer.lock(), config, &file, &diagnostic).unwrap();
        }
    }
}

pub fn print_matches<'a>(
    matches: impl Iterator<Item = NodeMatch<'a, SupportLang>>,
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
                e.replace(pattern, rewrite).unwrap().inserted_text,
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
