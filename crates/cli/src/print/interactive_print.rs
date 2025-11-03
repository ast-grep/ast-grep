use super::{ColoredPrinter, Diff, NodeMatch, PrintProcessor, Printer};
use crate::lang::SgLang;
use crate::utils::ErrorContext as EC;
use crate::utils::{self, clear};

use anyhow::{Context, Result};
use ast_grep_config::RuleConfig;
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term::termcolor::{Buffer, StandardStream};
use smallvec::{smallvec, SmallVec};

use std::borrow::Cow;
use std::ops::Range;
use std::path::{Path, PathBuf};

type InnerPrinter = ColoredPrinter<StandardStream>;

pub struct InteractivePrinter {
  accept_all: bool,
  from_stdin: bool,
  committed_cnt: usize,
  inner: InnerPrinter,
}

impl InteractivePrinter {
  pub fn new(inner: InnerPrinter, accept_all: bool, from_stdin: bool) -> Result<Self> {
    if from_stdin && !accept_all {
      Err(anyhow::anyhow!(EC::StdInIsNotInteractive))
    } else {
      Ok(Self {
        accept_all,
        from_stdin,
        inner,
        committed_cnt: 0,
      })
    }
  }

  fn prompt_edit(&self) -> char {
    if self.accept_all {
      return 'a';
    }
    const EDIT_PROMPT: &str = "Accept? [y]es/[↵], [n]o, [a]ll, [q]uit, [e]dit";
    utils::prompt(EDIT_PROMPT, "ynaqe\t", Some('y')).expect("Error happened during prompt")
  }

  fn prompt_view(&self) -> char {
    if self.accept_all {
      return '\n';
    }
    const VIEW_PROMPT: &str = "Next[enter], Quit[q], Edit[e]";
    utils::prompt(VIEW_PROMPT, "qe", Some('\n')).expect("cannot fail")
  }

  fn rewrite_action(&self, diffs: Diffs<()>, path: &PathBuf) -> Result<()> {
    if diffs.contents.is_empty() {
      return Ok(());
    }
    let new_content = apply_rewrite(diffs);
    if self.from_stdin {
      println!("{new_content}");
      Ok(())
    } else {
      std::fs::write(path, new_content).with_context(|| EC::WriteFile(path.clone()))
    }
  }

  fn process_highlights(&mut self, highlights: Highlights<Buffer>) -> Result<()> {
    let Highlights {
      path,
      first_line,
      inner,
    } = highlights;
    // if ast-grep is called with -U, do not output anything
    if self.accept_all {
      return Ok(());
    }
    utils::run_in_alternate_screen(|| {
      self.inner.process(inner)?;
      let resp = self.prompt_view();
      if resp == 'q' {
        Err(anyhow::anyhow!(EC::ExitInteractiveEditing))
      } else if resp == 'e' {
        open_in_editor(&path, first_line)?;
        Ok(())
      } else {
        Ok(())
      }
    })
  }

  fn process_diffs(&mut self, diffs: Diffs<Buffer>) -> Result<()> {
    let path = diffs.path.clone();
    let (confirmed, quit) = process_diffs_interactive(self, diffs)?;
    self.rewrite_action(confirmed, &path)?;
    if quit {
      Err(anyhow::anyhow!(EC::ExitInteractiveEditing))
    } else {
      Ok(())
    }
  }
}

impl Printer for InteractivePrinter {
  type Processed = Payload<InnerPrinter>;
  type Processor = InteractiveProcessor<InnerPrinter>;

  fn get_processor(&self) -> Self::Processor {
    InteractiveProcessor {
      inner: self.inner.get_processor(),
    }
  }

  fn process(&mut self, processed: Self::Processed) -> Result<()> {
    use InteractivePayload as IP;
    match processed {
      IP::Nothing => Ok(()),
      IP::Highlights(h) => self.process_highlights(h),
      IP::Diffs(d) => self.process_diffs(d),
    }
  }

