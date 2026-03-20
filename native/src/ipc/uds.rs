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
  path::Path,
  process,
  sync::{Arc, Mutex},
};

use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::UnixListener,
  runtime::{Builder, Runtime},
  sync::{Notify, mpsc},
};

pub type UnixDomainSocketBytes = Vec<u8>;
pub type UnixDomainSocketCtx = serde_json::Value;
pub type JSON = serde_json::Value;

enum StreamEvent {
  BodyChunk { chunk: Vec<u8>, flush: bool },
}

#[derive(Clone, Default)]
pub struct UnixDomainSocketOpts {
  pub block_size_kb: usize,
  pub socket_path: String,
  pub thread_limit: u64,
}

struct UnixDomainSocketReq {
  _opts: UnixDomainSocketOpts,

  has_body: bool,
  has_meta: bool,

  buff: Vec<u8>,
  binary: Vec<u8>,

  binary_length: usize,
  json_length: usize,

  topic: String,
  ctx: JSON,
}

impl UnixDomainSocketReq {
  pub fn new(_opts: UnixDomainSocketOpts) -> Self {
    Self {
      _opts,

      has_body: false,
      has_meta: false,

      buff: Vec::new(),
      binary: Vec::new(),

      binary_length: 0,
      json_length: 0,

      topic: String::new(),
      ctx: JSON::Null,
    }
  }

  pub fn add(&mut self, block: &[u8]) -> () {
    self.buff.extend_from_slice(block);
  }

  pub fn get_bytes(&self) -> Vec<u8> {
    self.binary.to_vec()
  }

  pub fn get_ctx(&self) -> UnixDomainSocketCtx {
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
    let pos: usize = match meta_bytes.iter().position(|&b| b == b'+') {
      Some(v) => v,
      None => return Err(Error::new(ErrorKind::InvalidData, "Missing '+' in meta.")),
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
      let data: &str = match std::str::from_utf8(&self.buff[..self.json_length]) {
        Ok(v) => v,
        Err(_) => {
          self.buff.clear();
          return Err(Error::new(ErrorKind::InvalidInput, "Invalid UTF-8."));
        }
      };

      let json: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => {
          self.buff.clear();
          return Err(Error::new(ErrorKind::InvalidInput, "Invalid JSON."));
        }
      };

      match json["topic"].as_str() {
        Some(s) => {
          self.topic = String::from(s);
        }
        None => {
          return Err(Error::new(ErrorKind::InvalidInput, "Invalid message."));
        }
      }

      if json.get("payload").is_none() {
        return Err(Error::new(ErrorKind::InvalidInput, "Invalid message."));
      }

      self.ctx = json["payload"].clone();

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

    self.binary.clear();

    self.binary_length = 0;
    self.json_length = 0;

    self.topic.clear();
    self.ctx = serde_json::json!({
      "topic": JSON::Null,
      "payload": {}
    });
  }
}

pub struct UnixDomainSocketStream {
  _opts: UnixDomainSocketOpts,
  cb_send: Arc<dyn Fn(Vec<u8>, bool) + Send + Sync>,
}

impl UnixDomainSocketStream {
  pub fn new(_opts: UnixDomainSocketOpts) -> Self {
    Self {
      _opts,
      cb_send: Arc::new(|chunk: Vec<u8>, _flush: bool| {
        println!("{:?}", chunk);
      }),
    }
  }

  pub fn on_send(&mut self, cb: Arc<dyn Fn(Vec<u8>, bool) + Send + Sync>) -> () {
    self.cb_send = cb;
  }

  pub fn push(&mut self, topic: &str, payload: &JSON, bytes: &[u8], flush: bool) -> () {
    let json: JSON = serde_json::json!({
        "topic": topic,
        "payload": payload
    });

    let mut buff: Vec<u8> = Vec::new();
    let message: String = serde_json::to_string(&json).unwrap();
    let meta: String = format!("{}+{}:", message.len(), bytes.len());
    buff.extend_from_slice(meta.as_bytes());
    buff.extend_from_slice(message.as_bytes());
    buff.extend_from_slice(bytes);

    (self.cb_send)(buff, flush);
  }
}

pub type UnixDomainSocketLogger = dyn Fn(&str, &str) + Send + Sync;
pub type UnixDomainSocketHandler =
  dyn Fn(Arc<Mutex<UnixDomainSocketCtx>>, Arc<Mutex<UnixDomainSocketBytes>>) + Send + Sync;

