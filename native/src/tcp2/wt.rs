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

use std::{
  collections::HashMap,
  io::{Error, ErrorKind},
  process, str,
  sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
  },
  time::{SystemTime, UNIX_EPOCH},
};

use tokio::{
  runtime::{Builder, Runtime},
  sync::{Notify, mpsc},
  task::JoinHandle,
  time::{Duration, timeout},
};

use wtransport::{
  Connection, Endpoint, Identity, ServerConfig,
  endpoint::{IncomingSession, SessionRequest, endpoint_side::Server},
};

pub type WebTransportBytes = Vec<u8>;
pub type WebTransportCtx = serde_json::Value;
pub type JSON = serde_json::Value;

#[derive(Clone, Default)]
pub struct WebTransportOpts {
  pub block_size_kb: usize,
  pub cert_pem: String,
  pub compression: bool,
  pub handshake_timeout: u64,
  pub key_pem: String,
  pub max_message_size_kb: u64,
  pub ping_timeout: u64,
  pub port: u16,
  pub send_timeout: u64,
  pub thread_limit: u64,
}

struct WebTransportReq {
  opts: WebTransportOpts,

  has_body: bool,
  has_meta: bool,

  topic: String,
  buff: Vec<u8>,
  binary: Vec<u8>,

  compression: Option<String>,
  binary_length: usize,
  json_length: usize,

  ctx: WebTransportCtx,
}

