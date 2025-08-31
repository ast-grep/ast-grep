use ast_grep_config::{from_yaml_string, GlobalRules, RuleCollection, RuleConfig};
use ast_grep_language::SupportLang;
use ast_grep_lsp::*;
use serde_json::Value;
use tokio::io::{duplex, split, AsyncReadExt, AsyncWriteExt, DuplexStream};
use futures::{SinkExt, StreamExt};
use std::io;
use std::path::Path;
use tokio_util::bytes::{BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder, Framed};


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

pub fn create_lsp() -> (DuplexStream, DuplexStream) {
  let base = Path::new("./").to_path_buf();

  // Create a rule finder closure that builds the rule collection from scratch
  let rule_finder = move || {
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
    let rc: RuleCollection<SupportLang> = RuleCollection::try_new(vec![config]).unwrap();
    Ok(rc)
  };

  let (service, socket) =
    LspService::build(|client| Backend::new(client, base, rule_finder)).finish();
  let (req_client, req_server) = duplex(1024);
  let (resp_server, resp_client) = duplex(1024);

  // start server as concurrent task
  tokio::spawn(Server::new(req_server, resp_server, socket).serve(service));

  (req_client, resp_client)
}

pub async fn initialize_lsp(
  req_client: &mut DuplexStream,
  resp_client: &mut DuplexStream,
) -> Vec<u8> {
  let initialize = r#"{
      "jsonrpc":"2.0",
      "id": 1,
      "method": "initialize",
      "params": {
        "capabilities": {
          "textDocumentSync": 1
        }
      }
    }"#;
  let mut buf = vec![0; 1024];

  req_client
    .write_all(req(initialize).as_bytes())
    .await
    .unwrap();
  let _ = resp_client.read(&mut buf).await.unwrap();

  buf
}

pub async fn request_code_action_to_lsp(
  req_client: &mut DuplexStream,
  resp_client: &mut DuplexStream,
) -> Vec<u8> {
  let code_action_request = r#"{
      "jsonrpc": "2.0",
      "id": 1,
      "method": "textDocument/codeAction",
      "params": {
        "range": {
          "end": {
            "character": 10,
            "line": 1
          },
          "start": {
            "character": 10,
            "line": 1
          }
        },
        "textDocument": {
          "uri": "file:///Users/codes/ast-grep-vscode/test.tsx"
        },
        "context": {
          "diagnostics": [
            {
              "range": {
                "start": {
                  "line": 0,
                  "character": 0
                },
                "end": {
                  "line": 0,
                  "character": 16
                }
              },
              "code": "no-console-rule",
              "source": "ast-grep",
              "message": "No console.log"
            }
          ],
          "only": ["source.fixAll"]
        }
      }
      }"#;

  let mut buf = vec![0; 1024];
  req_client
    .write_all(req(code_action_request).as_bytes())
    .await
    .unwrap();
  let _ = resp_client.read(&mut buf).await.unwrap();

  buf
}

pub async fn request_execute_command_to_lsp(
  req_client: &mut DuplexStream,
  resp_client: &mut DuplexStream,
) -> Vec<u8> {
  let execute_command_request: &str = r#"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "workspace/executeCommand",
    "params": {
      "command": "ast-grep.applyAllFixes",
      "arguments": [
        {
          "text": "class AstGrepTest {\n  test() {\n    console.log('Hello, world!')\n  }\n}\n\nclass AnotherCase {\n  get test2() {\n    return 123\n  }\n}\n\nconst NoProblemHere = {\n  test() {\n    if (Math.random() > 3) {\n      throw new Error('This is not an error')\n    }\n  },\n}\n",
          "uri": "file:///Users/codes/ast-grep-vscode/fixture/test.ts",
          "version": 1,
          "languageId": "typescript"
        }
      ]
    }
  }
  "#;
  let mut buf = vec![0; 1024];
  req_client
    .write_all(req(execute_command_request).as_bytes())
    .await
    .unwrap();
  let _ = resp_client.read(&mut buf).await.unwrap();

  buf
}

