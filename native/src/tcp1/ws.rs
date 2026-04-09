// MIT LICENSE
//
// COPYRIGHT (R) 2025 ARNELIFY. AUTHOR: TARON SARKISYAN
//
// PERMISSION IS HEREBY GRANTED, FREE OF CHARGE, TO ANY PERSON OBTAINING A COPY
// OF THIS SOFTWARE AND ASSOCIATED DOCUMENTATION FILES (THE "SOFTWARE"), TO DEAL
// IN THE SOFTWARE WITHOUT RESTRICTION, INCLUDING WITHOUT LIMITATION THE RIGHTS
// TO USE, COPY, MODIFY, MERGE, PUBLISH, DISTRIBUTE, SUBLICENSE, AND/OR SELL
// COPIES OF THE SOFTWARE, AND TO PERMIT PERSONS TO WHOM THE SOFTWARE IS
// FURNISHED TO DO SO, SUBJECT TO THE FOLLOWING CONDITIONS:
//
// THE ABOVE COPYRIGHT NOTICE AND THIS PERMISSION NOTICE SHALL BE INCLUDED IN ALL
// COPIES OR SUBSTANTIAL PORTIONS OF THE SOFTWARE.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use std::{
  collections::HashMap,
  io::{Error, ErrorKind},
  process,
  sync::{
    Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard,
    atomic::{AtomicU64, Ordering},
  },
  time::{SystemTime, UNIX_EPOCH},
};

use tokio::{
  net::{TcpListener, TcpStream},
  runtime::{Builder, Runtime},
  sync::{Notify, mpsc},
  task::JoinHandle,
  time::{Duration, timeout},
};

use tokio_tungstenite::{
  WebSocketStream as Stream,
  tungstenite::{Message, handshake, protocol},
};

pub type WebSocketCtx = serde_json::Value;
pub type WebSocketBytes = Vec<u8>;
pub type JSON = serde_json::Value;

#[derive(Clone, Default)]
pub struct WebSocketOpts {
  pub block_size_kb: usize,
  pub compression: bool,
  pub max_message_size_kb: u64,
  pub ping_timeout: u64,
  pub port: u16,
  pub rate_limit: u64,
  pub read_timeout: u64,
  pub send_timeout: u64,
  pub thread_limit: u64,
}

struct WebSocketReq {
  opts: WebSocketOpts,

  has_body: bool,
  has_headers: bool,
  has_method: bool,
  has_path: bool,
  has_protocol: bool,
  has_meta: bool,

  topic: String,
  buff: Vec<u8>,
  binary: Vec<u8>,
  ip: Option<String>,

  compression: Option<String>,
  binary_length: usize,
  json_length: usize,

  ctx: WebSocketCtx,
}

impl WebSocketReq {
  pub fn new(opts: WebSocketOpts) -> Self {
    Self {
      opts,
      has_body: false,
      has_headers: false,
      has_method: false,
      has_path: false,
      has_protocol: false,
      has_meta: false,

      topic: String::new(),
      buff: Vec::new(),
      binary: Vec::new(),
      ip: None,

      compression: None,
      binary_length: 0,
      json_length: 0,

      ctx: serde_json::json!({
        "_state": {
          "addr": JSON::Null,
          "cookie": {},
          "headers": {},
          "method": "GET",
          "path": "/",
          "protocol": "WebSocket",
          "topic": "_"
        },
        "params": {
          "files": {},
          "body": {},
          "query": {}
        }
      }),
    }
  }

  pub fn add(&mut self, block: &[u8]) -> () {
    self.buff.extend_from_slice(block);
  }

  fn decode(&self, encoded: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(encoded.len());
    let mut i = 0;

    while i < encoded.len() {
      match encoded[i] {
        b'%' => {
          if i + 2 < encoded.len() {
            let hex = &encoded[i + 1..i + 3];
            if let Ok(hex_str) = std::str::from_utf8(hex) {
              if let Ok(byte) = u8::from_str_radix(hex_str, 16) {
                decoded.push(byte);
                i += 3;
                continue;
              }
            }
          }

          decoded.push(b'%');
          i += 1;
        }

        b'+' => {
          decoded.push(b' ');
          i += 1;
        }

        byte => {
          decoded.push(byte);
          i += 1;
        }
      }
    }

    decoded
  }

  pub fn get_bytes(&self) -> Vec<u8> {
    self.binary.to_vec()
  }

  pub fn get_compression(&self) -> Option<String> {
    self.compression.clone()
  }

  pub fn get_ctx(&self) -> WebSocketCtx {
    self.ctx.clone()
  }

  pub fn get_ip(&self) -> String {
    if let Some(ip) = &self.ip {
      return ip.clone();
    }

    String::from("_")
  }

  pub fn get_topic(&self) -> String {
    self.topic.clone()
  }

