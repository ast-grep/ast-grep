use dashmap::DashMap;
use serde_json::Value;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use tower_lsp_server::lsp_types::{notification::Notification, request::Request};

pub mod jsonrpc;

#[derive(Debug, Clone, Copy)]
pub enum LspError {
  ChannelClosed,
  Timeout,
}

pub struct LspStreams {
  pub request_stream: DuplexStream,
  pub response_stream: DuplexStream,
}

type Handler = Box<dyn Fn(Value) -> Value + Send + Sync>;

pub struct LspClient {
  request_sender: mpsc::UnboundedSender<String>,
  next_id: i64,
  message_receiver: mpsc::UnboundedReceiver<Value>,
  handlers: Arc<DashMap<String, Handler>>,
}

impl LspClient {
  pub fn new(streams: LspStreams) -> Self {
    let (message_sender, message_receiver) = mpsc::unbounded_channel();
    let (request_sender, mut request_receiver) = mpsc::unbounded_channel::<String>();
    let handlers = Arc::new(DashMap::<String, Handler>::new());
    let handlers_clone = handlers.clone();

    tokio::spawn(async move {
      let mut resp_client = streams.response_stream;
      let mut req_client = streams.request_stream;
      loop {
        tokio::select! {
          // Handle outgoing requests from main thread
          Some(request_str) = request_receiver.recv() => {
            // Debug log outgoing message
            let messages = jsonrpc::parse_messages_from_bytes(request_str.as_bytes());
            for message in &messages {
              log_message(message, "SENT");
            }

            if req_client.write_all(request_str.as_bytes()).await.is_err() {
              break;
            }
          }

          // Handle incoming responses from server
          read_result = async {
            let mut buf = vec![0; 1024];
            let result = resp_client.read(&mut buf).await;
            (result, buf)
          } => {
            let (result, buf) = read_result;
            if let Ok(0) | Err(_) = result {
              break;
            }

            let messages = jsonrpc::parse_messages_from_bytes(&buf);

            for message in messages {
              // Debug log incoming message
              log_message(&message, "RECV");

              // Check if it's a server request and try to handle it
              if let Some(request) = jsonrpc::try_parse_request(&message) {
                if let Some(handler) = handlers_clone.get(&request.method) {
                  let result = handler(request.params);
                  let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.id,
                    "result": result
                  });
                  let response_json = serde_json::to_string(&response).unwrap();
                  let response_str = jsonrpc::format_message(&response_json);

                  // Debug log outgoing response
                  log_handler_response(request.id);

                  if req_client.write_all(response_str.as_bytes()).await.is_err() {
                    break;
                  }
                  continue; // Don't forward handled requests to main thread
                }
              }

              // Forward unhandled messages to main thread
              if message_sender.send(message).is_err() {
                break;
              }
            }
          }
        }
      }
    });

    LspClient {
      request_sender,
      next_id: 1,
      message_receiver,
      handlers,
    }
  }

  fn send_message<T: serde::Serialize>(&mut self, message: T) {
    let message_json = serde_json::to_string(&message).unwrap();
    let message_str = jsonrpc::format_message(&message_json);
    self.request_sender.send(message_str).unwrap();
  }

  pub fn send_request<R: Request>(&mut self, params: R::Params) -> i64
  where
    R::Params: serde::Serialize,
  {
    let id = self.next_id;
    self.next_id += 1;

    let request = serde_json::json!({
      "jsonrpc": "2.0",
      "id": id,
      "method": R::METHOD,
      "params": params
    });

    self.send_message(request);
    id
  }

  pub fn send_response<R: Request>(&mut self, id: i64, result: R::Result)
  where
    R::Result: serde::Serialize,
  {
    let response = serde_json::json!({
      "jsonrpc": "2.0",
      "id": id,
      "result": result
    });

    self.send_message(response);
  }

  pub fn send_notification<N: Notification>(&mut self, params: N::Params)
  where
    N::Params: serde::Serialize,
  {
    let notification = serde_json::json!({
      "jsonrpc": "2.0",
      "method": N::METHOD,
      "params": params
    });

    self.send_message(notification);
  }

  pub async fn wait_for_message(&mut self) -> Result<Value, LspError> {
    timeout(Duration::from_secs(10), self.message_receiver.recv())
      .await
      .map_err(|_| LspError::Timeout)?
      .ok_or(LspError::ChannelClosed)
  }

  pub async fn wait_for_response<R: Request>(&mut self, id: i64) -> Result<R::Result, LspError>
  where
    R::Result: serde::de::DeserializeOwned,
  {
    timeout(Duration::from_secs(10), async {
      loop {
        let message = self.wait_for_message().await?;

        // Check if it's a response with matching ID
        if let Some(response) =
          jsonrpc::try_parse_response(&message).filter(|r| r.id == id && r.result.is_some())
        {
          return serde_json::from_value(response.result.unwrap())
            .map_err(|_| LspError::ChannelClosed);
        }
      }
    })
    .await
    .map_err(|_| LspError::Timeout)?
  }

  pub fn add_handler<R: Request, F>(&mut self, handler: F)
  where
    R::Params: serde::de::DeserializeOwned,
    R::Result: serde::Serialize,
    F: Fn(R::Params) -> R::Result + Send + Sync + 'static,
  {
    let wrapper = Box::new(move |params_value: Value| -> Value {
      let params: R::Params = serde_json::from_value(params_value).unwrap();
      let result = handler(params);
      serde_json::to_value(result).unwrap()
    });
    self.handlers.insert(R::METHOD.to_string(), wrapper);
  }

  pub async fn wait_for_server_request<R: Request>(&mut self) -> Result<R::Params, LspError>
  where
    R::Params: serde::de::DeserializeOwned,
  {
    timeout(Duration::from_secs(10), async {
      loop {
        let message = self.wait_for_message().await?;
        if let Some(request) =
          jsonrpc::try_parse_request(&message).filter(|req| req.method == R::METHOD)
        {
          return serde_json::from_value(request.params).map_err(|_| LspError::ChannelClosed);
          // Could add a new error variant for deserialization
        }
      }
    })
    .await
    .map_err(|_| LspError::Timeout)?
  }
}

fn log_message(message: &Value, direction: &str) {
  if cfg!(test) {
    let message_type = if jsonrpc::is_request(message) {
      "request"
    } else if jsonrpc::is_response(message) {
      "response"
    } else {
      "notification"
    };
    eprintln!(
      "LSP_CLIENT {} {}: {} (id: {:?})",
      direction,
      message_type,
      message
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("N/A"),
      message.get("id").and_then(|v| v.as_i64())
    );
  }
}

fn log_handler_response(id: i64) {
  if cfg!(test) {
    eprintln!("LSP_CLIENT SENT response: N/A (id: {})", id);
  }
}
