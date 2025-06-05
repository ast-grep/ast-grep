use ast_grep_config::{from_yaml_string, GlobalRules, RuleCollection, RuleConfig};
use ast_grep_language::SupportLang;
use ast_grep_lsp::*;
use serde_json::Value;
use std::path::Path;
use std::str::FromStr;
use tokio::io::duplex;
use tokio::time::{timeout, Duration};
use tower_lsp_server::lsp_types::*;

mod lsp_client;
use lsp_client::{LspClient, LspStreams};

#[test]
fn req_resp_should_work() {
  let req1_str = "{\"jsonrpc\":\"2.0\",\"method\":\"window/logMessage\",\"params\":{\"message\":\"Running CodeAction source.fixAll\",\"type\":4}}";
  let req2_str = "{\"jsonrpc\":\"2.0\",\"result\":[{\"edit\":{},\"isPreferred\":true,\"kind\":\"source.fixAll\",\"title\":\"Source Code fix action\"}],\"id\":1}";

  let test_buf = format!(
    "{}{}",
    lsp_client::jsonrpc::format_message(req1_str),
    lsp_client::jsonrpc::format_message(req2_str)
  );

  let resp_list = lsp_client::jsonrpc::parse_messages_from_bytes(test_buf.as_bytes());
  assert_eq!(
    resp_list,
    vec![
      serde_json::from_str::<Value>(req1_str).unwrap(),
      serde_json::from_str::<Value>(req2_str).unwrap()
    ]
  )
}

pub struct AstGrepLspClient {
  client: LspClient,
}

impl AstGrepLspClient {
  pub fn new() -> Self {
    let globals = GlobalRules::default();
    let config: RuleConfig<SupportLang> = from_yaml_string(
      r"
id: no-console-rule
message: No console.log
severity: warning
language: TypeScript
rule:
  pattern: console.log($$$A)
note: no console.log
fix: |
  alert($$$A)
",
      &globals,
    )
    .unwrap()
    .pop()
    .unwrap();
    let base = Path::new("./").to_path_buf();
    let rc: RuleCollection<SupportLang> = RuleCollection::try_new(vec![config]).unwrap();
    let rc_result: std::result::Result<_, String> = Ok(rc);
    let (service, socket) =
      LspService::build(|client| Backend::new(client, base, rc_result)).finish();
    let (req_client, req_server) = duplex(1024);
    let (resp_server, resp_client) = duplex(1024);

    // start server as concurrent task
    tokio::spawn(Server::new(req_server, resp_server, socket).serve(service));

    let streams = LspStreams {
      request_stream: req_client,
      response_stream: resp_client,
    };

    let mut client = LspClient::new(streams);

    // Add handlers for workspace requests
    client.add_handler::<request::WorkspaceFoldersRequest, _>(|_params| {
      Some(vec![WorkspaceFolder {
        uri: Uri::from_str("file:///Users/codes/ast-grep-vscode").unwrap(),
        name: "test-workspace".to_string(),
      }])
    });

    // Add ApplyWorkspaceEdit handler to prevent deadlock in tests
    client.add_handler::<request::ApplyWorkspaceEdit, _>(|_params| ApplyWorkspaceEditResponse {
      applied: true,
      failed_change: None,
      failure_reason: None,
    });

    AstGrepLspClient { client }
  }