  fn set_cookie(&mut self, value: &str) -> Result<u8, Error> {
    let mut is_first: bool = true;

    for param in value.split(';') {
      if let Some(equal_start) = param.find('=') {
        if is_first {
          let name: &str = param[..equal_start].trim();
          let data: &str = param[equal_start + 1..].trim();
          self.ctx["_state"]["cookie"][name] = JSON::String(data.to_string());
          is_first = false;
          continue;
        }

        let name: &str = param[1..equal_start].trim();
        let data: &str = param[equal_start + 1..].trim();
        self.ctx["_state"]["cookie"][name] = JSON::String(data.to_string());
      }
    }

    Ok(1)
  }

  fn set_header(&mut self, key: String, value: String) -> Result<u8, Error> {
    self.ctx["_state"]["headers"][&key] = JSON::String(value.to_string());

    if key.eq_ignore_ascii_case("Cookie") {
      return self.set_cookie(&value);
    }

    Ok(1)
  }

  fn read_meta(&mut self, meta_end: usize) -> Result<u8, Error> {
    let meta_bytes: &[u8] = &self.buff[..meta_end];
    let pos = match meta_bytes.iter().position(|&b| b == b'+') {
      Some(v) => v,
      None => return Err(Error::new(ErrorKind::InvalidData, "Missing '+' in meta")),
    };

    let json_length: &str = match std::str::from_utf8(&meta_bytes[..pos]) {
      Ok(s) => s,
      Err(_) => {
        return Err(Error::new(ErrorKind::InvalidData, "Invalid UTF-8."));
      }
    };

    self.json_length = match json_length.parse() {
      Ok(n) => n,
      Err(_) => return Err(Error::new(ErrorKind::InvalidData, "Invalid meta.")),
    };

    let binary_length: &str = match std::str::from_utf8(&meta_bytes[pos + 1..]) {
      Ok(s) => s,
      Err(_) => {
        return Err(Error::new(ErrorKind::InvalidData, "Invalid UTF-8."));
      }
    };

    self.binary_length = match binary_length.parse() {
      Ok(n) => n,
      Err(_) => return Err(Error::new(ErrorKind::InvalidData, "Invalid meta.")),
    };

    let max_message_size: usize = (self.opts.max_message_size_kb * 1024) as usize;
    if self.json_length + self.binary_length > max_message_size {
      self.buff.clear();
      return Err(Error::new(
        ErrorKind::InvalidData,
        "The maximum size of the message has been exceeded.",
      ));
    }

    Ok(1)
  }

  pub fn set_addr(&mut self, addr: &str) -> () {
    self.ctx["_state"]["addr"] = JSON::String(String::from(addr));
  }

  fn read_body(&mut self) -> Result<Option<u8>, Error> {
    if !self.has_meta {
      let meta_end: usize = match self.buff.iter().position(|&b| b == b':') {
        Some(pos) => pos,
        None => {
          if self.buff.len() > 8192 {
            self.buff.clear();
            return Err(Error::new(
              ErrorKind::InvalidData,
              "The maximum size of the meta has been exceeded.",
            ));
          }

          return Ok(None);
        }
      };

      match self.read_meta(meta_end) {
        Ok(_) => {}
        Err(e) => {
          self.buff.clear();
          return Err(e);
        }
      }

      self.has_meta = true;
      self.buff.drain(..=meta_end);
    }

    if self.json_length != 0 && self.buff.len() >= self.json_length {
      let ctx: &str = match std::str::from_utf8(&self.buff[..self.json_length]) {
        Ok(v) => v,
        Err(_) => {
          self.buff.clear();
          return Err(Error::new(ErrorKind::InvalidInput, "Invalid UTF-8."));
        }
      };

      let json: serde_json::Value = match serde_json::from_str(ctx) {
        Ok(v) => v,
        Err(_) => {
          self.buff.clear();
          return Err(Error::new(ErrorKind::InvalidInput, "Invalid JSON."));
        }
      };

      if json.get("topic").is_none() || json.get("payload").is_none() {
        return Err(Error::new(ErrorKind::InvalidInput, "Invalid message."));
      }

      self.topic = String::from(json["topic"].as_str().unwrap_or(""));
      self.ctx["_state"]["topic"] = json["topic"].clone();

      if !json["payload"].is_object() {
        self.buff.clear();
        return Err(Error::new(
          ErrorKind::InvalidInput,
          "Invalid application/json.",
        ));
      }

      self.ctx["params"]["body"] = json["payload"].clone();

      self.buff.drain(..self.json_length);
      if self.binary_length == 0 {
        self.has_body = true;
        return Ok(Some(1));
      }
    }

    if self.binary_length != 0 && self.buff.len() >= self.binary_length {
      self.binary = self.buff[..self.binary_length].to_vec();

      self.has_body = true;
      self.buff.drain(..self.binary_length);
      return Ok(Some(1));
    }

    Ok(None)
  }