impl WebTransportReq {
  pub fn new(opts: WebTransportOpts) -> Self {
    Self {
      opts,

      has_body: false,
      has_meta: false,

      topic: String::new(),
      buff: Vec::new(),
      binary: Vec::new(),

      compression: None,
      binary_length: 0,
      json_length: 0,

      ctx: serde_json::json!({
        "_state": {
          "cookie": {},
          "headers": {},
          "method": JSON::Null,
          "path": JSON::Null,
          "protocol": "WebTransport",
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

  pub fn get_bytes(&self) -> Vec<u8> {
    self.binary.to_vec()
  }

  pub fn get_compression(&self) -> Option<String> {
    self.compression.clone()
  }

  pub fn get_ctx(&self) -> WebTransportCtx {
    self.ctx.clone()
  }

  pub fn get_topic(&self) -> String {
    self.topic.clone()
  }

  pub fn is_empty(&self) -> bool {
    self.buff.is_empty()
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
      let message: &str = match std::str::from_utf8(&self.buff[..self.json_length]) {
        Ok(v) => v,
        Err(_) => {
          self.buff.clear();
          return Err(Error::new(ErrorKind::InvalidInput, "Invalid UTF-8."));
        }
      };

      let json: serde_json::Value = match serde_json::from_str(message) {
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

  pub fn read_block(&mut self) -> Result<Option<u8>, Error> {
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
    self.ctx = serde_json::json!({
      "_state": {
        "cookie": {},
        "headers": {},
        "method": JSON::Null,
        "path": JSON::Null,
        "protocol": "WebTransport",
        "topic": "_"
      },
      "params": {
        "files": {},
        "body": {},
        "query": {}
      }
    });
  }
}

pub struct WebTransportStream {
  opts: WebTransportOpts,
  cb_send: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
  compression: Option<String>,
  headers_sent: bool,
  topic: String,
}

impl WebTransportStream {
  pub fn new(opts: WebTransportOpts) -> Self {
    Self {
      opts,
      cb_send: Arc::new(|chunk: Vec<u8>| {
        println!("{:?}", chunk);
      }),
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

pub type WebTransportLogger = dyn Fn(&str, &str) + Send + Sync;
pub type WebTransportHandler = dyn Fn(Arc<Mutex<WebTransportCtx>>, Arc<Mutex<WebTransportBytes>>, Arc<Mutex<WebTransportStream>>)
  + Send
  + Sync;

pub struct WebTransport {
  cb_logger: Arc<Mutex<Arc<WebTransportLogger>>>,
  handlers: Arc<Mutex<HashMap<String, Arc<WebTransportHandler>>>>,
  opts: WebTransportOpts,
  shutdown: Arc<Notify>,
}

impl WebTransport {
  pub fn new(opts: WebTransportOpts) -> Self {
    Self {
      opts,
      cb_logger: Arc::new(Mutex::new(Arc::new(move |_level: &str, message: &str| {
        println!("[Arnelify Server]: {}", message);
      }))),
      handlers: Arc::new(Mutex::new(HashMap::new())),
      shutdown: Arc::new(Notify::new()),
    }
  }

  async fn acceptor(
    &self,
    endpoint: &Endpoint<Server>,
    logger_rt: Arc<Mutex<Arc<WebTransportLogger>>>,
    handlers_rt: Arc<Mutex<HashMap<String, Arc<WebTransportHandler>>>>,
    opts_rt: Arc<WebTransportOpts>,
  ) -> () {
    let handshake_timeout: Duration = Duration::from_secs(self.opts.handshake_timeout);
    let session: IncomingSession = endpoint.accept().await;

    let logger_accept: Arc<Mutex<Arc<WebTransportLogger>>> = Arc::clone(&logger_rt);
    let handlers_accept: Arc<Mutex<HashMap<String, Arc<WebTransportHandler>>>> =
      Arc::clone(&handlers_rt);
    let opts_accept: Arc<WebTransportOpts> = Arc::clone(&opts_rt);

    tokio::spawn(async move {
      let session_req: SessionRequest = match timeout(handshake_timeout, session).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
          let logger_lock: std::sync::MutexGuard<'_, Arc<WebTransportLogger>> =
            logger_accept.lock().unwrap();
          logger_lock("warning", &format!("Session failed: {:?}", e));
          return;
        }
        Err(_) => {
          let logger_lock: std::sync::MutexGuard<'_, Arc<WebTransportLogger>> =
            logger_accept.lock().unwrap();
          logger_lock("warning", "Session timeout");
          return;
        }
      };

      let conn: Connection = match timeout(handshake_timeout, session_req.accept()).await {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => {
          let logger_lock: std::sync::MutexGuard<'_, Arc<WebTransportLogger>> =
            logger_accept.lock().unwrap();
          logger_lock("warning", &format!("Accept failed: {:?}", e));
          return;
        }
        Err(_) => {
          let logger_lock: std::sync::MutexGuard<'_, Arc<WebTransportLogger>> =
            logger_accept.lock().unwrap();
          logger_lock("warning", "Connection accept timeout");
          return;
        }
      };

      let (mut send_stream, mut recv_stream) = match conn.accept_bi().await {
        Ok(s) => s,
        Err(e) => {
          let logger_lock: std::sync::MutexGuard<'_, Arc<WebTransportLogger>> =
            logger_accept.lock().unwrap();
          logger_lock("error", &format!("accept_bi failed: {:?}", e));
          return;
        }
      };

      let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
      let unixtime: Arc<AtomicU64> = Arc::new(AtomicU64::new(
        SystemTime::now()
          .duration_since(UNIX_EPOCH)
          .unwrap()
          .as_secs(),
      ));

      // PONG TASK
      let pong_task: JoinHandle<()> = {
        let ping: Arc<AtomicU64> = unixtime.clone();
        let opts_pong: Arc<WebTransportOpts> = Arc::clone(&opts_accept);
        let tx_pong: mpsc::Sender<Vec<u8>> = tx.clone();

        tokio::spawn(async move {
          loop {
            tokio::time::sleep(Duration::from_secs(5)).await;

            let now: u64 = SystemTime::now()
              .duration_since(UNIX_EPOCH)
              .unwrap()
              .as_secs();

            if now - ping.load(Ordering::SeqCst) > opts_pong.ping_timeout {
              break;
            }

            let buff: Vec<u8> = br#"29+0:{"topic":"pong","payload":{}}"#.to_vec();
            let _ = tx_pong.send(buff).await;
          }
        })
      };

      // WRITE TASK
      let write_task: JoinHandle<()> = {
        let send_timeout: Duration = Duration::from_secs((*opts_accept).send_timeout);
        tokio::spawn(async move {
          while let Some(bytes) = rx.recv().await {
            if bytes == b"x" {
              break;
            }

            match timeout(send_timeout, send_stream.write_all(&bytes)).await {
              Ok(Ok(_)) => {}
              Ok(Err(_)) => {
                break;
              }
              Err(_) => {
                break;
              }
            }
          }

          let _ = send_stream.finish().await;
          let _ = conn.close(0u32.into(), b"closed by server");
        })
      };

      let read_task: JoinHandle<()> = {
        let block_size: usize = (*opts_accept).block_size_kb * 1024;
        let logger_read: Arc<Mutex<Arc<WebTransportLogger>>> = Arc::clone(&logger_accept);
        let opts_read: Arc<WebTransportOpts> = Arc::clone(&opts_accept);
        tokio::spawn(async move {
          let mut buff: Vec<u8> = vec![0u8; block_size];
          let mut req: WebTransportReq = WebTransportReq::new((*opts_read).clone());
          let stream: Arc<Mutex<WebTransportStream>> =
            Arc::new(Mutex::new(WebTransportStream::new((*opts_read).clone())));
          {
            let mut stream_lock: std::sync::MutexGuard<'_, WebTransportStream> =
              stream.lock().unwrap();
            stream_lock.on_send(Arc::new({
              let tx: mpsc::Sender<Vec<u8>> = tx.clone();
              move |chunk: Vec<u8>| {
                let _ = tx.try_send(chunk);
              }
            }));
          }

          loop {
            match recv_stream.read(&mut buff).await {
              Ok(Some(bytes_read)) => {
                req.add(&buff[..bytes_read]);

                loop {
                  match req.read_block() {
                    Ok(Some(_)) => {
                      let topic: String = req.get_topic();
                      if topic == "ping" {
                        unixtime.store(
                          SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                          Ordering::SeqCst,
                        );

                        continue;
                      }

                      let ctx_handler: Arc<Mutex<WebTransportCtx>> =
                        Arc::new(Mutex::new(req.get_ctx()));
                      let bytes_handler: Arc<Mutex<WebTransportBytes>> =
                        Arc::new(Mutex::new(req.get_bytes()));
                      let stream_handler: Arc<Mutex<WebTransportStream>> = Arc::clone(&stream);
                      {
                        let mut stream_lock: std::sync::MutexGuard<'_, WebTransportStream> =
                          stream_handler.lock().unwrap();
                        stream_lock.set_topic(&topic);
                        stream_lock.set_compression(req.get_compression());
                      }

                      req.reset();
                      let handler_opt: Option<Arc<WebTransportHandler>> = {
                        let handlers_lock: std::sync::MutexGuard<
                          '_,
                          HashMap<String, Arc<WebTransportHandler>>,
                        > = handlers_accept.lock().unwrap();
                        handlers_lock.get(&topic).cloned()
                      };

                      if let Some(handler) = handler_opt {
                        handler(ctx_handler, bytes_handler, stream_handler);
                      }
                    }
                    Ok(None) => {}
                    Err(e) => match e.kind() {
                      ErrorKind::InvalidData => {
                        let logger_lock: std::sync::MutexGuard<'_, Arc<WebTransportLogger>> =
                          logger_read.lock().unwrap();
                        logger_lock("warning", &format!("Block read error: {}", e));
                        break;
                      }
                      _ => {}
                    },
                  }

                  if req.is_empty() {
                    break;
                  }
                }
              }
              Ok(None) => {
                break;
              }
              Err(_) => {
                break;
              }
            }
          }
        })
      };

      tokio::select! {
          _ = read_task => {}
          _ = write_task => {}
          _ = pong_task => {}
      }
    });
  }
  pub fn logger(&self, cb: Arc<WebTransportLogger>) -> () {
    let mut logger_lock: std::sync::MutexGuard<'_, Arc<WebTransportLogger>> =
      self.cb_logger.lock().unwrap();
    *logger_lock = cb;
  }

  pub fn on(&self, topic: &str, cb: Arc<WebTransportHandler>) {
    let mut map: std::sync::MutexGuard<'_, HashMap<String, Arc<WebTransportHandler>>> =
      self.handlers.lock().unwrap();
    map.insert(String::from(topic), cb);
  }

  pub fn start(&self) {
    let logger_rt: Arc<Mutex<Arc<WebTransportLogger>>> = Arc::clone(&self.cb_logger);
    let handlers_rt: Arc<Mutex<HashMap<String, Arc<WebTransportHandler>>>> =
      Arc::clone(&self.handlers);
    let opts_rt: Arc<WebTransportOpts> = Arc::new(self.opts.clone());
    let shutdown_rt: Arc<Notify> = Arc::clone(&self.shutdown);

    let rt: Runtime = Builder::new_multi_thread()
      .worker_threads(self.opts.thread_limit as usize)
      .enable_all()
      .build()
      .expect("Failed to create runtime");

    rt.block_on(async move {
      let identity: Identity =
        match Identity::load_pemfiles(&opts_rt.cert_pem, &opts_rt.key_pem).await {
          Ok(v) => v,
          Err(e) => {
            let logger_lock: std::sync::MutexGuard<'_, Arc<WebTransportLogger>> =
              logger_rt.lock().unwrap();
            logger_lock("error", &format!("TLS load failed: {:?}", e));
            return;
          }
        };

      let config: ServerConfig = ServerConfig::builder()
        .with_bind_default(opts_rt.port)
        .with_identity(identity)
        .max_idle_timeout(Some(Duration::from_secs(10)))
        .expect("invalid idle timeout")
        .build();

      let endpoint: Endpoint<Server> = match Endpoint::server(config) {
        Ok(s) => {
          let logger_lock: std::sync::MutexGuard<'_, Arc<WebTransportLogger>> =
            logger_rt.lock().unwrap();
          logger_lock("success", &format!("WebTransport on port {}", opts_rt.port));
          s
        }
        Err(e) => {
          let logger_lock: std::sync::MutexGuard<'_, Arc<WebTransportLogger>> =
            logger_rt.lock().unwrap();
          logger_lock("danger", &format!("Server start failed: {:?}", e));
          process::exit(1);
        }
      };

      loop {
        tokio::select! {
          _ = shutdown_rt.notified() => {
            break;
          }
          _ = self.acceptor(
            &endpoint,
            Arc::clone(&logger_rt),
            Arc::clone(&handlers_rt),
            Arc::clone(&opts_rt),
          ) => {}
        }
      }
    });
  }

  pub fn stop(&self) {
    self.shutdown.notify_waiters();
  }
}