  fn after_print(&mut self) -> Result<()> {
    if self.committed_cnt > 0 {
      eprintln!("Applied {} changes", self.committed_cnt);
    }
    self.inner.after_print()
  }
}

#[derive(Clone, Debug)]
pub struct InteractiveDiff<D> {
  /// string content for the replacement
  replacement: String,
  range: Range<usize>,
  first_line: usize,
  title: Option<String>,
  display: D,
}

impl<D> InteractiveDiff<D> {
  fn new(diff: Diff, display: D) -> Self {
    Self {
      first_line: diff.node_match.start_pos().line(),
      replacement: diff.replacement,
      range: diff.range,
      title: diff.title,
      display,
    }
  }

  fn split(self) -> (InteractiveDiff<()>, D) {
    let pure = InteractiveDiff {
      first_line: self.first_line,
      range: self.range,
      replacement: self.replacement,
      title: self.title,
      display: (),
    };
    (pure, self.display)
  }
}

pub struct Highlights<D> {
  path: PathBuf,
  first_line: usize,
  inner: D,
}

pub struct Diffs<D> {
  path: PathBuf,
  // TODO: this clone is slow
  old_source: String,
  contents: Vec<SmallVec<[InteractiveDiff<D>; 1]>>,
}

pub enum InteractivePayload<D> {
  Nothing,
  Highlights(Highlights<D>),
  Diffs(Diffs<D>),
}

pub struct InteractiveProcessor<P: Printer> {
  inner: P::Processor,
}

pub type Payload<P> = InteractivePayload<<P as Printer>::Processed>;