  fn read_method(&mut self) -> Result<Option<u8>, Error> {
    let method_end: Option<usize> = self.buff.iter().position(|&b| b == b' ');
    if method_end.is_none() {
      if self.buff.len() > 8 {
        self.buff.clear();
        return Err(Error::new(
          ErrorKind::InvalidData,
          "The maximum size of the method has been exceeded.",
        ));
      }

      return Ok(None);
    }

    let method_end: usize = method_end.unwrap();
    if method_end > 8 {
      self.buff.clear();
      return Err(Error::new(
        ErrorKind::InvalidData,
        "The maximum size of the method has been exceeded.",
      ));
    }

    let method_bytes: &[u8] = &self.buff[..method_end];
    let is_support: bool = method_bytes == b"GET";

    if !is_support {
      self.buff.clear();
      return Err(Error::new(
        ErrorKind::InvalidData,
        "Unknown request method.",
      ));
    }

    let method: &str = match std::str::from_utf8(&method_bytes) {
      Ok(v) => v,
      Err(_) => {
        self.buff.clear();
        return Err(Error::new(ErrorKind::InvalidData, "Invalid UTF-8."));
      }
    };

    self.ctx["_state"]["method"] = JSON::String(method.to_string());
    self.has_method = true;
    self.has_body = false;

    self.buff.drain(..=method_end);
    Ok(Some(1))
  }

  fn read_path(&mut self) -> Result<Option<u8>, Error> {
    let path_end: usize = match self.buff.iter().position(|&b| b == b' ') {
      Some(pos) => pos,
      None => {
        if self.buff.len() > 2048 {
          self.buff.clear();
          return Err(Error::new(
            ErrorKind::InvalidData,
            "The maximum size of the URL has been exceeded.",
          ));
        }
        return Ok(None);
      }
    };

    if path_end > 2048 {
      self.buff.clear();
      return Err(Error::new(
        ErrorKind::InvalidData,
        "The maximum size of the URL has been exceeded.",
      ));
    }

    let location_bytes: &[u8] = &self.buff[..path_end];
    let query_pos: Option<usize> = location_bytes.iter().position(|&b| b == b'?');

    let (path_encoded, _query) = match query_pos {
      Some(q) => (&location_bytes[..q], Some(&location_bytes[q + 1..])),
      None => (location_bytes, None),
    };

    let path_bytes: Vec<u8> = self.decode(path_encoded);
    let path: &str = match std::str::from_utf8(&path_bytes) {
      Ok(v) => v,
      Err(_) => {
        self.buff.clear();
        return Err(Error::new(ErrorKind::InvalidData, "Invalid UTF-8."));
      }
    };

    if path.contains("..") || path.contains("\0") {
      self.ctx["_state"]["path"] = JSON::String(String::from("_"));
    } else {
      self.ctx["_state"]["path"] = JSON::String(String::from(path));
    }

    self.has_path = true;

    self.buff.drain(..=path_end);
    Ok(Some(1))
  }

  fn read_protocol(&mut self) -> Result<Option<u8>, Error> {
    let protocol_end: usize = match self.buff.windows(2).position(|w| w == b"\r\n") {
      Some(pos) => pos,
      None => {
        if self.buff.len() > 12 {
          self.buff.clear();
          return Err(Error::new(
            ErrorKind::InvalidData,
            "The maximum size of the protocol type has been exceeded.",
          ));
        }
        return Ok(None);
      }
    };

    if protocol_end > 12 {
      self.buff.clear();
      return Err(Error::new(
        ErrorKind::InvalidData,
        "The maximum size of the protocol type has been exceeded.",
      ));
    }

    self.has_protocol = true;

    self.buff.drain(..protocol_end + 2);
    Ok(Some(1))
  }

