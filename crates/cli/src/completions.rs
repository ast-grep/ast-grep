//! How to use generate shell completions.
//! Usage with bash:
//! ```console
//! sg complete > /usr/share/bash-completion/completions/ast_grep.bash
//! ```

//! Usage with zsh:
//! ```console
//! $ sg complete zsh > $HOME/.zsh/site-functions/_ast_grep
//! $ compinit
//! $ sg <TAB>
//! $ sg run --<TAB>
//! ```
//! fish:
//! ```console
//! $ sg complete fish > ast_grep.fish
//! $ ./ast_grep.fish
//! $ sg <TAB>
//! $ sg run --<TAB>
//! ```

use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};

use std::env;
use std::io;

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
    return Err(anyhow::anyhow!("impossible"))
  };
  let mut cmd = C::command();
  let cmd_name = match env::args().next() {
    Some(bin_name) => bin_name,
    None => cmd.get_name().to_string(),
  };
  generate(shell, &mut cmd, cmd_name, output);
  Ok(())
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
    assert!(output.contains("ast-grep"));
  }
}
