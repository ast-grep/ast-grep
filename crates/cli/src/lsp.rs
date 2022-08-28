use crate::config::find_config;
use ast_grep_lsp::{Backend, LspService, Server};
use std::io::Result;

async fn run_language_server_impl() {
    // env_logger::init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let configs = find_config(None);

    let (service, socket) = LspService::build(|client| Backend::new(client, configs)).finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}

pub fn run_language_server() -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            run_language_server_impl().await;
        });
    Ok(())
}
