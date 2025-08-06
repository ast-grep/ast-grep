use crate::config::ProjectConfig;
use crate::lang::SgLang;
use crate::utils::{ErrorContext as EC, RuleOverwrite};
use anyhow::{Context, Result};
use ast_grep_config::RuleCollection;
use ast_grep_lsp::{Backend, LspService, Server};
use clap::Args;

#[derive(Args)]
pub struct LspArg {}

/// Create a rule finding closure that can be used by LSP or other consumers
/// This allows decoupling the rule finding logic from the specific consumers
pub fn make_rule_finder(
  project_config: ProjectConfig,
) -> impl Fn() -> anyhow::Result<RuleCollection<SgLang>> + Send + Sync + 'static 
{
  move || {
    let (collection, _trace) = project_config.find_rules(RuleOverwrite::default())?;
    Ok(collection)
  }
}

async fn run_language_server_impl(_arg: LspArg, project: Result<ProjectConfig>) -> Result<()> {
  // env_logger::init();
  // TODO: move this error to client
  let project_config = project?;
  let stdin = tokio::io::stdin();
  let stdout = tokio::io::stdout();

  let config_base = project_config.project_dir.clone();

  // Create a rule finder closure that uses the CLI logic
  let rule_finder = make_rule_finder(project_config);

  let (service, socket) =
    LspService::build(|client| Backend::new(client, config_base, rule_finder)).finish();
  Server::new(stdin, stdout, socket).serve(service).await;
  Ok(())
}

pub fn run_language_server(arg: LspArg, project: Result<ProjectConfig>) -> Result<()> {
  tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .context(EC::StartLanguageServer)?
    .block_on(async { run_language_server_impl(arg, project).await })
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  #[ignore = "test lsp later"]
  fn test_lsp_start() {
    let arg = LspArg {};
    assert!(run_language_server(arg, Err(anyhow::anyhow!("error"))).is_err())
  }
}
