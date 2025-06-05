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

pub fn is_request(message: &Value) -> bool {
  message.get("method").is_some() && message.get("id").is_some()
}

pub fn is_response(message: &Value) -> bool {
  message.get("method").is_none() && message.get("id").is_some()
}

pub fn is_notification(message: &Value) -> bool {
  message.get("method").is_some() && message.get("id").is_none()
}

pub struct JsonRpcRequest {
  pub id: i64,
  pub method: String,
  pub params: Value,
}

pub struct JsonRpcResponse {
  pub id: i64,
  pub result: Option<Value>,
  pub error: Option<Value>,
}

pub struct JsonRpcNotification {
  pub method: String,
  pub params: Value,
}

pub fn try_parse_request(message: &Value) -> Option<JsonRpcRequest> {
  if !is_request(message) {
    return None;
  }

  let id = message.get("id")?.as_i64()?;
  let method = message.get("method")?.as_str()?.to_string();
  let params = message.get("params").cloned().unwrap_or(Value::Null);

  Some(JsonRpcRequest { id, method, params })
}

pub fn try_parse_response(message: &Value) -> Option<JsonRpcResponse> {
  if !is_response(message) {
    return None;
  }

  let id = message.get("id")?.as_i64()?;
  let result = message.get("result").cloned();
  let error = message.get("error").cloned();

  Some(JsonRpcResponse { id, result, error })
}

pub fn try_parse_notification(message: &Value) -> Option<JsonRpcNotification> {
  if !is_notification(message) {
    return None;
  }

  let method = message.get("method")?.as_str()?.to_string();
  let params = message.get("params").cloned().unwrap_or(Value::Null);

  Some(JsonRpcNotification { method, params })
}