  fn read_headers(&mut self) -> Result<Option<u8>, Error> {
    let headers_end: usize = match self.buff.windows(4).position(|w| w == b"\r\n\r\n") {
      Some(pos) => pos,
      None => {
        if self.buff.len() > 8192 {
          self.buff.clear();
          return Err(Error::new(
            ErrorKind::InvalidData,
            "The maximum size of headers has been exceeded.",
          ));
        }
        return Ok(None);
      }
    };

    if headers_end > 8192 {
      self.buff.clear();
      return Err(Error::new(
        ErrorKind::InvalidData,
        "The maximum size of headers has been exceeded.",
      ));
    }

    let mut start: usize = 0;
    while start < headers_end {
      if let Some(pos) = &self.buff[start..headers_end + 2]
        .windows(2)
        .position(|w| w == b"\r\n")
      {
        let line_bytes: &[u8] = &self.buff[start..start + pos];
        if let Some(pos) = line_bytes.iter().position(|&b| b == b':') {
          let key_bytes: &[u8] = &line_bytes[..pos];
          let key: &str = match std::str::from_utf8(&key_bytes) {
            Ok(v) => v.trim(),
            Err(_) => {
              self.buff.clear();
              return Err(Error::new(ErrorKind::InvalidData, "Invalid UTF-8."));
            }
          };

          let value_bytes: &[u8] = &line_bytes[pos + 1..];
          let value: &str = match std::str::from_utf8(&value_bytes) {
            Ok(v) => v.trim(),
            Err(_) => {
              self.buff.clear();
              return Err(Error::new(ErrorKind::InvalidData, "Invalid UTF-8."));
            }
          };

          if key == "X-Forwarded-For" {
            self.ip = Some(value.to_string());
          }

          if self.ip.is_none() && key == "X-Real-IP" {
            self.ip = Some(value.to_string());
          }

          if self.ip.is_none() && key == "CF-Connecting-IP" {
            self.ip = Some(value.to_string());
          }

          if self.ip.is_none() && key == "CF-Connecting-IPv6" {
            self.ip = Some(value.to_string());
          }

          self.set_header(key.to_string(), value.to_string())?;
        }

        start += pos + 2;
        continue;
      }

      break;
    }

    self.has_headers = true;
    self.buff.drain(..headers_end + 4);
    Ok(Some(1))
  }

  pub fn read_block(&mut self) -> Result<Option<u8>, Error> {
    if !self.has_method {
      match self.read_method() {
        Ok(Some(_)) => {}
        Ok(None) => return Ok(None),
        Err(e) => return Err(e),
      }
    }

    if !self.has_path {
      match self.read_path() {
        Ok(Some(_)) => {}
        Ok(None) => return Ok(None),
        Err(e) => return Err(e),
      }
    }

    if !self.has_protocol {
      match self.read_protocol() {
        Ok(Some(_)) => {}
        Ok(None) => return Ok(None),
        Err(e) => return Err(e),
      }
    }

    if !self.has_headers {
      match self.read_headers() {
        Ok(Some(_)) => {}
        Ok(None) => return Ok(None),
        Err(e) => return Err(e),
      }
    }

    if !self.has_body {
      match self.read_body() {
        Ok(Some(_)) => {}
        Ok(None) => return Ok(None),
        Err(e) => return Err(e),
      }
    }

    Ok(Some(1))
  }

  pub fn reset(&mut self) -> () {
    self.has_body = false;
    self.has_meta = false;

    self.topic.clear();
    self.binary.clear();

    self.compression = None;
    self.binary_length = 0;
    self.json_length = 0;

    self.ctx["_state"]["topic"] = JSON::Null;
    self.ctx["params"] = serde_json::json!({
      "files": {},
      "body": {},
      "query": {}
    });
  }
}

pub struct WebSocketStream {
  opts: WebSocketOpts,
  cb_send: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
  compression: Option<String>,
  headers_sent: bool,
  topic: String,
}

impl WebSocketStream {
  pub fn new(opts: WebSocketOpts) -> Self {
    Self {
      opts,
      cb_send: Arc::new(|_chunk: Vec<u8>| {}),
      compression: None,
      headers_sent: false,
      topic: String::new(),
    }
  }

  pub fn close(&mut self) -> () {
    self.compression = None;
    self.headers_sent = false;
    (self.cb_send)(Vec::from(b"x"));
  }

  pub fn on_send(&mut self, cb: Arc<dyn Fn(Vec<u8>) + Send + Sync>) {
    self.cb_send = cb;
  }

  pub fn push(&mut self, payload: &JSON, bytes: &[u8]) {
    let mut json: JSON = serde_json::json!({
        "topic": self.topic,
        "payload": payload
    });

    if !self.headers_sent {
      if let Some(compression) = &self.compression {
        json["compression"] = JSON::String(compression.clone());
      }
    }

    let mut buff: Vec<u8> = Vec::new();
    let message: String = serde_json::to_string(&json).unwrap();
    let meta: String = format!("{}+{}:", message.len(), bytes.len());
    buff.extend_from_slice(meta.as_bytes());
    buff.extend_from_slice(message.as_bytes());
    buff.extend_from_slice(bytes);

    if self.opts.compression && self.compression.is_some() {
      if let Some(_compression) = &self.compression {
        //TODO: Brotli
      }
    }

    (self.cb_send)(buff);
  }

  pub fn push_bytes(&mut self, bytes: &[u8]) -> () {
    let mut json: JSON = serde_json::json!({
        "topic": self.topic,
        "payload": "<bytes>"
    });

    if !self.headers_sent {
      if let Some(compression) = &self.compression {
        json["compression"] = JSON::String(compression.clone());
      }
    }

    let mut buff: Vec<u8> = Vec::new();
    let message: String = serde_json::to_string(&json).unwrap();
    let meta: String = format!("{}+{}:", message.len(), bytes.len());
    buff.extend_from_slice(meta.as_bytes());
    buff.extend_from_slice(message.as_bytes());
    buff.extend_from_slice(bytes);

    if self.opts.compression && self.compression.is_some() {
      if let Some(_compression) = &self.compression {
        //TODO: Brotli
      }
    }

    (self.cb_send)(buff);
  }

