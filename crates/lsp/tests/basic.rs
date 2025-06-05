use ast_grep_config::{from_yaml_string, GlobalRules, RuleCollection, RuleConfig};
use ast_grep_language::SupportLang;
use ast_grep_lsp::*;
use serde_json::Value;
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt, DuplexStream};
use tower_lsp_server::lsp_types::*;
use std::str::FromStr;

use std::path::Path;

pub fn req(msg: &str) -> String {
  format!("Content-Length: {}\r\n\r\n{}", msg.len(), msg)
}


// parse json rpc format
pub fn parse_jsonrpc(input: &mut &str) -> Option<Value> {
  let input_str = input.trim_start().trim_start_matches("Content-Length: ");

  let index = input_str.find('\r')?;
  let length = input_str[..index].parse::<usize>().ok()?;
  let input_str = &input_str[length.to_string().len()..];

  let input_str = input_str.trim_start_matches("\r\n\r\n");

  let body = &input_str[..length];
  let value = serde_json::from_str(&body[..length]).ok()?;
  *input = &input_str[length..];
  value
}

// A function that takes a byte slice as input and parse them to Vec<serde_json::Value>
pub fn resp(input: &[u8]) -> Vec<Value> {
  let mut input_str = std::str::from_utf8(input).unwrap();

  let mut resp_list = Vec::new();

  while let Some(val) = parse_jsonrpc(&mut input_str) {
    resp_list.push(val);
  }
  resp_list
}

#[test]
fn req_resp_should_work() {
  let req1_str = "{\"jsonrpc\":\"2.0\",\"method\":\"window/logMessage\",\"params\":{\"message\":\"Running CodeAction source.fixAll\",\"type\":4}}";
  let req2_str = "{\"jsonrpc\":\"2.0\",\"result\":[{\"edit\":{},\"isPreferred\":true,\"kind\":\"source.fixAll\",\"title\":\"Source Code fix action\"}],\"id\":1}";

  let test_buf = format!("{}{}", req(req1_str), req(req2_str));

  let resp_list = resp(test_buf.as_bytes());
  assert_eq!(
    resp_list,
    vec![
      serde_json::from_str::<Value>(req1_str).unwrap(),
      serde_json::from_str::<Value>(req2_str).unwrap()
    ]
  )
}

pub struct LspClient {
  req_client: DuplexStream,
  resp_client: DuplexStream,
  next_id: i32,
}

impl LspClient {
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

    LspClient {
      req_client,
      resp_client,
      next_id: 1,
    }
  }

  async fn send_request<T: serde::Serialize>(&mut self, method: &str, params: T) -> Vec<u8> {
    let id = self.next_id;
    self.next_id += 1;
    
    let request = serde_json::json!({
      "jsonrpc": "2.0",
      "id": id,
      "method": method,
      "params": params
    });
    let request = req(&serde_json::to_string(&request).unwrap());
    let mut buf = vec![0; 1024];

    self.req_client
      .write_all(request.as_bytes())
      .await
      .unwrap();
    let _ = self.resp_client.read(&mut buf).await.unwrap();

    buf
  }

  pub async fn initialize(&mut self) -> Vec<u8> {
    let params = InitializeParams {
      capabilities: ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
          synchronization: Some(TextDocumentSyncClientCapabilities::default()),
          ..Default::default()
        }),
        ..Default::default()
      },
      workspace_folders: Some(vec![]),
      ..Default::default()
    };
    
    self.send_request("initialize", params).await
  }

  pub async fn request_code_action(&mut self) -> Vec<u8> {
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

    self.send_request("textDocument/codeAction", params).await
  }

  pub async fn request_execute_command(&mut self) -> Vec<u8> {
    let text_doc_item = TextDocumentItem {
      uri: Uri::from_str("file:///Users/codes/ast-grep-vscode/fixture/test.ts").unwrap(),
      language_id: "typescript".to_string(),
      version: 1,
      text: "class AstGrepTest {\n  test() {\n    console.log('Hello, world!')\n  }\n}\n\nclass AnotherCase {\n  get test2() {\n    return 123\n  }\n}\n\nconst NoProblemHere = {\n  test() {\n    if (Math.random() > 3) {\n      throw new Error('This is not an error')\n    }\n  },\n}\n".to_string(),
    };
    
    let params = ExecuteCommandParams {
      command: "ast-grep.applyAllFixes".to_string(),
      arguments: vec![serde_json::to_value(text_doc_item).unwrap()],
      work_done_progress_params: WorkDoneProgressParams::default(),
    };

    self.send_request("workspace/executeCommand", params).await
  }
}

#[test]
fn test_basic() {
  tokio::runtime::Runtime::new().unwrap().block_on(async {
    let mut lsp_client = LspClient::new();

    let buf = lsp_client.initialize().await;

    assert!(!resp(&buf).is_empty());
  });
}

#[test]
#[ignore = "fixAll conflicts with quickfix"]
fn test_code_action() {
  tokio::runtime::Runtime::new().unwrap().block_on(async {
    let mut lsp_client = LspClient::new();

    lsp_client.initialize().await;

    let buf = lsp_client.request_code_action().await;
    // {"jsonrpc":"2.0","method":"window/logMessage","params":{"message":"Running CodeAction source.fixAll","type":3}}
    let resp_list = resp(&buf);

    let running_code_action_resp = resp_list
      .iter()
      .find(|v| v["method"] == "window/logMessage")
      .unwrap();

    assert_eq!(
      running_code_action_resp["params"]["message"],
      "Running CodeAction source.fixAll"
    );
  });
}

#[test]
fn test_execute_apply_all_fixes() {
  tokio::runtime::Runtime::new().unwrap().block_on(async {
    let mut lsp_client = LspClient::new();

    lsp_client.initialize().await;

    let buf = lsp_client.request_execute_command().await;

    // {"jsonrpc":"2.0","method":"window/logMessage","params":{"message":"Running ExecuteCommand ast-grep.applyAllFixes","type":3}}
    let resp_list = resp(&buf);

    let running_command_resp = resp_list
      .iter()
      .find(|v| v["method"] == "window/logMessage")
      .unwrap();

    assert_eq!(
      running_command_resp["params"]["message"],
      "Running ExecuteCommand ast-grep.applyAllFixes"
    );
  });
}