pub struct UnixDomainSocket {
  opts: UnixDomainSocketOpts,
  cb_logger: Arc<Mutex<Arc<UnixDomainSocketLogger>>>,
  handlers: Arc<Mutex<HashMap<String, Arc<UnixDomainSocketHandler>>>>,
  stream: Arc<Mutex<Option<Arc<Mutex<UnixDomainSocketStream>>>>>,
  shutdown: Arc<Notify>,
}

impl UnixDomainSocket {
  pub fn new(opts: UnixDomainSocketOpts) -> Self {
    Self {
      opts,
      cb_logger: Arc::new(Mutex::new(Arc::new(move |_level: &str, message: &str| {
        println!("[Arnelify Server]: {}", message);
      }))),
      handlers: Arc::new(Mutex::new(HashMap::new())),
      stream: Arc::new(Mutex::new(None)),
      shutdown: Arc::new(Notify::new()),
    }
  }

  async fn acceptor(
    &self,
    listener: &UnixListener,
    logger_rt: Arc<Mutex<Arc<UnixDomainSocketLogger>>>,
    handlers_rt: Arc<Mutex<HashMap<String, Arc<UnixDomainSocketHandler>>>>,
    opts_rt: Arc<UnixDomainSocketOpts>,
  ) -> () {
    match listener.accept().await {
      Ok((socket, _addr)) => {
        let (mut reader, mut writer) = socket.into_split();

        let logger_accept: Arc<Mutex<Arc<UnixDomainSocketLogger>>> = Arc::clone(&logger_rt);
        let handlers_accept: Arc<Mutex<HashMap<String, Arc<UnixDomainSocketHandler>>>> =
          Arc::clone(&handlers_rt);
        let opts_accept: Arc<UnixDomainSocketOpts> = Arc::clone(&opts_rt);

        let (tx, mut rx) = mpsc::channel::<StreamEvent>(32);
        let stream: Arc<Mutex<UnixDomainSocketStream>> = Arc::new(Mutex::new(
          UnixDomainSocketStream::new((*opts_accept).clone()),
        ));

        {
          let mut stream_lock: std::sync::MutexGuard<'_, UnixDomainSocketStream> =
            stream.lock().unwrap();
          stream_lock.on_send(Arc::new({
            let tx: mpsc::Sender<StreamEvent> = tx.clone();
            move |chunk: Vec<u8>, flush: bool| {
              let _ = tx.try_send(StreamEvent::BodyChunk { chunk, flush });
            }
          }));
        }

        {
          let mut stream_lock: std::sync::MutexGuard<
            '_,
            Option<Arc<Mutex<UnixDomainSocketStream>>>,
          > = self.stream.lock().unwrap();
          *stream_lock = Some(stream.clone());
        }

        // WRITE TASK
        tokio::spawn(async move {
          while let Some(event) = rx.recv().await {
            match event {
              StreamEvent::BodyChunk { chunk, flush } => {
                let _ = writer.write_all(&chunk).await;
                if flush {
                  writer.flush().await.unwrap();
                }
              }
            }
          }
        });

        // READ TASK
        tokio::spawn(async move {
          let block_size: usize = opts_accept.block_size_kb * 1024;
          let mut buff: Vec<u8> = vec![0u8; block_size];
          let mut req: UnixDomainSocketReq = UnixDomainSocketReq::new((*opts_accept).clone());

          loop {
            match reader.read(&mut buff).await {
              Ok(bytes_read) => {
                if bytes_read == 0 {
                  let logger_lock: std::sync::MutexGuard<'_, Arc<UnixDomainSocketLogger>> =
                    logger_accept.lock().unwrap();
                  logger_lock("warning", &format!("Client disconnected."));
                  break;
                }

                req.add(&buff[..bytes_read]);

                loop {
                  match req.read_block() {
                    Ok(Some(_)) => {
                      let topic: String = req.get_topic();
                      let ctx_handler: Arc<Mutex<UnixDomainSocketCtx>> =
                        Arc::new(Mutex::new(req.get_ctx()));
                      let bytes_handler: Arc<Mutex<UnixDomainSocketBytes>> =
                        Arc::new(Mutex::new(req.get_bytes().to_vec()));

                      req.reset();
                      let handler_opt: Option<Arc<UnixDomainSocketHandler>> = {
                        let handlers_lock: std::sync::MutexGuard<
                          '_,
                          HashMap<String, Arc<UnixDomainSocketHandler>>,
                        > = handlers_accept.lock().unwrap();
                        handlers_lock.get(&topic).cloned()
                      };

                      if let Some(handler) = handler_opt {
                        handler(ctx_handler, bytes_handler);
                      }
                    }
                    Ok(None) => {}
                    Err(_) => {
                      let logger_lock: std::sync::MutexGuard<'_, Arc<UnixDomainSocketLogger>> =
                        logger_accept.lock().unwrap();
                      logger_lock("error", &format!("Block read error."));
                      process::exit(1);
                    }
                  }

                  if req.is_empty() {
                    break;
                  }
                }
              }
              Err(_) => {
                let logger_lock: std::sync::MutexGuard<'_, Arc<UnixDomainSocketLogger>> =
                  logger_accept.lock().unwrap();
                logger_lock("error", &format!("Socket read error."));
                break;
              }
            }
          }
        });
      }
      Err(e) => {
        let logger_lock: std::sync::MutexGuard<'_, Arc<UnixDomainSocketLogger>> =
          logger_rt.lock().unwrap();
        logger_lock("danger", &format!("Acceptor error: {}", e));
      }
    }
  }