  pub fn push_json(&mut self, payload: &JSON) -> () {
    let mut json: JSON = serde_json::json!({
        "topic": self.topic,
        "payload": payload
    });

    if !self.headers_sent {
      if let Some(compression) = &self.compression {
        json["compression"] = JSON::String(compression.clone());
      }
    }

    let mut buff: Vec<u8> = Vec::new();
    let message: String = serde_json::to_string(&json).unwrap();
    let meta: String = format!("{}+{}:", message.len(), 0);
    buff.extend_from_slice(meta.as_bytes());
    buff.extend_from_slice(message.as_bytes());

    if self.opts.compression && self.compression.is_some() {
      if let Some(_compression) = &self.compression {
        //TODO: Brotli
      }
    }

    (self.cb_send)(buff);
  }

  pub fn set_compression(&mut self, compression: Option<String>) -> () {
    self.compression = compression;
    self.headers_sent = false;
  }

  pub fn set_topic(&mut self, topic: &str) {
    self.topic = String::from(topic);
  }
}

pub type WebSocketHandler = dyn Fn(Arc<Mutex<WebSocketCtx>>, Arc<Mutex<WebSocketBytes>>, Arc<Mutex<WebSocketStream>>)
  + Send
  + Sync;
pub type WebSocketLogger = dyn Fn(&str, &str) + Send + Sync;

async fn disconnect(
  handler: &Option<Arc<WebSocketHandler>>,
  args: &Arc<Mutex<Option<(Arc<Mutex<WebSocketCtx>>, Arc<Mutex<WebSocketBytes>>)>>>,
  opts: &WebSocketOpts,
) {
  if let Some(handler) = handler {
    let guard = args.lock().unwrap();
    if let Some((ctx, bytes)) = &*guard {
      handler(
        Arc::clone(ctx),
        Arc::clone(bytes),
        Arc::new(Mutex::new(WebSocketStream::new(opts.clone()))),
      );
    }
  }
}

