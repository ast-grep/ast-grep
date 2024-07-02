use crate::config::{find_config_path_with_default, find_rules, register_custom_language};
use crate::error::ErrorContext as EC;
use anyhow::{Context, Result};
use ast_grep_lsp::{Backend, LspService, Server};
use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct LspArg {
  /// Path to ast-grep root config, default is sgconfig.yml.
  #[clap(short, long, value_name = "CONFIG_FILE")]
  config: Option<PathBuf>,
}

fn find_config_base(config: Option<PathBuf>) -> Result<PathBuf> {
  let mut config_path = find_config_path_with_default(config, None)?;
  config_path.pop();
  Ok(config_path)
}

async fn run_language_server_impl(arg: LspArg) -> Result<()> {
  // env_logger::init();
  register_custom_language(arg.config.clone())?;
  let stdin = tokio::io::stdin();
  let stdout = tokio::io::stdout();
  let config_base = find_config_base(arg.config.clone())?;
  let config_result = find_rules(arg.config, None);
  let config_result_std: std::result::Result<_, String> = config_result.map_err(|e| {
    // convert anyhow::Error to String with chain of causes
    e.chain()
      .map(|e| e.to_string())
      .collect::<Vec<_>>()
      .join(". ")
  });
  let (service, socket) =
    LspService::build(|client| Backend::new(client, config_base, config_result_std)).finish();
  Server::new(stdin, stdout, socket).serve(service).await;
  Ok(())
}

pub fn run_language_server(arg: LspArg) -> Result<()> {
  tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .context(EC::StartLanguageServer)?
    .block_on(async { run_language_server_impl(arg).await })
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  #[ignore = "test lsp later"]
  fn test_lsp_start() {
    let arg = LspArg { config: None };
    assert!(run_language_server(arg).is_err())
  }
}