  pub fn _logger(&self, cb: Arc<UnixDomainSocketLogger>) -> () {
    let mut logger_lock: std::sync::MutexGuard<'_, Arc<UnixDomainSocketLogger>> =
      self.cb_logger.lock().unwrap();
    *logger_lock = cb;
  }

  pub fn on(&mut self, topic: &str, cb: Arc<UnixDomainSocketHandler>) -> () {
    let mut map: std::sync::MutexGuard<'_, HashMap<String, Arc<UnixDomainSocketHandler>>> =
      self.handlers.lock().unwrap();
    map.insert(String::from(topic), cb);
  }

  pub fn push(&self, topic: &str, payload: &JSON, bytes: UnixDomainSocketBytes, flush: bool) -> () {
    let lock: std::sync::MutexGuard<'_, Option<Arc<Mutex<UnixDomainSocketStream>>>> =
      self.stream.lock().unwrap();
    if let Some(stream) = &*lock {
      let mut stream_lock = stream.lock().unwrap();
      stream_lock.push(&topic, &payload, &bytes, flush);
    }
  }

  pub fn start(&self, on_start: Arc<dyn Fn() + Send + Sync>) -> () {
    let logger_rt: Arc<Mutex<Arc<UnixDomainSocketLogger>>> = Arc::clone(&self.cb_logger);
    let handlers_rt: Arc<Mutex<HashMap<String, Arc<UnixDomainSocketHandler>>>> =
      Arc::clone(&self.handlers);
    let opts_rt: Arc<UnixDomainSocketOpts> = Arc::new(self.opts.clone());
    let shutdown_rt: Arc<Notify> = Arc::clone(&self.shutdown);

    if Path::new(&opts_rt.socket_path).exists() {
      match std::fs::remove_file(&opts_rt.socket_path) {
        Ok(_) => {}
        Err(_) => {
          let logger_lock: std::sync::MutexGuard<'_, Arc<UnixDomainSocketLogger>> =
            logger_rt.lock().unwrap();
          logger_lock(
            "error",
            &format!("Error in Unix Domain Socket: Socket open error."),
          );
          process::exit(1);
        }
      }
    }

    let rt: Runtime = Builder::new_multi_thread()
      .worker_threads(self.opts.thread_limit as usize)
      .enable_all()
      .build()
      .unwrap();

    rt.block_on(async move {
      let listener: UnixListener = match UnixListener::bind(&opts_rt.socket_path) {
        Ok(v) => v,
        Err(_) => {
          let logger_lock: std::sync::MutexGuard<'_, Arc<UnixDomainSocketLogger>> =
            logger_rt.lock().unwrap();
          logger_lock(
            "warning",
            &format!("Error in Unix Domain Socket: Socket bind error."),
          );
          process::exit(1);
        }
      };

      on_start();

      loop {
        tokio::select! {
          _ = shutdown_rt.notified() => {
            break;
          }
          _ = self.acceptor(
            &listener,
            Arc::clone(&logger_rt),
            Arc::clone(&handlers_rt),
            Arc::clone(&opts_rt)
          ) => {}
        }
      }
    });
  }

  pub fn stop(&self) -> () {
    self.shutdown.notify_waiters();
  }
}