async fn acceptor(
  listener: &TcpListener,
  logger_rt: Arc<RwLock<Arc<WebSocketLogger>>>,
  handlers_rt: Arc<RwLock<HashMap<String, Arc<WebSocketHandler>>>>,
  opts_rt: Arc<WebSocketOpts>,
  rate_limit_map_rt: Arc<Mutex<HashMap<String, u64>>>,
) -> () {
  let logger_conn: Arc<RwLock<Arc<WebSocketLogger>>> = Arc::clone(&logger_rt);
  let handlers_conn: Arc<RwLock<HashMap<String, Arc<WebSocketHandler>>>> = Arc::clone(&handlers_rt);
  let opts_conn: Arc<WebSocketOpts> = Arc::clone(&opts_rt);
  let read_timeout: Duration = Duration::from_secs(opts_conn.read_timeout);
  let send_timeout: Duration = Duration::from_secs(opts_conn.send_timeout);

  let frame_size: usize = opts_conn.block_size_kb * 1024 / 4;
  let max_message_size: usize = (opts_conn.max_message_size_kb * 1024) as usize;

  match listener.accept().await {
    Ok((socket, addr)) => {
      // CONNECTION TASK
      tokio::spawn(async move {
        let block_size: usize = (*opts_conn).block_size_kb * 1024;
        let (tx, mut rx) = mpsc::channel::<Message>(64);
        let mut req: WebSocketReq = WebSocketReq::new((*opts_conn).clone());
        req.set_addr(&addr.to_string());
        let stream: Arc<Mutex<WebSocketStream>> =
          Arc::new(Mutex::new(WebSocketStream::new((*opts_conn).clone())));
        {
          let mut stream_lock: std::sync::MutexGuard<'_, WebSocketStream> = stream.lock().unwrap();
          stream_lock.on_send(Arc::new({
            let tx: mpsc::Sender<Message> = tx.clone();
            move |chunk: Vec<u8>| {
              if chunk == b"x" {
                let _ = tx.try_send(Message::Close(None));
                return;
              }

              let _ = tx.try_send(Message::Binary(Bytes::from(chunk)));
            }
          }));
        }

        let mut ip: String = String::from("_");
        let last_args: Arc<Mutex<Option<(Arc<Mutex<WebSocketCtx>>, Arc<Mutex<WebSocketBytes>>)>>> =
          Arc::new(Mutex::new(None));
        let logger_final: Arc<RwLock<Arc<WebSocketLogger>>> = Arc::clone(&logger_conn);

        let (on_connect, on_disconnect) = {
          let handlers_lock = handlers_conn.read().unwrap();
          (
            handlers_lock.get("_connect").cloned(),
            handlers_lock.get("_disconnect").cloned(),
          )
        };

        let config: protocol::WebSocketConfig = protocol::WebSocketConfig::default();
        config.accept_unmasked_frames(false);
        config.max_message_size(Some(max_message_size));
        config.max_frame_size(Some(frame_size));
        config.read_buffer_size(block_size);
        config.write_buffer_size(block_size);

        let req_headers = tokio_tungstenite::accept_hdr_async_with_config(
          socket,
          |http_req: &handshake::server::Request, mut http_res: handshake::server::Response| {
            const SERVER: http::HeaderValue = http::HeaderValue::from_static("Arnelify Server");
            http_res.headers_mut().insert("Server", SERVER);
            let mut buff: Vec<u8> = Vec::new();

            let location: &str = http_req
              .uri()
              .path_and_query()
              .map(|pq| pq.as_str())
              .unwrap_or(http_req.uri().path());

            buff.extend_from_slice(b"GET ");
            buff.extend_from_slice(location.as_bytes());
            buff.extend_from_slice(b" HTTP/1.1\r\n");

            for (k, v) in http_req.headers().iter() {
              buff.extend_from_slice(k.as_str().as_bytes());
              buff.extend_from_slice(b": ");
              buff.extend_from_slice(v.as_bytes());
              buff.extend_from_slice(b"\r\n");
            }

            buff.extend_from_slice(b"\r\n");
            req.add(&buff);
            let _ = req.read_block();

            ip = req.get_ip();

            {
              let mut map: std::sync::MutexGuard<'_, HashMap<String, u64>> =
                rate_limit_map_rt.lock().unwrap();
              let entry: &mut u64 = map.entry(ip.clone()).or_insert(0);
              if *entry >= (*opts_conn).rate_limit {
                return Err(handshake::server::ErrorResponse::new(Some(
                  "429 Too Many Requests".into(),
                )));
              }

              *entry += 1;
            }

            Ok(http_res)
          },
          Some(config),
        );

        // HANDSHAKE
        let socket: Stream<TcpStream> = match timeout(read_timeout, req_headers).await {
          Ok(Ok(v)) => v,
          Ok(Err(_e)) => {
            let logger_lock: RwLockReadGuard<'_, Arc<WebSocketLogger>> =
              logger_conn.read().unwrap();
            (logger_lock)("warning", &format!("Client {}: Handshake error", addr));
            return;
          }
          _ => {
            let logger_lock: RwLockReadGuard<'_, Arc<WebSocketLogger>> =
              logger_conn.read().unwrap();
            (logger_lock)("warning", &format!("Client {}: Handshake timeout", addr));
            return;
          }
        };

        if let Some(handler) = &on_connect {
          let ctx_conn: Arc<Mutex<WebSocketCtx>> = Arc::new(Mutex::new(req.get_ctx()));
          let bytes_conn: Arc<Mutex<WebSocketBytes>> = Arc::new(Mutex::new(Vec::new()));
          let stream_conn: Arc<Mutex<WebSocketStream>> = Arc::clone(&stream);
          handler(ctx_conn, bytes_conn, stream_conn);

          let logger_lock: RwLockReadGuard<'_, Arc<WebSocketLogger>> = logger_conn.read().unwrap();
          (logger_lock)("info", &format!("Client {}: Connected", addr));
        }

        let (mut write, mut read) = socket.split();
        let last: Arc<AtomicU64> = Arc::new(AtomicU64::new(
          SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        ));

        let read_task: JoinHandle<()> = {
          let last_args_read = Arc::clone(&last_args);
          let last_read: Arc<AtomicU64> = Arc::clone(&last);
          let logger_read: Arc<RwLock<Arc<WebSocketLogger>>> = Arc::clone(&logger_conn);
          let tx_read: mpsc::Sender<Message> = tx.clone();

          // READ TASK
          tokio::spawn(async move {
            loop {
              match read.next().await {
                Some(msg) => match msg {
                  Ok(Message::Binary(bytes)) => {
                    req.add(&bytes);

                    match req.read_block() {
                      Ok(Some(_)) => {
                        let topic: String = req.get_topic();
                        let ctx_handler: Arc<Mutex<WebSocketCtx>> =
                          Arc::new(Mutex::new(req.get_ctx()));
                        let bytes_handler: Arc<Mutex<WebSocketBytes>> =
                          Arc::new(Mutex::new(req.get_bytes()));
                        let stream_handler: Arc<Mutex<WebSocketStream>> = Arc::clone(&stream);
                        {
                          let mut stream_lock: std::sync::MutexGuard<'_, WebSocketStream> =
                            stream_handler.lock().unwrap();
                          stream_lock.set_topic(&topic);
                          stream_lock.set_compression(req.get_compression());
                        }

                        {
                          let logger_lock: RwLockReadGuard<'_, Arc<WebSocketLogger>> =
                            logger_read.read().unwrap();
                          logger_lock(
                            "info",
                            &format!("Client {}: Sent payload: {}", addr, req.get_ctx()),
                          );
                        }

                        req.reset();

                        // SAVE LAST ARGS FOR DISCONNECT
                        let ctx_last: Arc<Mutex<WebSocketCtx>> = Arc::clone(&ctx_handler);
                        let bytes_last: Arc<Mutex<WebSocketBytes>> = Arc::clone(&bytes_handler);
                        {
                          *last_args_read.lock().unwrap() = Some((ctx_last, bytes_last));
                        }

                        let handler_opt: Option<Arc<WebSocketHandler>> = {
                          let handlers_lock: RwLockReadGuard<
                            '_,
                            HashMap<String, Arc<WebSocketHandler>>,
                          > = handlers_conn.read().unwrap();
                          handlers_lock
                            .get(&topic)
                            .or_else(|| handlers_lock.get("_"))
                            .cloned()
                        };

                        if let Some(handler) = handler_opt {
                          handler(ctx_handler, bytes_handler, stream_handler);
                        }
                      }
                      Ok(None) => {
                        let logger_lock: RwLockReadGuard<'_, Arc<WebSocketLogger>> =
                          logger_read.read().unwrap();
                        logger_lock("warning", &format!("Client {}: Block read error.", addr));
                        req.reset();
                      }
                      Err(e) => {
                        let logger_lock: RwLockReadGuard<'_, Arc<WebSocketLogger>> =
                          logger_read.read().unwrap();
                        logger_lock(
                          "warning",
                          &format!("Client {}: Block read error: {}", addr, e),
                        );
                        break;
                      }
                    }
                  }

                  Ok(Message::Ping(payload)) => {
                    let _ = tx_read.send(Message::Pong(payload)).await;
                  }

                  Ok(Message::Pong(_)) => {
                    last_read.store(
                      SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                      Ordering::SeqCst,
                    );
                  }

                  Ok(Message::Close(frame)) => {
                    let _ = tx_read.send(Message::Close(frame)).await;
                    break;
                  }
                  Err(_) => break,
                  _ => {}
                },
                None => break,
              }
            }
          })
        };

        // WRITE TASK
        let write_task: JoinHandle<()> = {
          let logger_write: Arc<RwLock<Arc<WebSocketLogger>>> = Arc::clone(&logger_conn);
          tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
              match timeout(send_timeout, write.send(msg)).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) => {}
                Err(_) => {
                  let logger: RwLockReadGuard<'_, Arc<WebSocketLogger>> =
                    logger_write.read().unwrap();
                  logger("warning", &format!("Client {}: Write timeout", addr));
                  break;
                }
              }
            }

            let logger: RwLockReadGuard<'_, Arc<WebSocketLogger>> = logger_write.read().unwrap();
            logger("info", &format!("Client {}: Write task finished", addr));
          })
        };

        // PING TASK
        let ping_task: JoinHandle<()> = {
          let last_pong: Arc<AtomicU64> = Arc::clone(&last);
          let opts_ping: Arc<WebSocketOpts> = Arc::clone(&opts_conn);
          let timeout_ping: Duration = Duration::from_secs((*opts_conn).ping_timeout);
          let tx_ping: mpsc::Sender<Message> = tx.clone();
          tokio::spawn(async move {
            loop {
              tokio::time::sleep(timeout_ping).await;

              let now: u64 = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

              if now - last_pong.load(Ordering::SeqCst) > opts_ping.ping_timeout {
                break;
              }

              let _ = tx_ping.send(Message::Ping(Bytes::new())).await;
            }
          })
        };

        tokio::select! {
          _ = read_task => {},
          _ = write_task => {}
        }

        ping_task.abort();
        {
          let mut map: std::sync::MutexGuard<'_, HashMap<String, u64>> =
            rate_limit_map_rt.lock().unwrap();
          if let Some(entry) = map.get_mut(&ip) {
            if *entry > 0 {
              *entry -= 1;
            }

            if *entry == 0 {
              map.remove(&ip);
            }
          }
        }

        disconnect(&on_disconnect, &last_args, &opts_conn).await;
        let logger_lock: RwLockReadGuard<'_, Arc<WebSocketLogger>> = logger_final.read().unwrap();
        logger_lock("info", &format!("Client {}: Disconnected", addr));
      });
    }
    Err(e) => {
      let logger_lock: RwLockReadGuard<'_, Arc<WebSocketLogger>> = logger_conn.read().unwrap();
      logger_lock("warning", &format!("Acceptor error: {}", e));
      return;
    }
  }
}

