use ast_grep_config::{from_yaml_string, GlobalRules, RuleCollection, RuleConfig};
use ast_grep_language::SupportLang;
use ast_grep_lsp::*;
use serde_json::Value;
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt, DuplexStream};

pub fn req(msg: &str) -> String {
  format!("Content-Length: {}\r\n\r\n{}", msg.len(), msg)
}

// A function that takes a byte slice as input and returns the content length as an option
pub fn resp(input: &[u8]) -> Option<&str> {
  let input_str = std::str::from_utf8(input).ok()?;
  let mut splits = input_str.split("\r\n\r\n");
  let header = splits.next()?;
  let body = splits.next()?;
  let length_str = header.trim_start_matches("Content-Length: ");
  let length = length_str.parse::<usize>().ok()?;
  Some(&body[..length])
}

pub fn create_lsp() -> (DuplexStream, DuplexStream) {
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
  let rc_result: std::result::Result<_, String> = Ok(rc);
  let (service, socket) = LspService::build(|client| Backend::new(client, rc_result)).finish();
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
          "uri": "file:///Users/appe/Documents/codes/ast-grep-vscode/fixture/test.ts",
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

    dbg!(&buf);
    assert!(resp(&buf).unwrap().starts_with('{'));
  });
}

#[test]
fn test_code_action() {
  tokio::runtime::Runtime::new().unwrap().block_on(async {
    let (mut req_client, mut resp_client) = create_lsp();

    let buf = initialize_lsp(&mut req_client, &mut resp_client).await;
    assert!(resp(&buf).unwrap().starts_with('{'));

    let buf = request_code_action_to_lsp(&mut req_client, &mut resp_client).await;
    let json_val: Value = serde_json::from_str(resp(&buf).unwrap()).unwrap();
    // {"jsonrpc":"2.0","method":"window/logMessage","params":{"message":"run code action!","type":3}}
    dbg!(String::from_utf8(buf).unwrap());

    assert_eq!(json_val["method"], "window/logMessage");
    assert_eq!(
      json_val["params"]["message"],
      "Running CodeAction source.fixAll"
    );
  });
}

#[test]
fn test_execute_apply_all_fixes() {
  tokio::runtime::Runtime::new().unwrap().block_on(async {
    let (mut req_client, mut resp_client) = create_lsp();

    let buf = initialize_lsp(&mut req_client, &mut resp_client).await;
    assert!(resp(&buf).unwrap().starts_with('{'));

    let buf = request_execute_command_to_lsp(&mut req_client, &mut resp_client).await;
    // {"jsonrpc":"2.0","method":"window/logMessage","params":{"message":"Running ExecuteCommand ast-grep.applyAllFixes","type":3}}
    let json_val: Value = serde_json::from_str(resp(&buf).unwrap()).unwrap();
    dbg!(String::from_utf8(buf).unwrap());
    assert_eq!(json_val["method"], "window/logMessage");
    assert_eq!(
      json_val["params"]["message"],
      "Running ExecuteCommand ast-grep.applyAllFixes"
    );
  });
}
