use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::sync::mpsc;
use tower_lsp_server::lsp_types::{request::Request, notification::Notification};

#[derive(Debug)]
pub enum LspError {
  ChannelClosed,
}

mod jsonrpc {
  use serde_json::Value;

  pub fn format_message(content: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", content.len(), content)
  }

  fn parse_message_from_string(input: &mut &str) -> Option<Value> {
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

  pub fn parse_messages_from_bytes(input: &[u8]) -> Vec<Value> {
    let mut input_str = std::str::from_utf8(input).unwrap();
    let mut messages = Vec::new();

    while let Some(message) = parse_message_from_string(&mut input_str) {
      messages.push(message);
    }
    messages
  }
}

pub struct LspStreams {
  pub request_stream: DuplexStream,
  pub response_stream: DuplexStream,
}

pub struct LspClient {
  request_sender: mpsc::UnboundedSender<String>,
  next_id: i32,
  message_receiver: mpsc::UnboundedReceiver<Value>,
}

impl LspClient {
  pub fn new(streams: LspStreams) -> Self {
    let (message_sender, message_receiver) = mpsc::unbounded_channel();
    let (request_sender, mut request_receiver) = mpsc::unbounded_channel::<String>();
    
    
    tokio::spawn(async move {
      let mut resp_client = streams.response_stream;
      let mut req_client = streams.request_stream;
      loop {
        tokio::select! {
          // Handle outgoing requests from main thread
          Some(request_str) = request_receiver.recv() => {
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
            match result {
              Ok(0) => break, // Connection closed
              Ok(_) => {
                let messages = jsonrpc::parse_messages_from_bytes(&buf);
                
                for message in messages {
                  if message_sender.send(message).is_err() {
                    break;
                  }
                }
              }
              Err(_) => break, // Read error
            }
          }
        }
      }
    });

    LspClient {
      request_sender,
      next_id: 1,
      message_receiver,
    }
  }

  fn send_message<T: serde::Serialize>(&mut self, message: T) {
    let message_json = serde_json::to_string(&message).unwrap();
    let message_str = jsonrpc::format_message(&message_json);
    self.request_sender.send(message_str).unwrap();
  }

  pub fn send_request<R: Request>(&mut self, params: R::Params) -> i32
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


  pub fn send_response<R: Request>(&mut self, id: i32, result: R::Result)
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
    self.message_receiver.recv().await.ok_or(LspError::ChannelClosed)
  }

  pub async fn wait_for_response<R: Request>(&mut self, id: i32) -> Result<R::Result, LspError>
  where
    R::Result: serde::de::DeserializeOwned,
  {
    loop {
      let message = self.wait_for_message().await?;
      
      // Check if it's a response (has id, no method) with matching ID
      if message.get("method").is_none() {
        if let Some(response_id) = message.get("id").and_then(|v| v.as_i64()).filter(|&rid| rid as i32 == id) {
          if let Some(result) = message.get("result") {
            return serde_json::from_value(result.clone())
              .map_err(|_| LspError::ChannelClosed);
          }
        }
      }
    }
  }

  pub async fn wait_for_server_request<R: Request>(&mut self) -> Result<R::Params, LspError>
  where
    R::Params: serde::de::DeserializeOwned,
  {
    loop {
      let message = self.wait_for_message().await?;
      if message.get("method").and_then(|v| v.as_str()) == Some(R::METHOD) {
        if let Some(params) = message.get("params") {
          return serde_json::from_value(params.clone())
            .map_err(|_| LspError::ChannelClosed); // Could add a new error variant for deserialization
        }
      }
    }
  }
}