pub struct WebSocket {
  cb_logger: Arc<RwLock<Arc<WebSocketLogger>>>,
  handlers: Arc<RwLock<HashMap<String, Arc<WebSocketHandler>>>>,
  opts: WebSocketOpts,
  rate_limit_map: Arc<Mutex<HashMap<String, u64>>>,
  shutdown: Arc<Notify>,
}

impl WebSocket {
  pub fn new(opts: WebSocketOpts) -> Self {
    let handlers: Arc<RwLock<HashMap<String, Arc<WebSocketHandler>>>> =
      Arc::new(RwLock::new(HashMap::new()));
    {
      let mut handlers_write: RwLockWriteGuard<'_, HashMap<String, Arc<WebSocketHandler>>> =
        handlers.write().unwrap();
      handlers_write.insert(
        String::from("_"),
        Arc::new(
          move |_ctx: Arc<Mutex<WebSocketCtx>>,
                _bytes: Arc<Mutex<WebSocketBytes>>,
                stream: Arc<Mutex<WebSocketStream>>| {
            let mut stream_lock: std::sync::MutexGuard<'_, WebSocketStream> =
              stream.lock().unwrap();
            stream_lock.push_json(&serde_json::json!({
              "code": 404,
              "error": "Not found."
            }));
            stream_lock.close();
          },
        ),
      );

      handlers_write.insert(
        String::from("_connect"),
        Arc::new(
          move |_ctx: Arc<Mutex<WebSocketCtx>>,
                _bytes: Arc<Mutex<WebSocketBytes>>,
                _stream: Arc<Mutex<WebSocketStream>>| {},
        ),
      );

      handlers_write.insert(
        String::from("_disconnect"),
        Arc::new(
          move |_ctx: Arc<Mutex<WebSocketCtx>>,
                _bytes: Arc<Mutex<WebSocketBytes>>,
                _stream: Arc<Mutex<WebSocketStream>>| {},
        ),
      );
    }

    Self {
      opts,
      cb_logger: Arc::new(RwLock::new(Arc::new(move |_level: &str, message: &str| {
        println!("[Arnelify Server]: {}", message);
      }))),
      handlers,
      rate_limit_map: Arc::new(Mutex::new(HashMap::new())),
      shutdown: Arc::new(Notify::new()),
    }
  }

