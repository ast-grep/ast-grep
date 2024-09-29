//! How to use generate shell completions.
//! Usage with bash:
//! ```console
//! sg completions > ast_grep.bash
//! $ ./ast_grep.bash
//! $ sg <TAB>
//! $ sg run --<TAB>
//! ```
//! Usage with zsh, the completion scripts have to be in a path that belongs to `$fpath`:
//! ```console
//! $ sg completions zsh > $HOME/.zsh/completions/_ast_grep
//! $ echo "fpath=($HOME/.zsh/completions $fpath)" >> ~/.zshrc
//! $ compinit
//! $ sg <TAB>
//! $ sg run --<TAB>
//! ```
//! Usage with fish:
//! ```console
//! $ sg completions fish > ast_grep.fish
//! $ ./ast_grep.fish
//! $ sg <TAB>
//! $ sg run --<TAB>
//! ```

use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};

use crate::utils::ErrorContext as EC;

use std::env;
use std::io;
use std::path::Path;

#[derive(Parser)]
pub struct CompletionsArg {
  /// Output the completion file for given shell.
  /// If not provided, shell flavor will be inferred from environment.
  #[arg(value_enum)]
  shell: Option<Shell>,
}

pub fn run_shell_completion<C: CommandFactory>(arg: CompletionsArg) -> Result<()> {
  run_shell_completion_impl::<C, _>(arg, &mut io::stdout())
}

fn run_shell_completion_impl<C: CommandFactory, W: io::Write>(
  arg: CompletionsArg,
  output: &mut W,
) -> Result<()> {
  let Some(shell) = arg.shell.or_else(Shell::from_env) else {
    return Err(anyhow::anyhow!(EC::CannotInferShell));
  };
  let mut cmd = C::command();
  let cmd_name = match get_bin_name() {
    Some(cmd) => cmd,
    None => cmd.get_name().to_string(),
  };
  generate(shell, &mut cmd, cmd_name, output);
  Ok(())
}

// https://github.com/clap-rs/clap/blob/063b1536289f72369bcd59d61449d355aa3a1d6b/clap_builder/src/builder/command.rs#L781
fn get_bin_name() -> Option<String> {
  let bin_path = env::args().next()?;
  let p = Path::new(&bin_path);
  let name = p.file_name()?;
  Some(name.to_str()?.to_string())
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::App;

  #[test]
  fn test_generate_command() {
    let mut output = vec![];
    let arg = CompletionsArg {
      shell: Some(Shell::Bash),
    };
    run_shell_completion_impl::<App, _>(arg, &mut output).expect("should succeed");
    let output = String::from_utf8(output).expect("should be valid");
    assert!(output.contains("ast_grep"));
  }
}
