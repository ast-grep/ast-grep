use crate::lang::SgLang;
use crate::print::{ColorArg, JsonStyle};
use crate::utils::ErrorContext as EC;
use crate::utils::Tracing;

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use ignore::{
  overrides::{Override, OverrideBuilder},
  WalkBuilder, WalkParallel,
};
use serde::{Deserialize, Serialize};

use std::path::PathBuf;

/// input related options
#[derive(Args)]
pub struct InputArgs {
  /// The paths to search. You can provide multiple paths separated by spaces.
  #[clap(value_parser, default_value = ".")]
  pub paths: Vec<PathBuf>,

  /// Follow symbolic links.
  ///
  /// This flag instructs ast-grep to follow symbolic links while traversing
  /// directories. This behavior is disabled by default. Note that ast-grep will
  /// check for symbolic link loops and report errors if it finds one. ast-grep will
  /// also report errors for broken links.
  #[clap(long)]
  pub follow: bool,

  /// Do not respect hidden file system or ignore files (.gitignore, .ignore, etc.).
  ///
  /// You can suppress multiple ignore files by passing `no-ignore` multiple times.
  #[clap(long, action = clap::ArgAction::Append, value_name = "FILE_TYPE")]
  pub no_ignore: Vec<IgnoreFile>,

  /// Enable search code from StdIn.
  ///
  /// Use this if you need to take code stream from standard input.
  #[clap(long)]
  pub stdin: bool,

  /// Include or exclude file paths.
  ///
  /// Include or exclude files and directories for searching that match the
  /// given glob. This always overrides any other ignore logic. Multiple glob
  /// flags may be used. Globbing rules match .gitignore globs. Precede a
  /// glob with a ! to exclude it. If multiple globs match a file or
  /// directory, the glob given later in the command line takes precedence.
  #[clap(long, action = clap::ArgAction::Append)]
  pub globs: Vec<String>,
}

impl InputArgs {
  pub fn walk(&self) -> Result<WalkParallel> {
    let threads = num_cpus::get().min(12);
    let globs = self.build_globs().context(EC::BuildGlobs)?;
    Ok(
      NoIgnore::disregard(&self.no_ignore)
        .walk(&self.paths)
        .threads(threads)
        .follow_links(self.follow)
        .overrides(globs)
        .build_parallel(),
    )
  }

  pub fn walk_lang(&self, lang: SgLang) -> WalkParallel {
    let threads = num_cpus::get().min(12);
    NoIgnore::disregard(&self.no_ignore)
      .walk(&self.paths)
      .threads(threads)
      .follow_links(self.follow)
      .types(lang.augmented_file_type())
      .build_parallel()
  }

  fn build_globs(&self) -> Result<Override> {
    let cwd = std::env::current_dir()?;
    let mut builder = OverrideBuilder::new(cwd);
    for glob in &self.globs {
      builder.add(glob)?;
    }
    Ok(builder.build()?)
  }
}

/// output related options
#[derive(Args)]
pub struct OutputArgs {
  /// Start interactive edit session.
  ///
  /// You can confirm the code change and apply it to files selectively,
  /// or you can open text editor to tweak the matched code.
  /// Note that code rewrite only happens inside a session.
  #[clap(short, long)]
  pub interactive: bool,

  /// Apply all rewrite without confirmation if true.
  #[clap(short = 'U', long)]
  pub update_all: bool,