#[test]
fn test_basic() {
  tokio::runtime::Runtime::new().unwrap().block_on(async {
    let (mut req_client, mut resp_client) = create_lsp();

    let buf = initialize_lsp(&mut req_client, &mut resp_client).await;

    assert!(!resp(&buf).is_empty());
  });
}

#[test]
#[ignore = "fixAll conflicts with quickfix"]
fn test_code_action() {
  tokio::runtime::Runtime::new().unwrap().block_on(async {
    let (mut req_client, mut resp_client) = create_lsp();

    initialize_lsp(&mut req_client, &mut resp_client).await;

    let buf = request_code_action_to_lsp(&mut req_client, &mut resp_client).await;
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
    let (mut req_client, mut resp_client) = create_lsp();

    initialize_lsp(&mut req_client, &mut resp_client).await;

    let buf = request_execute_command_to_lsp(&mut req_client, &mut resp_client).await;

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

#[tokio::test]
async fn test_file_watcher_registration() {
  let (mut req_client, mut resp_client) = create_lsp();
  let initialize = r#"{
      "jsonrpc":"2.0",
      "id": 1,
      "method": "initialize",
      "params": {
        "capabilities": {
          "workspace": {
            "didChangeWatchedFiles": {
              "dynamicRegistration": true
            }
          }
        }
      }
    }"#;

  // Send initialize request
  req_client
    .write_all(req(initialize).as_bytes())
    .await
    .unwrap();

  let mut buf = vec![0; 4096];
  let len = resp_client.read(&mut buf).await.unwrap();
  let response = String::from_utf8_lossy(&buf[..len]);

  // Should contain initialization response
  assert!(response.contains("result") || response.contains("initialize"));

  // Send initialized notification
  let initialized = r#"{
      "jsonrpc":"2.0",
      "method": "initialized",
      "params": {}
    }"#;

  req_client
    .write_all(req(initialized).as_bytes())
    .await
    .unwrap();

  // Read responses - there should be file watcher registration
  let mut buf = vec![0; 4096];
  let len = resp_client.read(&mut buf).await.unwrap();
  let response = String::from_utf8_lossy(&buf[..len]);

  // Should contain capability registration for file watching
  assert!(
    response.contains("client/registerCapability")
      || response.contains("workspace/didChangeWatchedFiles")
      || response.contains("window/logMessage")
  );
}

#[tokio::test]
async fn test_did_change_watched_files() {
  let (mut req_client, mut resp_client) = create_lsp();
  initialize_lsp(&mut req_client, &mut resp_client).await;

  // Send didChangeWatchedFiles notification
  let change_notification = r#"{
      "jsonrpc":"2.0",
      "method": "workspace/didChangeWatchedFiles",
      "params": {
        "changes": [
          {
            "uri": "file:///test/sgconfig.yml",
            "type": 2
          }
        ]
      }
    }"#;

  req_client
    .write_all(req(change_notification).as_bytes())
    .await
    .unwrap();

  // Give some time for processing
  tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

  let mut buf = vec![0; 4096];
  let len = resp_client.read(&mut buf).await.unwrap();
  let response = String::from_utf8_lossy(&buf[..len]);

  // Should contain log messages about configuration changes
  assert!(
    response.contains("Configuration files changed")
      || response.contains("watched files have changed")
  );
}


// Helper: send_did_open_framed
pub async fn send_did_open_framed(
  framed: &mut Framed<DuplexStream, LspCodec>,
  uri: &str,
  language_id: &str,
  text: &str,
) {
  let did_open = serde_json::json!({
    "jsonrpc": "2.0",
    "method": "textDocument/didOpen",
    "params": {
      "textDocument": {
        "uri": uri,
        "languageId": language_id,
        "version": 1,
        "text": text
      }
    }
  });
  framed.send(did_open).await.unwrap();
}