impl<P> PrintProcessor<Payload<P>> for InteractiveProcessor<P>
where
  P: Printer + 'static,
{
  fn print_rule(
    &self,
    matches: Vec<NodeMatch>,
    file: SimpleFile<Cow<str>, &str>,
    rule: &RuleConfig<SgLang>,
  ) -> Result<Payload<P>> {
    let Some(first_match) = matches.first() else {
      return Ok(InteractivePayload::Nothing);
    };
    let first_line = first_match.start_pos().line();
    let path = PathBuf::from(file.name().to_string());
    let inner = self.inner.print_rule(matches, file, rule)?;
    let highlights = Highlights {
      inner,
      first_line,
      path,
    };
    Ok(InteractivePayload::Highlights(highlights))
  }

  fn print_matches(&self, matches: Vec<NodeMatch>, path: &Path) -> Result<Payload<P>> {
    let Some(first_match) = matches.first() else {
      return Ok(InteractivePayload::Nothing);
    };
    let first_line = first_match.start_pos().line();
    let inner = self.inner.print_matches(matches, path)?;
    let highlights = Highlights {
      inner,
      first_line,
      path: path.to_path_buf(),
    };
    Ok(InteractivePayload::Highlights(highlights))
  }

  fn print_diffs(&self, diffs: Vec<Diff>, path: &Path) -> Result<Payload<P>> {
    let old_source = get_old_source(diffs.first());
    let mut contents = Vec::with_capacity(diffs.len());
    for diff in diffs {
      let display = self.inner.print_diffs(vec![diff.clone()], path)?;
      let content = InteractiveDiff::new(diff, display);
      contents.push(smallvec![content]);
    }
    Ok(InteractivePayload::Diffs(Diffs {
      path: path.to_path_buf(),
      old_source,
      contents,
    }))
  }
  fn print_rule_diffs(
    &self,
    diffs: Vec<(Diff<'_>, &RuleConfig<SgLang>)>,
    path: &Path,
  ) -> Result<Payload<P>> {
    let old_source = get_old_source(diffs.first().map(|d| &d.0));
    let mut contents = Vec::with_capacity(diffs.len());
    for (diff_list, rule) in diffs {
      let diffs = diff_list.into_list();
      let content: Result<_> = diffs
        .into_iter()
        .map(|diff| {
          let display = self
            .inner
            .print_rule_diffs(vec![(diff.clone(), rule)], path)?;
          let diff = InteractiveDiff::new(diff, display);
          Ok(diff)
        })
        .collect();
      contents.push(content?);
    }
    Ok(InteractivePayload::Diffs(Diffs {
      path: path.to_path_buf(),
      old_source,
      contents,
    }))
  }
}

fn get_old_source(diff: Option<&Diff>) -> String {
  let Some(node) = diff else {
    return String::new();
  };
  node.get_root_text().to_string()
}

fn process_diffs_interactive(
  interactive: &mut InteractivePrinter,
  diffs: Diffs<Buffer>,
) -> Result<(Diffs<()>, bool)> {
  let mut confirmed = vec![];
  let mut end = 0;
  let mut quit = false;
  let path = diffs.path;
  for diff in diffs.contents {
    let diff_list: Vec<_> = diff
      .into_iter()
      .filter(|diff| diff.range.start >= end)
      .collect();
    if diff_list.is_empty() {
      continue;
    }
    use InteractionChoice as IC;
    let to_confirm = match print_diff_and_prompt_action(interactive, &path, diff_list)? {
      IC::Yes(c) => c,
      IC::All(c) => {
        interactive.accept_all = true;
        c
      }
      IC::No => continue,
      IC::Quit => {
        quit = true;
        break;
      }
    };
    end = to_confirm.range.end;
    confirmed.push(smallvec![to_confirm]);
    interactive.committed_cnt = interactive.committed_cnt.saturating_add(1);
  }
  let diffs = Diffs {
    path,
    old_source: diffs.old_source,
    contents: confirmed,
  };
  Ok((diffs, quit))
}

enum InteractionChoice {
  Yes(InteractiveDiff<()>),
  All(InteractiveDiff<()>),
  No,
  Quit,
}

/// returns if accept_current and accept_all
fn print_diff_and_prompt_action(
  interactive: &mut InteractivePrinter,
  path: &Path,
  mut processed: Vec<InteractiveDiff<Buffer>>,
) -> Result<InteractionChoice> {
  // default to first diff when accept_all
  if interactive.accept_all {
    let confirmed = processed.remove(0).split().0;
    return Ok(InteractionChoice::Yes(confirmed));
  }
  utils::run_in_alternate_screen(|| {
    let mut to_confirm = Vec::with_capacity(processed.len());
    let mut display = Vec::with_capacity(processed.len());
    for diff in processed {
      let (c, d) = diff.split();
      to_confirm.push(c);
      display.push(d);
    }
    let mut index = 0;
    let len = to_confirm.len();
    let titles: Vec<_> = to_confirm.iter().map(|d| d.title.as_deref()).collect();
    let ret = loop {
      let confirmed = to_confirm[index].clone();
      let display = display[index].clone();
      interactive.inner.process(display)?;
      interactive.inner.print_diff_title(&titles, index)?;
      break match interactive.prompt_edit() {
        '\t' => {
          index = (index + 1) % len;
          clear()?;
          continue;
        }
        'y' => InteractionChoice::Yes(confirmed),
        'a' => InteractionChoice::All(confirmed),
        'e' => {
          let pos = confirmed.first_line;
          open_in_editor(path, pos)?;
          InteractionChoice::No
        }
        'q' => InteractionChoice::Quit,
        'n' => InteractionChoice::No,
        _ => return Err(anyhow::anyhow!("Unexpected choice")),
      };
    };
    Ok(ret)
  })
}

fn apply_rewrite(diffs: Diffs<()>) -> String {
  let mut new_content = String::new();
  let old_content = diffs.old_source;
  let mut start = 0;
  for mut diff_list in diffs.contents {
    let diff = diff_list.remove(0);
    let range = diff.range;
    new_content.push_str(&old_content[start..range.start]);
    new_content.push_str(&diff.replacement);
    start = range.end;
  }
  // add trailing statements
  new_content.push_str(&old_content[start..]);
  new_content
}

/// start_line is zero-based
fn open_in_editor(path: &Path, start_line: usize) -> Result<()> {
  let editor = std::env::var("EDITOR").unwrap_or_else(|_| String::from("vim"));
  let exit = std::process::Command::new(editor)
    .arg(path)
    .arg(format!("+{}", start_line + 1))
    .spawn()
    .context(EC::OpenEditor)?
    .wait()
    .context(EC::OpenEditor)?;
  if exit.success() {
    Ok(())
  } else {
    Err(anyhow::anyhow!(EC::OpenEditor))
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use ast_grep_config::{from_yaml_string, Fixer, GlobalRules};
  use ast_grep_core::tree_sitter::{StrDoc, Visitor};
  use ast_grep_core::{AstGrep, Matcher};
  use ast_grep_language::SupportLang;

  fn make_rule(rule: &str) -> RuleConfig<SgLang> {
    let globals = GlobalRules::default();
    from_yaml_string(
      &format!(
        r"
id: test
message: test rule
severity: info
language: TypeScript
{rule}"
      ),
      &globals,
    )
    .unwrap()
    .pop()
    .unwrap()
  }

  fn make_diffs(grep: &AstGrep<StrDoc<SgLang>>, matcher: impl Matcher, fixer: &Fixer) -> Diffs<()> {
    let root = grep.root();
    let old_source = root.root().get_text().to_string();
    let contents = Visitor::new(&matcher)
      .reentrant(false)
      .visit(root)
      .map(|nm| {
        let diff = Diff::generate(nm, &matcher, fixer);
        smallvec![InteractiveDiff {
          first_line: 0,
          range: diff.range,
          replacement: diff.replacement,
          title: diff.title,
          display: (),
        }]
      })
      .collect();
    Diffs {
      old_source,
      path: PathBuf::new(),
      contents,
    }
  }

  #[test]
  fn test_apply_rewrite() {
    let root = AstGrep::new("let a = () => c++", SupportLang::TypeScript.into());
    let config = make_rule(
      r"
rule:
  all:
    - pattern: $B
    - any:
        - pattern: $A++
fix: ($B, lifecycle.update(['$A']))",
    );
    let mut matcher = config.matcher;
    let fixer = matcher.fixer.remove(0);
    let diffs = make_diffs(&root, matcher, &fixer);
    let ret = apply_rewrite(diffs);
    assert_eq!(ret, "let a = () => (c++, lifecycle.update(['c']))");
  }

  #[test]
  fn test_rewrite_nested() {
    let root = AstGrep::new("Some(Some(1))", SupportLang::TypeScript.into());
    let diffs = make_diffs(
      &root,
      "Some($A)",
      &Fixer::from_str::<SgLang>("$A", &SupportLang::TypeScript.into())
        .expect("fixer must compile"),
    );
    let ret = apply_rewrite(diffs);
    assert_eq!("Some(1)", ret);
  }

  // https://github.com/ast-grep/ast-grep/issues/668
  #[test]
  fn test_rewrite_with_empty_lines() {
    let root = AstGrep::new("\n\n\nSome(1)", SupportLang::TypeScript.into());
    let diffs = make_diffs(
      &root,
      "Some($A)",
      &Fixer::from_str::<SgLang>("$A", &SupportLang::TypeScript.into())
        .expect("fixer must compile"),
    );
    let ret = apply_rewrite(diffs);
    assert_eq!("\n\n\n1", ret);
  }

  fn test_open_editor_respect_editor_env() {
    std::env::set_var("EDITOR", "echo");
    let exit = open_in_editor(&PathBuf::from("Cargo.toml"), 1);
    assert!(exit.is_ok());
  }

  fn test_open_editor_error_handling() {
    std::env::set_var("EDITOR", "NOT_EXIST_XXXXX");
    let exit = open_in_editor(&PathBuf::from("Cargo.toml"), 1);
    let error = exit.expect_err("should be error");
    let error = error.downcast_ref::<EC>().expect("should be error context");
    assert!(matches!(error, EC::OpenEditor));
  }

  #[test]
  fn test_open_editor() {
    // these two tests must run in sequence
    // since setting env will cause racing condition
    test_open_editor_respect_editor_env();
    test_open_editor_error_handling();
  }
}