  /// Output matches in structured JSON .
  ///
  /// If this flag is set, ast-grep will output matches in JSON format.
  /// You can pass optional value to this flag by using `--json=<style>` syntax
  /// to further control how JSON object is formatted and printed. ast-grep will `pretty`-print JSON if no value is passed.
  /// Note, the json flag must use `=` to specify its value.
  /// It conflicts with interactive.
  #[clap(
      long,
      conflicts_with = "interactive",
      value_name="style",
      num_args(0..=1),
      require_equals = true,
      default_missing_value = "pretty"
  )]
  pub json: Option<JsonStyle>,

  /// Controls output color.
  ///
  /// This flag controls when to use colors. The default setting is 'auto', which
  /// means ast-grep will try to guess when to use colors. If ast-grep is
  /// printing to a terminal, then it will use colors, but if it is redirected to a
  /// file or a pipe, then it will suppress color output. ast-grep will also suppress
  /// color output in some other circumstances. For example, no color will be used
  /// if the TERM environment variable is not set or set to 'dumb'.
  #[clap(long, default_value = "auto", value_name = "WHEN")]
  pub color: ColorArg,

  /// Show tracing information for file/rule discovery and scanning.
  ///
  /// This flag helps user to inspect ast-grep's internal filtering of files and rules.
  /// tracing will output how many and why files and rules are scanned and skipped.
  /// tracing information outputs to stderr and does not affect the result of the search.
  #[clap(long, default_value = "nothing", value_name = "LEVEL")]
  pub tracing: Tracing,
}

impl OutputArgs {
  // either explicit interactive or implicit update_all
  pub fn needs_interactive(&self) -> bool {
    self.interactive || self.update_all
  }
}

/// File types to ignore, this is mostly the same as ripgrep.
#[derive(Clone, Copy, Deserialize, Serialize, ValueEnum)]
pub enum IgnoreFile {
  /// Search hidden files and directories. By default, hidden files and directories are skipped.
  Hidden,
  /// Don't respect .ignore files.
  /// This does *not* affect whether ast-grep will ignore files and directories whose names begin with a dot.
  /// For that, use --no-ignore hidden.
  Dot,
  /// Don't respect ignore files that are manually configured for the repository such as git's '.git/info/exclude'.
  Exclude,
  /// Don't respect ignore files that come from "global" sources such as git's
  /// `core.excludesFile` configuration option (which defaults to `$HOME/.config/git/ignore`).
  Global,
  /// Don't respect ignore files (.gitignore, .ignore, etc.) in parent directories.
  Parent,
  /// Don't respect version control ignore files (.gitignore, etc.).
  /// This implies --no-ignore parent for VCS files.
  /// Note that .ignore files will continue to be respected.
  Vcs,
}

#[derive(Default)]
pub struct NoIgnore {
  disregard_hidden: bool,
  disregard_parent: bool,
  disregard_dot: bool,
  disregard_vcs: bool,
  disregard_global: bool,
  disregard_exclude: bool,
}

impl NoIgnore {
  pub fn disregard(ignores: &[IgnoreFile]) -> Self {
    let mut ret = NoIgnore::default();
    use IgnoreFile::*;
    for ignore in ignores {
      match ignore {
        Hidden => ret.disregard_hidden = true,
        Dot => ret.disregard_dot = true,
        Exclude => ret.disregard_exclude = true,
        Global => ret.disregard_global = true,
        Parent => ret.disregard_parent = true,
        Vcs => ret.disregard_vcs = true,
      }
    }
    ret
  }

  pub fn walk(&self, path: &[PathBuf]) -> WalkBuilder {
    let mut paths = path.iter();
    let mut builder = WalkBuilder::new(paths.next().expect("non empty"));
    for path in paths {
      builder.add(path);
    }
    builder
      .hidden(!self.disregard_hidden)
      .parents(!self.disregard_parent)
      .ignore(!self.disregard_dot)
      .git_global(!self.disregard_vcs && !self.disregard_global)
      .git_ignore(!self.disregard_vcs)
      .git_exclude(!self.disregard_vcs && !self.disregard_exclude);
    builder
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_build_globs() {
    let input = InputArgs {
      paths: vec![],
      follow: true,
      no_ignore: vec![IgnoreFile::Dot, IgnoreFile::Exclude],
      stdin: false,
      globs: vec!["*.rs".to_string(), "!*.toml".to_string()],
    };
    assert!(input.build_globs().is_ok());
    let input = InputArgs {
      paths: vec![],
      follow: true,
      no_ignore: vec![IgnoreFile::Dot, IgnoreFile::Exclude],
      stdin: false,
      globs: vec!["*.{rs".to_string()],
    };
    assert!(input.build_globs().is_err());
  }
}