pub async fn wait_for_diagnostics(
  sender: &mut Framed<DuplexStream, LspCodec>,
) -> Option<serde_json::Value> {
  // Wait for diagnostics
  let mut diagnostics: Option<serde_json::Value> = None;
  for _ in 0..20 {
    match tokio::time::timeout(std::time::Duration::from_secs(2), sender.next()).await {
      Ok(Some(Ok(msg))) => {
        if msg.get("method") == Some(&serde_json::json!("textDocument/publishDiagnostics")) {
          diagnostics = Some(msg["params"]["diagnostics"].clone());
          break;
        } else if msg.get("method") == Some(&serde_json::json!("workspace/workspaceFolders")) {
          // Respond with empty workspaceFolders
          let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": msg["id"].clone(),
            "result": [{
              "uri": "file:///Users/codes/ast-grep-vscode",
              "name": "ast-grep-vscode"
            }]
          });
          sender.send(response).await.unwrap();
        }
      }
      _ => {}
    }
  }

  return diagnostics;
}

pub async fn request_code_action(
  sender: &mut Framed<DuplexStream, LspCodec>,
  file_uri: &str,
  diagnostic: &serde_json::Value,
) -> Option<serde_json::Value> {
  let code_action_request = serde_json::json!({
    "jsonrpc": "2.0",
    "id": 1,
    "method": "textDocument/codeAction",
    "params": {
      "range": diagnostic["range"].clone(),
      "textDocument": { "uri": file_uri },
      "context": {
        "diagnostics": [diagnostic.clone()]
      }
    }
  });
  sender.send(code_action_request).await.unwrap();
  for _ in 0..20 {
    match tokio::time::timeout(std::time::Duration::from_secs(2), sender.next()).await {
      Ok(Some(Ok(msg))) => {
        if msg.get("id") == Some(&serde_json::json!(1)) {
          return Some(msg);
        }
      }
      _ => {}
    }
  }
  None
}

#[tokio::test]
async fn test_code_action_data_support_enabled() {
  let mut client = create_lsp_framed();
  // Initialize with data_support enabled
  let initialize = serde_json::json!({
      "jsonrpc": "2.0",
      "id": 1,
      "method": "initialize",
      "params": {
          "capabilities": {
              "textDocument": {
                  "publishDiagnostics": {
                      "dataSupport": true
                  }
              }
          }
      }
  });
  client.send(initialize).await.unwrap();
  // Wait for initialize response
  for _ in 0..10 {
    if let Some(Ok(msg)) = client.next().await {
      if msg.get("id") == Some(&serde_json::json!(1)) {
        // Send 'initialized' notification after receiving 'initialize' response
        let initialized = serde_json::json!({
          "jsonrpc": "2.0",
          "method": "initialized",
          "params": {}
        });
        client.send(initialized).await.unwrap();
        break;
      }
    }
  }
  // Send file content to server
  let file_uri = "file:///Users/codes/ast-grep-vscode/test.ts";
  let file_content = "console.log('Hello, world!')\n";
  send_did_open_framed(&mut client, file_uri, "typescript", file_content).await;
  tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

  let diagnostics = wait_for_diagnostics(&mut client).await;

  let diagnostics = diagnostics.expect("No diagnostics received");
  let diagnostic = diagnostics
    .as_array()
    .cloned()
    .and_then(|arr| arr.get(0).cloned())
    .unwrap();

  let code_action = request_code_action(&mut client, file_uri, &diagnostic).await;

  // Request code action using diagnostics from server
  let code_action = code_action.expect("No code action response");
  assert!(
    code_action["result"].as_array().unwrap().iter().len() > 0,
    "No code actions returned"
  );
}