  pub fn logger(&self, cb: Arc<WebSocketLogger>) {
    let mut logger_write: RwLockWriteGuard<'_, Arc<WebSocketLogger>> =
      self.cb_logger.write().unwrap();
    *logger_write = cb;
  }

  pub fn on(&self, topic: &str, cb: Arc<WebSocketHandler>) {
    let mut handlers_write: RwLockWriteGuard<'_, HashMap<String, Arc<WebSocketHandler>>> =
      self.handlers.write().unwrap();
    handlers_write.insert(String::from(topic), cb);
  }

  pub fn start(&self) {
    let logger_rt: Arc<RwLock<Arc<WebSocketLogger>>> = Arc::clone(&self.cb_logger);
    let handlers_rt: Arc<RwLock<HashMap<String, Arc<WebSocketHandler>>>> =
      Arc::clone(&self.handlers);
    let opts_rt: Arc<WebSocketOpts> = Arc::new(self.opts.clone());
    let rate_limit_map_rt: Arc<Arc<Mutex<HashMap<String, u64>>>> =
      Arc::new(self.rate_limit_map.clone());
    let shutdown_rt: Arc<Notify> = Arc::clone(&self.shutdown);

    let rt: Runtime = Builder::new_multi_thread()
      .worker_threads(self.opts.thread_limit as usize)
      .enable_all()
      .build()
      .unwrap();

    rt.block_on(async move {
      let addr: (&str, u16) = ("0.0.0.0", opts_rt.port);
      let listener = TcpListener::bind(&addr).await.unwrap_or_else(|_| {
        let logger_lock: RwLockReadGuard<'_, Arc<WebSocketLogger>> = logger_rt.read().unwrap();
        logger_lock(
          "error",
          &format!("Port {} already in use.", (*opts_rt).port),
        );
        process::exit(1);
      });

      {
        let logger_lock: RwLockReadGuard<'_, Arc<WebSocketLogger>> = logger_rt.read().unwrap();
        logger_lock("success", &format!("WebSocket on port {}", (*opts_rt).port));
      }

      loop {
        tokio::select! {
          _ = shutdown_rt.notified() => {
            break;
          }
          _ = acceptor(
            &listener,
            Arc::clone(&logger_rt),
            Arc::clone(&handlers_rt),
            Arc::clone(&opts_rt),
            Arc::clone(&rate_limit_map_rt)
          ) => {}
        }
      }
    });
  }

  pub fn stop(&self) {
    self.shutdown.notify_waiters();
  }
}