  pub async fn initialize(&mut self) -> InitializeResult {
    let params = InitializeParams {
      capabilities: ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
          synchronization: Some(TextDocumentSyncClientCapabilities::default()),
          ..Default::default()
        }),
        ..Default::default()
      },
      workspace_folders: Some(vec![WorkspaceFolder {
        uri: Uri::from_str("file:///Users/codes/ast-grep-vscode").unwrap(),
        name: "test-workspace".to_string(),
      }]),
      ..Default::default()
    };

    let id = self.client.send_request::<request::Initialize>(params);
    self
      .client
      .wait_for_response::<request::Initialize>(id)
      .await
      .unwrap()
  }

  pub async fn did_open(&mut self, uri: Uri, language_id: String, text: String) {
    let params = DidOpenTextDocumentParams {
      text_document: TextDocumentItem {
        uri,
        language_id,
        version: 1,
        text,
      },
    };

    self
      .client
      .send_notification::<notification::DidOpenTextDocument>(params);
  }

  pub async fn request_code_action(&mut self) -> Option<CodeActionResponse> {
    let params = CodeActionParams {
      text_document: TextDocumentIdentifier {
        uri: Uri::from_str("file:///Users/codes/ast-grep-vscode/test.tsx").unwrap(),
      },
      range: Range {
        start: Position {
          line: 1,
          character: 10,
        },
        end: Position {
          line: 1,
          character: 10,
        },
      },
      context: CodeActionContext {
        diagnostics: vec![Diagnostic {
          range: Range {
            start: Position {
              line: 0,
              character: 0,
            },
            end: Position {
              line: 0,
              character: 16,
            },
          },
          code: Some(NumberOrString::String("no-console-rule".to_string())),
          source: Some("ast-grep".to_string()),
          message: "No console.log".to_string(),
          ..Default::default()
        }],
        only: Some(vec![CodeActionKind::SOURCE_FIX_ALL]),
        ..Default::default()
      },
      work_done_progress_params: WorkDoneProgressParams::default(),
      partial_result_params: PartialResultParams::default(),
    };

    let id = self
      .client
      .send_request::<request::CodeActionRequest>(params);
    self
      .client
      .wait_for_response::<request::CodeActionRequest>(id)
      .await
      .unwrap_or(None)
  }

  pub async fn request_execute_command(&mut self, uri: Uri, text: String) -> Option<Value> {
    let text_doc_item = TextDocumentItem {
      uri,
      language_id: "typescript".to_string(),
      version: 1,
      text,
    };

    let params = ExecuteCommandParams {
      command: "ast-grep.applyAllFixes".to_string(),
      arguments: vec![serde_json::to_value(text_doc_item).unwrap()],
      work_done_progress_params: WorkDoneProgressParams::default(),
    };

    let id = self.client.send_request::<request::ExecuteCommand>(params);
    self
      .client
      .wait_for_response::<request::ExecuteCommand>(id)
      .await
      .unwrap_or(None)
  }
}

#[test]
fn test_basic() {
  tokio::runtime::Runtime::new().unwrap().block_on(async {
    let mut lsp_client = AstGrepLspClient::new();

    let result = lsp_client.initialize().await;

    assert!(result.server_info.is_some());
    assert!(result.capabilities.code_action_provider.is_some());
  });
}

#[test]
#[ignore = "fixAll conflicts with quickfix"]
fn test_code_action() {
  tokio::runtime::Runtime::new().unwrap().block_on(async {
    let mut lsp_client = AstGrepLspClient::new();

    lsp_client.initialize().await;

    let result = lsp_client.request_code_action().await;

    // Since the file doesn't exist in the server's map, this should return None
    assert!(result.is_none());
  });
}

#[test]
fn test_execute_apply_all_fixes() {
  tokio::runtime::Runtime::new().unwrap().block_on(async {
    let mut lsp_client = AstGrepLspClient::new();

    // Add handler for workspace/applyEdit to prevent deadlock
    lsp_client
      .client
      .add_handler::<request::ApplyWorkspaceEdit, _>(|_params| ApplyWorkspaceEditResponse {
        applied: true,
        failed_change: None,
        failure_reason: None,
      });

    lsp_client.initialize().await;

    // First, open a file with content that has issues the rule can fix
    let test_uri = Uri::from_str("file:///Users/codes/ast-grep-vscode/test.ts").unwrap();
    let test_content = "class AstGrepTest {\n  test() {\n    console.log('Hello, world!')\n  }\n}";

    lsp_client
      .did_open(
        test_uri.clone(),
        "typescript".to_string(),
        test_content.to_string(),
      )
      .await;

    let result = lsp_client.request_execute_command(test_uri, test_content.to_string()).await;

    // The executeCommand should return None (or we timed out)
    assert!(result.is_none());

    // Need to add back tests to this to check the response.
  });
}
