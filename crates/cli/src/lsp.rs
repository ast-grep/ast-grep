use crate::config::{find_rules, register_custom_language};
use crate::error::ErrorContext as EC;
use anyhow::{Context, Result};
use ast_grep_lsp::{Backend, LspService, Server};

async fn run_language_server_impl() -> Result<()> {
  // env_logger::init();
  register_custom_language(None)?;
  let stdin = tokio::io::stdin();
  let stdout = tokio::io::stdout();
  let config_result = find_rules(None, None);
  let config_result_std: std::result::Result<_, String> = config_result.map_err(|e| {
    // convert anyhow::Error to String with chain of causes
    e.chain()
      .map(|e| e.to_string())
      .collect::<Vec<_>>()
      .join(". ")
  });
  let (service, socket) = LspService::build(|client| Backend::new(client, config_result_std))
    .custom_method("ast-grep/search", Backend::search)
    .finish();
  Server::new(stdin, stdout, socket).serve(service).await;
  Ok(())
}

pub fn run_language_server() -> Result<()> {
  tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .context(EC::StartLanguageServer)?
    .block_on(async { run_language_server_impl().await })
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  #[ignore = "test lsp later"]
  fn test_lsp_start() {
    assert!(run_language_server().is_err())
  }
}
