use crate::config::{find_rules, register_custom_language, ProjectConfig};
use crate::utils::ErrorContext as EC;
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

async fn run_language_server_impl(arg: LspArg) -> Result<()> {
  // env_logger::init();
  let project_config = ProjectConfig::by_config_path(arg.config.clone())?;
  // TODO: move this error to client
  let project_config = project_config.ok_or_else(|| anyhow::anyhow!(EC::ProjectNotExist))?;
  let config_base = project_config.project_dir.clone();
  register_custom_language(Some(project_config))?;
  let stdin = tokio::io::stdin();
  let stdout = tokio::io::stdout();
  let config_result = find_rules(arg.config, Default::default());
  let config_result_std: std::result::Result<_, String> = config_result
    .map_err(|e| {
      // convert anyhow::Error to String with chain of causes
      e.chain()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join(". ")
    })
    .map(|r| r.0);
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
