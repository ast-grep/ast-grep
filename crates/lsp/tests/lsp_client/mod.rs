use serde_json::Value;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::sync::mpsc;
use tower_lsp_server::lsp_types::{request::Request, notification::Notification};
use dashmap::DashMap;

mod jsonrpc;

#[derive(Debug)]
pub enum LspError {
  ChannelClosed,
}

pub struct LspStreams {
  pub request_stream: DuplexStream,
  pub response_stream: DuplexStream,
}

type Handler = Box<dyn Fn(Value) -> Value + Send + Sync>;

pub struct LspClient {
  request_sender: mpsc::UnboundedSender<String>,
  next_id: i32,
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
      
      // Check if it's a response with matching ID
      if let Some(response) = jsonrpc::try_parse_response(&message) {
        if response.id as i32 == id && response.result.is_some() {
          return serde_json::from_value(response.result.unwrap())
            .map_err(|_| LspError::ChannelClosed);
        }
      }
    }
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
    loop {
      let message = self.wait_for_message().await?;
      if let Some(request) = jsonrpc::try_parse_request(&message) {
        if request.method == R::METHOD {
          return serde_json::from_value(request.params)
            .map_err(|_| LspError::ChannelClosed); // Could add a new error variant for deserialization
        }
      }
    }
  }
}