#[tokio::test]
async fn test_code_action_data_support_disabled() {
  let mut client = create_lsp_framed();
  // Initialize with data_support disabled
  let initialize = serde_json::json!({
      "jsonrpc": "2.0",
      "id": 1,
      "method": "initialize",
      "params": {
          "capabilities": {
          }
      }
  });
  client.send(initialize).await.unwrap();
  // Wait for initialize response
  for _ in 0..10 {
    if let Some(Ok(msg)) = client.next().await {
      if msg.get("id") == Some(&serde_json::json!(1)) {
        // Send 'initialized' notification after receiving 'initialize' response
        let initialized = serde_json::json!({
          "jsonrpc": "2.0",
          "method": "initialized",
          "params": {}
        });
        client.send(initialized).await.unwrap();
        break;
      }
    }
  }
  // Send file content to server
  let file_uri = "file:///Users/codes/ast-grep-vscode/test.ts";
  let file_content = "console.log('Hello, world!')\n";
  send_did_open_framed(&mut client, file_uri, "typescript", file_content).await;
  tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

  let diagnostics = wait_for_diagnostics(&mut client).await;

  let diagnostics = diagnostics.expect("No diagnostics received");

  let mut diagnostic = diagnostics
    .as_array()
    .cloned()
    .and_then(|arr| arr.get(0).cloned())
    .unwrap();

  // Remove 'data' field to simulate client without dataSupport
  diagnostic.as_object_mut().unwrap().remove("data");
  let code_action = request_code_action(&mut client, file_uri, &diagnostic).await;

  let code_action = code_action.expect("No code action response");
  assert!(
    code_action["result"].as_array().unwrap().iter().len() > 0,
    "No code actions returned"
  );
}

// Custom LSP Codec for Content-Length framed JSON-RPC
#[derive(Default)]
pub struct LspCodec;

impl Decoder for LspCodec {
  type Item = serde_json::Value;
  type Error = io::Error;

  fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
    let src_str =
      std::str::from_utf8(&src[..]).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let header = "Content-Length: ";
    let header_pos = src_str.find(header);
    if let Some(pos) = header_pos {
      let rest = &src_str[pos + header.len()..];
      let crlf = rest.find("\r\n\r\n");
      if let Some(crlf_pos) = crlf {
        let len_str = &rest[..crlf_pos];
        let content_len: usize = len_str
          .trim()
          .parse()
          .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let body_start = pos + header.len() + crlf_pos + 4;
        if src.len() >= body_start + content_len {
          let json_bytes = &src[body_start..body_start + content_len];
          let value = serde_json::from_slice(json_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
          // Remove processed bytes manually
          let _ = src.split_to(body_start + content_len);
          return Ok(Some(value));
        }
      }
    }
    Ok(None)
  }
}

impl Encoder<serde_json::Value> for LspCodec {
  type Error = io::Error;

  fn encode(&mut self, item: serde_json::Value, dst: &mut BytesMut) -> Result<(), Self::Error> {
    let json =
      serde_json::to_string(&item).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let header = format!("Content-Length: {}\r\n\r\n", json.len());
    dst.put(header.as_bytes());
    dst.put(json.as_bytes());
    Ok(())
  }
}

pub fn create_lsp_framed() -> Framed<DuplexStream, LspCodec> {
  let base = Path::new("./").to_path_buf();
  let rule_finder = move || {
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
    let rc: RuleCollection<SupportLang> = RuleCollection::try_new(vec![config]).unwrap();
    Ok(rc)
  };
  let (service, socket) =
    LspService::build(|client| Backend::new(client, base, rule_finder)).finish();
  let (client_write, server_read) = duplex(16384);
  //let (server_write, client_read) = duplex(16384);
  let (r, w) = split(server_read);
  tokio::spawn(Server::new(r, w, socket).serve(service));

  Framed::new(client_write, LspCodec::default())
}

#[test]
pub fn test_framed_codec() {
  let mut codec = LspCodec::default();
  let mut buf = BytesMut::new();
  let msg = serde_json::json!({
    "jsonrpc": "2.0",
    "method": "testMethod",
    "params": {
      "key": "value"
    }
  });
  codec.encode(msg.clone(), &mut buf).unwrap();
  let decoded = codec.decode(&mut buf).unwrap().unwrap();
  assert_eq!(decoded, msg);
} 