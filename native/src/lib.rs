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

use neon::prelude::*;

mod ipc;
mod tcp1;
mod tcp2;

use tcp1::{Http1, Http1Ctx, Http1Handler, Http1Logger, Http1Opts, Http1Stream};
use tcp1::{Http2, Http2Ctx, Http2Handler, Http2Logger, Http2Opts, Http2Stream};
use tcp1::{
  WebSocket, WebSocketBytes, WebSocketCtx, WebSocketHandler, WebSocketLogger, WebSocketOpts,
  WebSocketStream,
};

use tcp2::{Http3, Http3Ctx, Http3Handler, Http3Logger, Http3Opts, Http3Stream};
use tcp2::{
  WebTransport, WebTransportBytes, WebTransportCtx, WebTransportHandler, WebTransportLogger,
  WebTransportOpts, WebTransportStream,
};

use ipc::{
  UnixDomainSocket, UnixDomainSocketBytes, UnixDomainSocketCtx, UnixDomainSocketHandler,
  UnixDomainSocketOpts, UnixDomainSocketStream,
};

use std::{
  collections::HashMap,
  convert::TryFrom,
  sync::{
    Arc, Mutex, MutexGuard, OnceLock,
    atomic::{AtomicU64, Ordering},
    mpsc,
  },
  thread,
  time::{SystemTime, UNIX_EPOCH},
};

type JSON = serde_json::Value;

type Http1Streams = HashMap<u64, (Arc<Mutex<Http1Stream>>, mpsc::Sender<u8>)>;
static HTTP1_MAP: OnceLock<Mutex<HashMap<u64, Arc<Http1>>>> = OnceLock::new();
static HTTP1_ID: OnceLock<Mutex<u64>> = OnceLock::new();
static HTTP1_STREAM_ID: AtomicU64 = AtomicU64::new(1);
static HTTP1_STREAMS: OnceLock<Mutex<Http1Streams>> = OnceLock::new();
static HTTP1_UDS_MAP: OnceLock<Mutex<HashMap<u64, Arc<UnixDomainSocket>>>> = OnceLock::new();

type Http2Streams = HashMap<u64, (Arc<Mutex<Http2Stream>>, mpsc::Sender<u8>)>;
static HTTP2_MAP: OnceLock<Mutex<HashMap<u64, Arc<Http2>>>> = OnceLock::new();
static HTTP2_ID: OnceLock<Mutex<u64>> = OnceLock::new();
static HTTP2_STREAM_ID: AtomicU64 = AtomicU64::new(1);
static HTTP2_STREAMS: OnceLock<Mutex<Http2Streams>> = OnceLock::new();
static HTTP2_UDS_MAP: OnceLock<Mutex<HashMap<u64, Arc<UnixDomainSocket>>>> = OnceLock::new();

type WebSocketStreams = HashMap<String, Arc<Mutex<WebSocketStream>>>;
static WS_MAP: OnceLock<Mutex<HashMap<u64, Arc<WebSocket>>>> = OnceLock::new();
static WS_ID: OnceLock<Mutex<u64>> = OnceLock::new();
static WS_STREAMS: OnceLock<Mutex<WebSocketStreams>> = OnceLock::new();
static WS_UDS_MAP: OnceLock<Mutex<HashMap<u64, Arc<UnixDomainSocket>>>> = OnceLock::new();

type Http3Streams = HashMap<u64, (Arc<Mutex<Http3Stream>>, mpsc::Sender<u8>)>;
static HTTP3_MAP: OnceLock<Mutex<HashMap<u64, Arc<Http3>>>> = OnceLock::new();
static HTTP3_ID: OnceLock<Mutex<u64>> = OnceLock::new();
static HTTP3_STREAM_ID: AtomicU64 = AtomicU64::new(1);
static HTTP3_STREAMS: OnceLock<Mutex<Http3Streams>> = OnceLock::new();
static HTTP3_UDS_MAP: OnceLock<Mutex<HashMap<u64, Arc<UnixDomainSocket>>>> = OnceLock::new();

type WebTransportStreams = HashMap<u64, Arc<Mutex<WebTransportStream>>>;
static WT_MAP: OnceLock<Mutex<HashMap<u64, Arc<WebTransport>>>> = OnceLock::new();
static WT_ID: OnceLock<Mutex<u64>> = OnceLock::new();
static WT_STREAM_ID: AtomicU64 = AtomicU64::new(1);
static WT_STREAMS: OnceLock<Mutex<WebTransportStreams>> = OnceLock::new();
static WT_UDS_MAP: OnceLock<Mutex<HashMap<u64, Arc<UnixDomainSocket>>>> = OnceLock::new();

fn get_str(opts: &JSON, key: &str) -> String {
  opts
    .get(key)
    .and_then(JSON::as_str)
    .expect(&format!(
      "[Arnelify Server]: NEON error: '{}' missing or not a string.",
      key
    ))
    .to_string()
}

fn get_u64(opts: &JSON, key: &str) -> u64 {
  opts.get(key).and_then(JSON::as_u64).expect(&format!(
    "[Arnelify Server]: NEON error: '{}' missing or not a u64.",
    key
  ))
}

fn get_usize(opts: &JSON, key: &str) -> usize {
  let val: u64 = get_u64(opts, key);
  usize::try_from(val).expect(&format!(
    "[Arnelify Server]: NEON error: '{}' out of usize range.",
    key
  ))
}

fn get_u32(opts: &JSON, key: &str) -> u32 {
  let val: u64 = get_u64(opts, key);
  u32::try_from(val).expect(&format!(
    "[Arnelify Server]: NEON error: '{}' out of u32 range.",
    key
  ))
}

fn get_u16(opts: &JSON, key: &str) -> u16 {
  let val: u64 = get_u64(opts, key);
  u16::try_from(val).expect(&format!(
    "[Arnelify Server]: NEON error: '{}' out of u16 range.",
    key
  ))
}

fn get_u8(opts: &JSON, key: &str) -> u8 {
  let val: u64 = get_u64(opts, key);
  u8::try_from(val).expect(&format!(
    "[Arnelify Server]: NEON error: '{}' out of u8 range.",
    key
  ))
}

fn get_bool(opts: &JSON, key: &str) -> bool {
  opts.get(key).and_then(JSON::as_bool).expect(&format!(
    "[Arnelify Server]: NEON error: '{}' missing or not a bool.",
    key
  ))
}

pub fn generate_request_id() -> u128 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .expect("Time went backwards")
    .as_nanos()
}

fn http1_create(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let js_opts = cx.argument::<JsString>(0)?.value(&mut cx);
  let opts: JSON = match serde_json::from_str(&js_opts) {
    Ok(json) => json,
    Err(_) => {
      println!("[Arnelify Server]: NEON error in http1_create: Invalid JSON in 'c_opts'.");
      return Ok(cx.number(0.0));
    }
  };

  let id: &Mutex<u64> = HTTP1_ID.get_or_init(|| Mutex::new(0));
  let new_id: u64 = {
    let mut js: MutexGuard<'_, u64> = id.lock().unwrap();
    *js += 1;
    *js
  };

  let uds_opts: UnixDomainSocketOpts = UnixDomainSocketOpts {
    block_size_kb: get_usize(&opts, "block_size_kb"),
    socket_path: get_str(&opts, "socket_path"),
    thread_limit: get_u64(&opts, "thread_limit"),
  };

  let uds_http1_add_header: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let key: String = v[1].as_str().unwrap_or("").to_string();
          let value: String = v[2].as_str().unwrap_or("").to_string();

          if let Some(map) = HTTP1_STREAMS.get() {
            let http1_stream: Option<(Arc<Mutex<Http1Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http1Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http1_stream {
              Some((http1_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http1Stream> =
                  http1_stream_safe.lock().unwrap();
                stream_lock.add_header(&key, &value);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http1_add_header: No stream found."
                ]);

                if let Some(map) = HTTP1_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http1_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http1_end: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          if let Some(map) = HTTP1_STREAMS.get() {
            let http1_stream: Option<(Arc<Mutex<Http1Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http1Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http1_stream {
              Some((http1_stream_safe, tx)) => {
                let mut stream_lock: MutexGuard<'_, Http1Stream> =
                  http1_stream_safe.lock().unwrap();
                stream_lock.end();
                let _ = tx.send(1);
              }
              None => {
                let args: JSON =
                  serde_json::json!(["error", "NEON error in uds_http1_end: No stream found."]);

                if let Some(map) = HTTP1_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http1_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http1_push_bytes: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let is_attachment: bool = v[1].as_u64().unwrap_or(0) != 0;
          let bytes: Vec<u8> = bytes.lock().unwrap().clone();

          if let Some(map) = HTTP1_STREAMS.get() {
            let http1_stream: Option<(Arc<Mutex<Http1Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http1Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http1_stream {
              Some((http1_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http1Stream> =
                  http1_stream_safe.lock().unwrap();
                stream_lock.push_bytes(&bytes, is_attachment);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http1_push_bytes: No stream found."
                ]);

                if let Some(map) = HTTP1_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http1_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http1_push_file: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let file_path: &str = v[1].as_str().unwrap_or("");
          let is_attachment: bool = v[2].as_u64().unwrap_or(0) != 0;

          if let Some(map) = HTTP1_STREAMS.get() {
            let http1_stream: Option<(Arc<Mutex<Http1Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http1Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http1_stream {
              Some((http1_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http1Stream> =
                  http1_stream_safe.lock().unwrap();
                stream_lock.push_file(&file_path, is_attachment);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http1_push_file: No stream found."
                ]);

                if let Some(map) = HTTP1_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http1_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http1_push_json: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let json: JSON = v[1].clone();
          let is_attachment: bool = v[2].as_u64().unwrap_or(0) != 0;

          if let Some(map) = HTTP1_STREAMS.get() {
            let http1_stream: Option<(Arc<Mutex<Http1Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http1Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http1_stream {
              Some((http1_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http1Stream> =
                  http1_stream_safe.lock().unwrap();
                stream_lock.push_json(&json, is_attachment);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http1_push_json: No stream found."
                ]);

                if let Some(map) = HTTP1_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http1_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http1_set_code: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let code: u64 = v[1].as_u64().unwrap_or(0);

          if let Some(map) = HTTP1_STREAMS.get() {
            let http1_stream: Option<(Arc<Mutex<Http1Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http1Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http1_stream {
              Some((http1_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http1Stream> =
                  http1_stream_safe.lock().unwrap();
                stream_lock.set_code(code as u16);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http1_set_code: No stream found."
                ]);

                if let Some(map) = HTTP1_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http1_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http1_set_compression: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let compression: &str = v[1].as_str().unwrap_or("");

          if let Some(map) = HTTP1_STREAMS.get() {
            let http1_stream: Option<(Arc<Mutex<Http1Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http1Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http1_stream {
              Some((http1_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http1Stream> =
                  http1_stream_safe.lock().unwrap();
                if compression.len() > 0 {
                  stream_lock.set_compression(Some(String::from(compression)));
                  return;
                }

                stream_lock.set_compression(None);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http1_set_compression: No stream found."
                ]);

                if let Some(map) = HTTP1_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http1_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http1_set_headers: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let mut headers: Vec<(String, String)> = Vec::new();
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          if let Some(JSON::Object(map)) = v.get(1) {
            for (key, value) in map {
              let value = match value {
                JSON::String(s) => s.clone(),
                JSON::Number(n) => n.to_string(),
                JSON::Bool(b) => b.to_string(),
                _ => continue,
              };

              headers.push((key.clone(), value));
            }
          }

          if let Some(map) = HTTP1_STREAMS.get() {
            let http1_stream: Option<(Arc<Mutex<Http1Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http1Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http1_stream {
              Some((http1_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http1Stream> =
                  http1_stream_safe.lock().unwrap();
                stream_lock.set_headers(headers);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http1_set_headers: No stream found."
                ]);

                if let Some(map) = HTTP1_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http1_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds: UnixDomainSocket = UnixDomainSocket::new(uds_opts);
  uds.on("http1_add_header", uds_http1_add_header);
  uds.on("http1_end", uds_http1_end);
  uds.on("http1_push_bytes", uds_http1_push_bytes);
  uds.on("http1_push_file", uds_http1_push_file);
  uds.on("http1_push_json", uds_http1_push_json);
  uds.on("http1_set_code", uds_http1_set_code);
  uds.on("http1_set_compression", uds_http1_set_compression);
  uds.on("http1_set_headers", uds_http1_set_headers);

  let uds_map: &Mutex<HashMap<u64, Arc<UnixDomainSocket>>> =
    HTTP1_UDS_MAP.get_or_init(|| Mutex::new(HashMap::new()));
  {
    uds_map.lock().unwrap().insert(new_id as u64, Arc::new(uds));
  }

  let http1_opts: Http1Opts = Http1Opts {
    allow_empty_files: get_bool(&opts, "allow_empty_files"),
    block_size_kb: get_usize(&opts, "block_size_kb"),
    charset: get_str(&opts, "charset"),
    compression: get_bool(&opts, "compression"),
    keep_alive: get_u8(&opts, "keep_alive"),
    keep_extensions: get_bool(&opts, "keep_extensions"),
    max_fields: get_u32(&opts, "max_fields"),
    max_fields_size_total_mb: get_usize(&opts, "max_fields_size_total_mb"),
    max_files: get_u32(&opts, "max_files"),
    max_files_size_total_mb: get_usize(&opts, "max_files_size_total_mb"),
    max_file_size_mb: get_usize(&opts, "max_file_size_mb"),
    port: get_u16(&opts, "port"),
    storage_path: get_str(&opts, "storage_path"),
    thread_limit: get_u64(&opts, "thread_limit"),
  };

  let http1: Http1 = Http1::new(http1_opts);
  let http1_map: &Mutex<HashMap<u64, Arc<Http1>>> =
    HTTP1_MAP.get_or_init(|| Mutex::new(HashMap::new()));
  {
    http1_map
      .lock()
      .unwrap()
      .insert(new_id as u64, Arc::new(http1));
  }

  Ok(cx.number(new_id as f64))
}

fn http1_destroy(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  if let Some(map) = HTTP1_MAP.get() {
    map.lock().unwrap().remove(&id);
  }

  if let Some(map) = HTTP1_UDS_MAP.get() {
    map.lock().unwrap().remove(&id);
  }

  Ok(cx.undefined())
}

fn http1_logger(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  let http1_logger: Arc<Http1Logger> = Arc::new(move |level: &str, message: &str| -> () {
    let args: JSON = serde_json::json!([level, message]);
    let bytes: UnixDomainSocketBytes = Vec::new();

    if let Some(map) = HTTP1_UDS_MAP.get() {
      if let Some(uds) = map.lock().unwrap().get(&id) {
        uds.send("http1_logger", &args, bytes, true);
      }
    }
  });

  if let Some(map) = HTTP1_UDS_MAP.get() {
    if let Some(uds) = map.lock().unwrap().get(&id) {
      uds.logger(Arc::clone(&http1_logger));
    }
  }

  if let Some(map) = HTTP1_MAP.get() {
    if let Some(http1) = map.lock().unwrap().get(&id) {
      http1.logger(Arc::clone(&http1_logger));
    }
  }

  Ok(cx.undefined())
}

fn http1_on(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let path: String = cx.argument::<JsString>(1)?.value(&mut cx);
  let id: u64 = js_id as u64;

  let path_safe: String = path.clone();
  let http1_handler: Arc<Http1Handler> = Arc::new(
    move |ctx: Arc<Mutex<Http1Ctx>>, stream: Arc<Mutex<Http1Stream>>| -> () {
      let stream_id: u64 = HTTP1_STREAM_ID.fetch_add(1, Ordering::Relaxed);
      let (tx, rx) = mpsc::channel::<u8>();

      HTTP1_STREAMS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .insert(stream_id, (stream, tx));

      let ctx: Http1Ctx = ctx.lock().unwrap().clone();
      let args: JSON = serde_json::json!([stream_id, path_safe, ctx]);
      let bytes: UnixDomainSocketBytes = Vec::new();

      if let Some(map) = HTTP1_UDS_MAP.get() {
        if let Some(uds) = map.lock().unwrap().get(&id) {
          uds.send("http1_on", &args, bytes, true);
        }
      }

      while let Ok(v) = rx.recv() {
        if v == 1 {
          break;
        }
      }

      if let Some(map) = HTTP1_STREAMS.get() {
        map.lock().unwrap().remove(&stream_id);
      }
    },
  );

  if let Some(map) = HTTP1_MAP.get() {
    if let Some(http1) = map.lock().unwrap().get(&id) {
      http1.on(&path, http1_handler);
    }
  }

  Ok(cx.undefined())
}

fn http1_start_ipc(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  let (tx, rx) = mpsc::channel::<u8>();

  if let Some(map) = HTTP1_UDS_MAP.get() {
    if let Some(uds) = map.lock().unwrap().get(&id) {
      let uds_safe: Arc<UnixDomainSocket> = Arc::clone(uds);
      thread::spawn(move || {
        uds_safe.start(Arc::new(move || {
          let _ = tx.send(1);
        }));
      });
    }
  }

  loop {
    match rx.recv() {
      Ok(1) => break,
      Ok(_) => continue,
      Err(_) => break,
    }
  }

  Ok(cx.undefined())
}

fn http1_start(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  if let Some(map) = HTTP1_MAP.get() {
    if let Some(http1) = map.lock().unwrap().get(&id) {
      let http1_safe: Arc<Http1> = Arc::clone(http1);
      thread::spawn(move || {
        http1_safe.start();
      });
    }
  }

  Ok(cx.undefined())
}

fn http1_stop(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  if let Some(map) = HTTP1_UDS_MAP.get() {
    if let Some(uds) = map.lock().unwrap().get(&id) {
      uds.stop();
    }
  }

  if let Some(map) = HTTP1_MAP.get() {
    if let Some(http1) = map.lock().unwrap().get(&id) {
      http1.stop();
    }
  }

  Ok(cx.undefined())
}

fn http2_create(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let js_opts = cx.argument::<JsString>(0)?.value(&mut cx);
  let opts: JSON = match serde_json::from_str(&js_opts) {
    Ok(json) => json,
    Err(_) => {
      println!("[Arnelify Server]: NEON error in http2_create: Invalid JSON in 'c_opts'.");
      return Ok(cx.number(0.0));
    }
  };

  let id: &Mutex<u64> = HTTP2_ID.get_or_init(|| Mutex::new(0));
  let new_id: u64 = {
    let mut js: MutexGuard<'_, u64> = id.lock().unwrap();
    *js += 1;
    *js
  };

  let uds_opts: UnixDomainSocketOpts = UnixDomainSocketOpts {
    block_size_kb: get_usize(&opts, "block_size_kb"),
    socket_path: get_str(&opts, "socket_path"),
    thread_limit: get_u64(&opts, "thread_limit"),
  };

  let uds_http2_add_header: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let key: String = v[1].as_str().unwrap_or("").to_string();
          let value: String = v[2].as_str().unwrap_or("").to_string();

          if let Some(map) = HTTP2_STREAMS.get() {
            let http2_stream: Option<(Arc<Mutex<Http2Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http2Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http2_stream {
              Some((http2_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http2Stream> =
                  http2_stream_safe.lock().unwrap();
                stream_lock.add_header(&key, &value);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http2_add_header: No stream found."
                ]);

                if let Some(map) = HTTP2_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http2_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http2_end: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          if let Some(map) = HTTP2_STREAMS.get() {
            let http2_stream: Option<(Arc<Mutex<Http2Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http2Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http2_stream {
              Some((http2_stream_safe, tx)) => {
                let mut stream_lock: MutexGuard<'_, Http2Stream> =
                  http2_stream_safe.lock().unwrap();
                stream_lock.end();
                let _ = tx.send(1);
              }
              None => {
                let args: JSON =
                  serde_json::json!(["error", "NEON error in uds_http2_end: No stream found."]);

                if let Some(map) = HTTP2_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http2_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http2_push_bytes: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let is_attachment: bool = v[1].as_u64().unwrap_or(0) != 0;
          let bytes: Vec<u8> = bytes.lock().unwrap().clone();

          if let Some(map) = HTTP2_STREAMS.get() {
            let http2_stream: Option<(Arc<Mutex<Http2Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http2Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http2_stream {
              Some((http2_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http2Stream> =
                  http2_stream_safe.lock().unwrap();
                stream_lock.push_bytes(&bytes, is_attachment);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http2_push_bytes: No stream found."
                ]);

                if let Some(map) = HTTP2_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http2_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http2_push_file: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let file_path: &str = v[1].as_str().unwrap_or("");
          let is_attachment: bool = v[2].as_u64().unwrap_or(0) != 0;

          if let Some(map) = HTTP2_STREAMS.get() {
            let http2_stream: Option<(Arc<Mutex<Http2Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http2Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http2_stream {
              Some((http2_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http2Stream> =
                  http2_stream_safe.lock().unwrap();
                stream_lock.push_file(&file_path, is_attachment);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http2_push_file: No stream found."
                ]);

                if let Some(map) = HTTP2_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http2_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http2_push_json: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let json: JSON = v[1].clone();
          let is_attachment: bool = v[2].as_u64().unwrap_or(0) != 0;

          if let Some(map) = HTTP2_STREAMS.get() {
            let http2_stream: Option<(Arc<Mutex<Http2Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http2Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http2_stream {
              Some((http2_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http2Stream> =
                  http2_stream_safe.lock().unwrap();
                stream_lock.push_json(&json, is_attachment);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http2_push_json: No stream found."
                ]);

                if let Some(map) = HTTP2_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http2_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http2_set_code: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let code: u64 = v[1].as_u64().unwrap_or(0);

          if let Some(map) = HTTP2_STREAMS.get() {
            let http2_stream: Option<(Arc<Mutex<Http2Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http2Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http2_stream {
              Some((http2_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http2Stream> =
                  http2_stream_safe.lock().unwrap();
                stream_lock.set_code(code as u16);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http2_set_code: No stream found."
                ]);

                if let Some(map) = HTTP2_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http2_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http2_set_compression: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let compression: &str = v[1].as_str().unwrap_or("");

          if let Some(map) = HTTP2_STREAMS.get() {
            let http2_stream: Option<(Arc<Mutex<Http2Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http2Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http2_stream {
              Some((http2_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http2Stream> =
                  http2_stream_safe.lock().unwrap();
                if compression.len() > 0 {
                  stream_lock.set_compression(Some(String::from(compression)));
                  return;
                }

                stream_lock.set_compression(None);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http2_set_compression: No stream found."
                ]);

                if let Some(map) = HTTP2_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http2_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http2_set_headers: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let mut headers: Vec<(String, String)> = Vec::new();
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);

          if let Some(JSON::Object(map)) = v.get(1) {
            for (key, value) in map {
              let value = match value {
                JSON::String(s) => s.clone(),
                JSON::Number(n) => n.to_string(),
                JSON::Bool(b) => b.to_string(),
                _ => continue,
              };

              headers.push((key.clone(), value));
            }
          }

          if let Some(map) = HTTP2_STREAMS.get() {
            let http2_stream: Option<(Arc<Mutex<Http2Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http2Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http2_stream {
              Some((http2_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http2Stream> =
                  http2_stream_safe.lock().unwrap();
                stream_lock.set_headers(headers);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http2_set_headers: No stream found."
                ]);

                if let Some(map) = HTTP2_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http2_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds: UnixDomainSocket = UnixDomainSocket::new(uds_opts);
  uds.on("http2_add_header", uds_http2_add_header);
  uds.on("http2_end", uds_http2_end);
  uds.on("http2_push_bytes", uds_http2_push_bytes);
  uds.on("http2_push_file", uds_http2_push_file);
  uds.on("http2_push_json", uds_http2_push_json);
  uds.on("http2_set_code", uds_http2_set_code);
  uds.on("http2_set_compression", uds_http2_set_compression);
  uds.on("http2_set_headers", uds_http2_set_headers);

  let uds_map: &Mutex<HashMap<u64, Arc<UnixDomainSocket>>> =
    HTTP2_UDS_MAP.get_or_init(|| Mutex::new(HashMap::new()));
  {
    uds_map.lock().unwrap().insert(new_id as u64, Arc::new(uds));
  }

  let http2_opts: Http2Opts = Http2Opts {
    allow_empty_files: get_bool(&opts, "allow_empty_files"),
    block_size_kb: get_usize(&opts, "block_size_kb"),
    cert_pem: get_str(&opts, "cert_pem"),
    charset: get_str(&opts, "charset"),
    compression: get_bool(&opts, "compression"),
    keep_alive: get_u8(&opts, "keep_alive"),
    keep_extensions: get_bool(&opts, "keep_extensions"),
    key_pem: get_str(&opts, "key_pem"),
    max_fields: get_u32(&opts, "max_fields"),
    max_fields_size_total_mb: get_usize(&opts, "max_fields_size_total_mb"),
    max_files: get_u32(&opts, "max_files"),
    max_files_size_total_mb: get_usize(&opts, "max_files_size_total_mb"),
    max_file_size_mb: get_usize(&opts, "max_file_size_mb"),
    port: get_u16(&opts, "port"),
    storage_path: get_str(&opts, "storage_path"),
    thread_limit: get_u64(&opts, "thread_limit"),
  };

  let http2: Http2 = Http2::new(http2_opts);
  let http2_map: &Mutex<HashMap<u64, Arc<Http2>>> =
    HTTP2_MAP.get_or_init(|| Mutex::new(HashMap::new()));
  {
    http2_map
      .lock()
      .unwrap()
      .insert(new_id as u64, Arc::new(http2));
  }

  Ok(cx.number(new_id as f64))
}

fn http2_destroy(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  if let Some(map) = HTTP2_MAP.get() {
    map.lock().unwrap().remove(&id);
  }

  if let Some(map) = HTTP2_UDS_MAP.get() {
    map.lock().unwrap().remove(&id);
  }

  Ok(cx.undefined())
}

fn http2_logger(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  let http2_logger: Arc<Http2Logger> = Arc::new(move |level: &str, message: &str| -> () {
    let args: JSON = serde_json::json!([level, message]);
    let bytes: UnixDomainSocketBytes = Vec::new();

    if let Some(map) = HTTP2_UDS_MAP.get() {
      if let Some(uds) = map.lock().unwrap().get(&id) {
        uds.send("http2_logger", &args, bytes, true);
      }
    }
  });

  if let Some(map) = HTTP1_UDS_MAP.get() {
    if let Some(uds) = map.lock().unwrap().get(&id) {
      uds.logger(Arc::clone(&http2_logger));
    }
  }

  if let Some(map) = HTTP2_MAP.get() {
    if let Some(http2) = map.lock().unwrap().get(&id) {
      http2.logger(Arc::clone(&http2_logger));
    }
  }

  Ok(cx.undefined())
}

fn http2_on(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let path: String = cx.argument::<JsString>(1)?.value(&mut cx);
  let id: u64 = js_id as u64;

  let path_safe: String = path.clone();
  let http2_handler: Arc<Http2Handler> = Arc::new(
    move |ctx: Arc<Mutex<Http2Ctx>>, stream: Arc<Mutex<Http2Stream>>| -> () {
      let (tx, rx) = mpsc::channel::<u8>();
      let stream_id: u64 = HTTP2_STREAM_ID.fetch_add(1, Ordering::Relaxed);

      HTTP2_STREAMS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .insert(stream_id, (stream, tx));

      let ctx: Http2Ctx = ctx.lock().unwrap().clone();
      let args: JSON = serde_json::json!([stream_id, path_safe, ctx]);
      let bytes: UnixDomainSocketBytes = Vec::new();

      if let Some(map) = HTTP2_UDS_MAP.get() {
        if let Some(uds) = map.lock().unwrap().get(&id) {
          uds.send("http2_on", &args, bytes, true);
        }
      }

      while let Ok(v) = rx.recv() {
        if v == 1 {
          break;
        }
      }

      if let Some(map) = HTTP2_STREAMS.get() {
        map.lock().unwrap().remove(&stream_id);
      }
    },
  );

  if let Some(map) = HTTP2_MAP.get() {
    if let Some(http2) = map.lock().unwrap().get(&id) {
      http2.on(&path, http2_handler);
    }
  }

  Ok(cx.undefined())
}

fn http2_start_ipc(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  let (tx, rx) = mpsc::channel::<u8>();

  if let Some(map) = HTTP2_UDS_MAP.get() {
    if let Some(uds) = map.lock().unwrap().get(&id) {
      let uds_safe: Arc<UnixDomainSocket> = Arc::clone(uds);
      thread::spawn(move || {
        uds_safe.start(Arc::new(move || {
          let _ = tx.send(1);
        }));
      });
    }
  }

  loop {
    match rx.recv() {
      Ok(1) => break,
      Ok(_) => continue,
      Err(_) => break,
    }
  }

  Ok(cx.undefined())
}

fn http2_start(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  if let Some(map) = HTTP2_MAP.get() {
    if let Some(http2) = map.lock().unwrap().get(&id) {
      let http2_safe: Arc<Http2> = Arc::clone(http2);
      thread::spawn(move || {
        http2_safe.start();
      });
    }
  }

  Ok(cx.undefined())
}

fn http2_stop(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  if let Some(map) = HTTP2_UDS_MAP.get() {
    if let Some(uds) = map.lock().unwrap().get(&id) {
      uds.stop();
    }
  }

  if let Some(map) = HTTP2_MAP.get() {
    if let Some(http2) = map.lock().unwrap().get(&id) {
      http2.stop();
    }
  }

  Ok(cx.undefined())
}

fn ws_create(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let js_opts: String = cx.argument::<JsString>(0)?.value(&mut cx);
  let opts: JSON = match serde_json::from_str(&js_opts) {
    Ok(json) => json,
    Err(_) => {
      println!("[Arnelify Server]: NEON error in ws_create: Invalid JSON in 'c_opts'.");
      return Ok(cx.number(0.0));
    }
  };

  let id: &Mutex<u64> = WS_ID.get_or_init(|| Mutex::new(0));
  let new_id: u64 = {
    let mut js: MutexGuard<'_, u64> = id.lock().unwrap();
    *js += 1;
    *js
  };

  let uds_opts: UnixDomainSocketOpts = UnixDomainSocketOpts {
    block_size_kb: get_usize(&opts, "block_size_kb"),
    socket_path: get_str(&opts, "socket_path"),
    thread_limit: get_u64(&opts, "thread_limit"),
  };

  let uds_ws_close: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let addr: &str = v[0].as_str().unwrap_or("");

          if let Some(map) = WS_STREAMS.get() {
            let ws_stream: Option<Arc<Mutex<WebSocketStream>>> = {
              let streams: MutexGuard<'_, WebSocketStreams> = map.lock().unwrap();
              streams.get(addr).cloned()
            };

            match ws_stream {
              Some(ws_stream_safe) => {
                let mut stream_lock: MutexGuard<'_, WebSocketStream> =
                  ws_stream_safe.lock().unwrap();
                stream_lock.close();
              }
              None => {
                let args: JSON =
                  serde_json::json!(["error", "NEON error in uds_ws_close: No stream found."]);

                if let Some(map) = WS_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("ws_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_ws_push: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let addr: &str = v[0].as_str().unwrap_or("");
          let json: JSON = v[1].clone();
          let bytes: Vec<u8> = bytes.lock().unwrap().clone();

          if let Some(map) = WS_STREAMS.get() {
            let ws_stream: Option<Arc<Mutex<WebSocketStream>>> = {
              let streams: MutexGuard<'_, WebSocketStreams> = map.lock().unwrap();
              streams.get(addr).cloned()
            };

            match ws_stream {
              Some(ws_stream_safe) => {
                let mut stream_lock: MutexGuard<'_, WebSocketStream> =
                  ws_stream_safe.lock().unwrap();
                stream_lock.push(&json, &bytes);
              }
              None => {
                let args: JSON =
                  serde_json::json!(["error", "NEON error in uds_ws_push: No stream found."]);

                if let Some(map) = WS_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("ws_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_ws_push_bytes: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let addr: &str = v[0].as_str().unwrap_or("");
          let bytes: Vec<u8> = bytes.lock().unwrap().clone();

          if let Some(map) = WS_STREAMS.get() {
            let ws_stream: Option<Arc<Mutex<WebSocketStream>>> = {
              let streams: MutexGuard<'_, WebSocketStreams> = map.lock().unwrap();
              streams.get(addr).cloned()
            };

            match ws_stream {
              Some(ws_stream_safe) => {
                let mut stream_lock: MutexGuard<'_, WebSocketStream> =
                  ws_stream_safe.lock().unwrap();
                stream_lock.push_bytes(&bytes);
              }
              None => {
                let args: JSON =
                  serde_json::json!(["error", "NEON error in uds_ws_push_bytes: No stream found."]);

                if let Some(map) = WS_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("ws_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_ws_push_json: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let addr: &str = v[0].as_str().unwrap_or("");
          let json: JSON = v[1].clone();

          if let Some(map) = WS_STREAMS.get() {
            let ws_stream: Option<Arc<Mutex<WebSocketStream>>> = {
              let streams: MutexGuard<'_, WebSocketStreams> = map.lock().unwrap();
              streams.get(addr).cloned()
            };

            match ws_stream {
              Some(ws_stream_safe) => {
                let mut stream_lock: MutexGuard<'_, WebSocketStream> =
                  ws_stream_safe.lock().unwrap();
                stream_lock.push_json(&json);
              }
              None => {
                let args: JSON =
                  serde_json::json!(["error", "NEON error in uds_ws_push_json: No stream found."]);

                if let Some(map) = WS_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("ws_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_ws_set_compression: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let addr: &str = v[0].as_str().unwrap_or("");
          let compression: &str = v[1].as_str().unwrap_or("");

          if let Some(map) = WS_STREAMS.get() {
            let ws_stream: Option<Arc<Mutex<WebSocketStream>>> = {
              let streams: MutexGuard<'_, WebSocketStreams> = map.lock().unwrap();
              streams.get(addr).cloned()
            };

            match ws_stream {
              Some(ws_stream_safe) => {
                let mut stream_lock: MutexGuard<'_, WebSocketStream> =
                  ws_stream_safe.lock().unwrap();
                if compression.len() > 0 {
                  stream_lock.set_compression(Some(String::from(compression)));
                  return;
                }

                stream_lock.set_compression(None);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_ws_set_compression: No stream found."
                ]);

                if let Some(map) = WS_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("ws_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds: UnixDomainSocket = UnixDomainSocket::new(uds_opts);
  uds.on("ws_close", uds_ws_close);
  uds.on("ws_push", uds_ws_push);
  uds.on("ws_push_bytes", uds_ws_push_bytes);
  uds.on("ws_push_json", uds_ws_push_json);
  uds.on("ws_set_compression", uds_ws_set_compression);

  let uds_map: &Mutex<HashMap<u64, Arc<UnixDomainSocket>>> =
    WS_UDS_MAP.get_or_init(|| Mutex::new(HashMap::new()));
  {
    uds_map.lock().unwrap().insert(new_id as u64, Arc::new(uds));
  }

  let ws_opts: WebSocketOpts = WebSocketOpts {
    block_size_kb: get_usize(&opts, "block_size_kb"),
    compression: get_bool(&opts, "compression"),
    max_message_size_kb: get_u64(&opts, "max_message_size_kb"),
    ping_timeout: get_u64(&opts, "ping_timeout"),
    port: get_u16(&opts, "port"),
    rate_limit: get_u64(&opts, "rate_limit"),
    read_timeout: get_u64(&opts, "read_timeout"),
    send_timeout: get_u64(&opts, "send_timeout"),
    thread_limit: get_u64(&opts, "thread_limit"),
  };

  let ws: WebSocket = WebSocket::new(ws_opts);

  ws.on(
    "_disconnect",
    Arc::new(
      |ctx: Arc<Mutex<WebSocketCtx>>,
       _bytes: Arc<Mutex<WebSocketBytes>>,
       _stream: Arc<Mutex<WebSocketStream>>| {
        let ctx: WebSocketCtx = ctx.lock().unwrap().clone();
        if let Some(addr) = ctx["_state"]["addr"].as_str() {
          if let Some(map) = WS_STREAMS.get() {
            map.lock().unwrap().remove(addr);
          }
        }
      },
    ),
  );

  let ws_map: &Mutex<HashMap<u64, Arc<WebSocket>>> =
    WS_MAP.get_or_init(|| Mutex::new(HashMap::new()));
  {
    ws_map.lock().unwrap().insert(new_id as u64, Arc::new(ws));
  }

  Ok(cx.number(new_id as f64))
}

fn ws_destroy(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  if let Some(map) = WS_MAP.get() {
    map.lock().unwrap().remove(&id);
  }

  if let Some(map) = WS_UDS_MAP.get() {
    map.lock().unwrap().remove(&id);
  }

  Ok(cx.undefined())
}

fn ws_logger(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  let ws_logger: Arc<WebSocketLogger> = Arc::new(move |level: &str, message: &str| -> () {
    let args: JSON = serde_json::json!([level, message]);
    let bytes: UnixDomainSocketBytes = Vec::new();

    if let Some(map) = WS_UDS_MAP.get() {
      if let Some(uds) = map.lock().unwrap().get(&id) {
        uds.send("ws_logger", &args, bytes, true);
      }
    }
  });

  if let Some(map) = HTTP1_UDS_MAP.get() {
    if let Some(uds) = map.lock().unwrap().get(&id) {
      uds.logger(Arc::clone(&ws_logger));
    }
  }

  if let Some(map) = WS_MAP.get() {
    if let Some(ws) = map.lock().unwrap().get(&id) {
      ws.logger(Arc::clone(&ws_logger));
    }
  }

  Ok(cx.undefined())
}

fn ws_on(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let topic: String = cx.argument::<JsString>(1)?.value(&mut cx);
  let id: u64 = js_id as u64;

  let topic_safe: String = topic.clone();
  let ws_handler: Arc<WebSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<WebSocketCtx>>,
          bytes: Arc<Mutex<WebSocketBytes>>,
          stream: Arc<Mutex<WebSocketStream>>|
          -> () {
      let ctx: WebSocketCtx = ctx.lock().unwrap().clone();
      if let Some(addr) = ctx["_state"]["addr"].as_str() {
        if topic_safe == "_disconnect" {
          if let Some(map) = WS_STREAMS.get() {
            map.lock().unwrap().remove(addr);
          }

          return;
        }

        WS_STREAMS
          .get_or_init(|| Mutex::new(HashMap::new()))
          .lock()
          .unwrap()
          .insert(String::from(addr), stream);

        let args: JSON = serde_json::json!([addr, topic_safe, ctx]);
        let bytes: WebSocketBytes = bytes.lock().unwrap().clone();

        if let Some(map) = WS_UDS_MAP.get() {
          if let Some(uds) = map.lock().unwrap().get(&id) {
            uds.send("ws_on", &args, bytes, true);
          }
        }
      }
    },
  );

  if let Some(map) = WS_MAP.get() {
    if let Some(ws) = map.lock().unwrap().get(&id) {
      ws.on(&topic, ws_handler);
    }
  }

  Ok(cx.undefined())
}

fn ws_start_ipc(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  let (tx, rx) = mpsc::channel::<u8>();

  if let Some(map) = WS_UDS_MAP.get() {
    if let Some(uds) = map.lock().unwrap().get(&id) {
      let uds_safe: Arc<UnixDomainSocket> = Arc::clone(uds);
      thread::spawn(move || {
        uds_safe.start(Arc::new(move || {
          let _ = tx.send(1);
        }));
      });
    }
  }

  loop {
    match rx.recv() {
      Ok(1) => break,
      Ok(_) => continue,
      Err(_) => break,
    }
  }

  Ok(cx.undefined())
}

fn ws_start(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  if let Some(map) = WS_MAP.get() {
    if let Some(ws) = map.lock().unwrap().get(&id) {
      let ws_safe: Arc<WebSocket> = Arc::clone(ws);
      thread::spawn(move || {
        ws_safe.start();
      });
    }
  }

  Ok(cx.undefined())
}

fn ws_stop(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  if let Some(map) = WS_UDS_MAP.get() {
    if let Some(uds) = map.lock().unwrap().get(&id) {
      uds.stop();
    }
  }

  if let Some(map) = WS_MAP.get() {
    if let Some(ws) = map.lock().unwrap().get(&id) {
      ws.stop();
    }
  }

  Ok(cx.undefined())
}

fn http3_create(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let js_opts = cx.argument::<JsString>(0)?.value(&mut cx);
  let opts: JSON = match serde_json::from_str(&js_opts) {
    Ok(json) => json,
    Err(_) => {
      println!("[Arnelify Server]: NEON error in http3_create: Invalid JSON in 'c_opts'.");
      return Ok(cx.number(0.0));
    }
  };

  let id: &Mutex<u64> = HTTP3_ID.get_or_init(|| Mutex::new(0));
  let new_id: u64 = {
    let mut js: MutexGuard<'_, u64> = id.lock().unwrap();
    *js += 1;
    *js
  };

  let uds_opts: UnixDomainSocketOpts = UnixDomainSocketOpts {
    block_size_kb: get_usize(&opts, "block_size_kb"),
    socket_path: get_str(&opts, "socket_path"),
    thread_limit: get_u64(&opts, "thread_limit"),
  };

  let uds_http3_add_header: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let key: String = v[1].as_str().unwrap_or("").to_string();
          let value: String = v[2].as_str().unwrap_or("").to_string();

          if let Some(map) = HTTP3_STREAMS.get() {
            let http3_stream: Option<(Arc<Mutex<Http3Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http3Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http3_stream {
              Some((http3_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http3Stream> =
                  http3_stream_safe.lock().unwrap();
                stream_lock.add_header(&key, &value);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http3_add_header: No stream found."
                ]);

                if let Some(map) = HTTP3_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http3_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http3_end: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);

          if let Some(map) = HTTP3_STREAMS.get() {
            let http3_stream: Option<(Arc<Mutex<Http3Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http3Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http3_stream {
              Some((http3_stream_safe, tx)) => {
                let mut stream_lock: MutexGuard<'_, Http3Stream> =
                  http3_stream_safe.lock().unwrap();
                stream_lock.end();
                let _ = tx.send(1);
              }
              None => {
                let args: JSON =
                  serde_json::json!(["error", "NEON error in uds_http3_end: No stream found."]);

                if let Some(map) = HTTP3_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http3_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http3_push_bytes: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let is_attachment: bool = v[1].as_u64().unwrap_or(0) != 0;
          let bytes: Vec<u8> = bytes.lock().unwrap().clone();

          if let Some(map) = HTTP3_STREAMS.get() {
            let http3_stream: Option<(Arc<Mutex<Http3Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http3Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http3_stream {
              Some((http3_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http3Stream> =
                  http3_stream_safe.lock().unwrap();
                stream_lock.push_bytes(&bytes, is_attachment);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http3_push_bytes: No stream found."
                ]);

                if let Some(map) = HTTP3_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http3_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http3_push_file: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let file_path: &str = v[1].as_str().unwrap_or("");
          let is_attachment: bool = v[2].as_u64().unwrap_or(0) != 0;

          if let Some(map) = HTTP3_STREAMS.get() {
            let http3_stream: Option<(Arc<Mutex<Http3Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http3Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http3_stream {
              Some((http3_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http3Stream> =
                  http3_stream_safe.lock().unwrap();
                stream_lock.push_file(&file_path, is_attachment);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http3_push_file: No stream found."
                ]);

                if let Some(map) = HTTP3_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http3_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http3_push_json: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let json: JSON = v[1].clone();
          let is_attachment: bool = v[2].as_u64().unwrap_or(0) != 0;

          if let Some(map) = HTTP3_STREAMS.get() {
            let http3_stream: Option<(Arc<Mutex<Http3Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http3Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http3_stream {
              Some((http3_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http3Stream> =
                  http3_stream_safe.lock().unwrap();
                stream_lock.push_json(&json, is_attachment);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http3_push_json: No stream found."
                ]);

                if let Some(map) = HTTP3_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http3_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http3_set_code: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let code: u64 = v[1].as_u64().unwrap_or(0);

          if let Some(map) = HTTP3_STREAMS.get() {
            let http3_stream: Option<(Arc<Mutex<Http3Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http3Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http3_stream {
              Some((http3_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http3Stream> =
                  http3_stream_safe.lock().unwrap();
                stream_lock.set_code(code as u16);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http3_set_code: No stream found."
                ]);

                if let Some(map) = HTTP3_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http3_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http3_set_compression: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let compression: &str = v[1].as_str().unwrap_or("");

          if let Some(map) = HTTP3_STREAMS.get() {
            let http3_stream: Option<(Arc<Mutex<Http3Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http3Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http3_stream {
              Some((http3_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http3Stream> =
                  http3_stream_safe.lock().unwrap();
                if compression.len() > 0 {
                  stream_lock.set_compression(Some(String::from(compression)));
                  return;
                }

                stream_lock.set_compression(None);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http3_set_compression: No stream found."
                ]);

                if let Some(map) = HTTP3_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http3_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_http3_set_headers: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let mut headers: Vec<(String, String)> = Vec::new();
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          if let Some(JSON::Object(map)) = v.get(1) {
            for (key, value) in map {
              let value = match value {
                JSON::String(s) => s.clone(),
                JSON::Number(n) => n.to_string(),
                JSON::Bool(b) => b.to_string(),
                _ => continue,
              };

              headers.push((key.clone(), value));
            }
          }

          if let Some(map) = HTTP3_STREAMS.get() {
            let http3_stream: Option<(Arc<Mutex<Http3Stream>>, mpsc::Sender<u8>)> = {
              let streams: MutexGuard<'_, Http3Streams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match http3_stream {
              Some((http3_stream_safe, _tx)) => {
                let mut stream_lock: MutexGuard<'_, Http3Stream> =
                  http3_stream_safe.lock().unwrap();
                stream_lock.set_headers(headers);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_http3_set_headers: No stream found."
                ]);

                if let Some(map) = HTTP3_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("http3_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds: UnixDomainSocket = UnixDomainSocket::new(uds_opts);
  uds.on("http3_add_header", uds_http3_add_header);
  uds.on("http3_end", uds_http3_end);
  uds.on("http3_push_bytes", uds_http3_push_bytes);
  uds.on("http3_push_file", uds_http3_push_file);
  uds.on("http3_push_json", uds_http3_push_json);
  uds.on("http3_set_code", uds_http3_set_code);
  uds.on("http3_set_compression", uds_http3_set_compression);
  uds.on("http3_set_headers", uds_http3_set_headers);

  let uds_map: &Mutex<HashMap<u64, Arc<UnixDomainSocket>>> =
    HTTP3_UDS_MAP.get_or_init(|| Mutex::new(HashMap::new()));
  {
    uds_map.lock().unwrap().insert(new_id as u64, Arc::new(uds));
  }

  let http3_opts: Http3Opts = Http3Opts {
    allow_empty_files: get_bool(&opts, "allow_empty_files"),
    block_size_kb: get_usize(&opts, "block_size_kb"),
    cert_pem: get_str(&opts, "cert_pem"),
    charset: get_str(&opts, "charset"),
    compression: get_bool(&opts, "compression"),
    keep_alive: get_u8(&opts, "keep_alive"),
    keep_extensions: get_bool(&opts, "keep_extensions"),
    key_pem: get_str(&opts, "key_pem"),
    max_fields: get_u32(&opts, "max_fields"),
    max_fields_size_total_mb: get_usize(&opts, "max_fields_size_total_mb"),
    max_files: get_u32(&opts, "max_files"),
    max_files_size_total_mb: get_usize(&opts, "max_files_size_total_mb"),
    max_file_size_mb: get_usize(&opts, "max_file_size_mb"),
    port: get_u16(&opts, "port"),
    storage_path: get_str(&opts, "storage_path"),
    thread_limit: get_u64(&opts, "thread_limit"),
  };

  let http3: Http3 = Http3::new(http3_opts);
  let http3_map: &Mutex<HashMap<u64, Arc<Http3>>> =
    HTTP3_MAP.get_or_init(|| Mutex::new(HashMap::new()));
  {
    http3_map
      .lock()
      .unwrap()
      .insert(new_id as u64, Arc::new(http3));
  }

  Ok(cx.number(new_id as f64))
}

fn http3_destroy(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  if let Some(map) = HTTP3_MAP.get() {
    map.lock().unwrap().remove(&id);
  }

  if let Some(map) = HTTP3_UDS_MAP.get() {
    map.lock().unwrap().remove(&id);
  }

  Ok(cx.undefined())
}

fn http3_logger(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  let http3_logger: Arc<Http3Logger> = Arc::new(move |level: &str, message: &str| -> () {
    let args: JSON = serde_json::json!([level, message]);
    let bytes: UnixDomainSocketBytes = Vec::new();

    if let Some(map) = HTTP3_UDS_MAP.get() {
      if let Some(uds) = map.lock().unwrap().get(&id) {
        uds.send("http3_logger", &args, bytes, true);
      }
    }
  });

  if let Some(map) = HTTP1_UDS_MAP.get() {
    if let Some(uds) = map.lock().unwrap().get(&id) {
      uds.logger(Arc::clone(&http3_logger));
    }
  }

  if let Some(map) = HTTP3_MAP.get() {
    if let Some(http3) = map.lock().unwrap().get(&id) {
      http3.logger(Arc::clone(&http3_logger));
    }
  }

  Ok(cx.undefined())
}

fn http3_on(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let path: String = cx.argument::<JsString>(1)?.value(&mut cx);
  let id: u64 = js_id as u64;

  let path_safe: String = path.clone();
  let http3_handler: Arc<Http3Handler> = Arc::new(
    move |ctx: Arc<Mutex<Http3Ctx>>, stream: Arc<Mutex<Http3Stream>>| -> () {
      let (tx, rx) = mpsc::channel::<u8>();
      let stream_id: u64 = HTTP3_STREAM_ID.fetch_add(1, Ordering::Relaxed);

      HTTP3_STREAMS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .insert(stream_id, (stream, tx));

      let ctx: Http3Ctx = ctx.lock().unwrap().clone();
      let args: JSON = serde_json::json!([stream_id, path_safe, ctx]);
      let bytes: UnixDomainSocketBytes = Vec::new();

      if let Some(map) = HTTP3_UDS_MAP.get() {
        if let Some(uds) = map.lock().unwrap().get(&id) {
          uds.send("http3_on", &args, bytes, true);
        }
      }

      while let Ok(v) = rx.recv() {
        if v == 1 {
          break;
        }
      }

      if let Some(map) = HTTP3_STREAMS.get() {
        map.lock().unwrap().remove(&stream_id);
      }
    },
  );

  if let Some(map) = HTTP3_MAP.get() {
    if let Some(http3) = map.lock().unwrap().get(&id) {
      http3.on(&path, http3_handler);
    }
  }

  Ok(cx.undefined())
}

fn http3_start_ipc(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  let (tx, rx) = mpsc::channel::<u8>();

  if let Some(map) = HTTP3_UDS_MAP.get() {
    if let Some(uds) = map.lock().unwrap().get(&id) {
      let uds_safe: Arc<UnixDomainSocket> = Arc::clone(uds);
      thread::spawn(move || {
        uds_safe.start(Arc::new(move || {
          let _ = tx.send(1);
        }));
      });
    }
  }

  loop {
    match rx.recv() {
      Ok(1) => break,
      Ok(_) => continue,
      Err(_) => break,
    }
  }

  Ok(cx.undefined())
}

fn http3_start(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  if let Some(map) = HTTP3_MAP.get() {
    if let Some(http3) = map.lock().unwrap().get(&id) {
      let http3_safe: Arc<Http3> = Arc::clone(http3);
      thread::spawn(move || {
        http3_safe.start();
      });
    }
  }

  Ok(cx.undefined())
}

fn http3_stop(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  if let Some(map) = HTTP3_UDS_MAP.get() {
    if let Some(uds) = map.lock().unwrap().get(&id) {
      uds.stop();
    }
  }

  if let Some(map) = HTTP3_MAP.get() {
    if let Some(http3) = map.lock().unwrap().get(&id) {
      http3.stop();
    }
  }

  Ok(cx.undefined())
}

fn wt_create(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let js_opts = cx.argument::<JsString>(0)?.value(&mut cx);
  let opts: JSON = match serde_json::from_str(&js_opts) {
    Ok(json) => json,
    Err(_) => {
      println!("[Arnelify Server]: NEON error in wt_create: Invalid JSON in 'c_opts'.");
      return Ok(cx.number(0.0));
    }
  };

  let id: &Mutex<u64> = WT_ID.get_or_init(|| Mutex::new(0));
  let new_id: u64 = {
    let mut js: MutexGuard<'_, u64> = id.lock().unwrap();
    *js += 1;
    *js
  };

  let uds_opts: UnixDomainSocketOpts = UnixDomainSocketOpts {
    block_size_kb: get_usize(&opts, "block_size_kb"),
    socket_path: get_str(&opts, "socket_path"),
    thread_limit: get_u64(&opts, "thread_limit"),
  };

  let uds_wt_close: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          if let (Some(streams_map), Some(uds_map)) = (WT_STREAMS.get(), WT_UDS_MAP.get()) {
            let wt_stream_opt: Option<Arc<Mutex<WebTransportStream>>> =
              streams_map.lock().unwrap().remove(&stream_id);

            if let Some(wt_stream_safe) = wt_stream_opt {
              wt_stream_safe.lock().unwrap().close();
            } else {
              let args: JSON =
                serde_json::json!(["error", "NEON error in uds_wt_close: No stream found."]);
              if let Some(uds) = uds_map.lock().unwrap().get(&new_id) {
                uds.send("wt_logger", &args, Vec::new(), true);
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_wt_push: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let json: JSON = v[1].clone();
          let bytes: Vec<u8> = bytes.lock().unwrap().clone();

          if let Some(map) = WT_STREAMS.get() {
            let wt_stream: Option<Arc<Mutex<WebTransportStream>>> = {
              let streams: MutexGuard<'_, WebTransportStreams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match wt_stream {
              Some(wt_stream_safe) => {
                let mut stream_lock: MutexGuard<'_, WebTransportStream> =
                  wt_stream_safe.lock().unwrap();
                stream_lock.push(&json, &bytes);
              }
              None => {
                let args: JSON =
                  serde_json::json!(["error", "NEON error in uds_wt_push: No stream found."]);

                if let Some(map) = WT_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("wt_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_wt_push_bytes: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let bytes: Vec<u8> = bytes.lock().unwrap().clone();

          if let Some(map) = WT_STREAMS.get() {
            let wt_stream: Option<Arc<Mutex<WebTransportStream>>> = {
              let streams: MutexGuard<'_, WebTransportStreams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match wt_stream {
              Some(wt_stream_safe) => {
                let mut stream_lock: MutexGuard<'_, WebTransportStream> =
                  wt_stream_safe.lock().unwrap();
                stream_lock.push_bytes(&bytes);
              }
              None => {
                let args: JSON =
                  serde_json::json!(["error", "NEON error in uds_wt_push_bytes: No stream found."]);

                if let Some(map) = WT_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("wt_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_wt_push_json: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let json: JSON = v[1].clone();

          if let Some(map) = WT_STREAMS.get() {
            let wt_stream: Option<Arc<Mutex<WebTransportStream>>> = {
              let streams: MutexGuard<'_, WebTransportStreams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match wt_stream {
              Some(wt_stream_safe) => {
                let mut stream_lock: MutexGuard<'_, WebTransportStream> =
                  wt_stream_safe.lock().unwrap();
                stream_lock.push_json(&json);
              }
              None => {
                let args: JSON =
                  serde_json::json!(["error", "NEON error in uds_wt_push_json: No stream found."]);

                if let Some(map) = WT_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("wt_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds_wt_set_compression: Arc<UnixDomainSocketHandler> = Arc::new(
    move |ctx: Arc<Mutex<UnixDomainSocketCtx>>,
          _bytes: Arc<Mutex<UnixDomainSocketBytes>>,
          _stream: Arc<Mutex<UnixDomainSocketStream>>|
          -> () {
      let args: JSON = ctx.lock().unwrap().clone();

      match args.as_array() {
        Some(v) => {
          let stream_id: u64 = v[0].as_u64().unwrap_or(0);
          let compression: &str = v[1].as_str().unwrap_or("");

          if let Some(map) = WT_STREAMS.get() {
            let wt_stream: Option<Arc<Mutex<WebTransportStream>>> = {
              let streams: MutexGuard<'_, WebTransportStreams> = map.lock().unwrap();
              streams.get(&stream_id).cloned()
            };

            match wt_stream {
              Some(wt_stream_safe) => {
                let mut stream_lock: MutexGuard<'_, WebTransportStream> =
                  wt_stream_safe.lock().unwrap();
                if compression.len() > 0 {
                  stream_lock.set_compression(Some(String::from(compression)));
                  return;
                }

                stream_lock.set_compression(None);
              }
              None => {
                let args: JSON = serde_json::json!([
                  "error",
                  "NEON error in uds_wt_set_compression: No stream found."
                ]);

                if let Some(map) = WT_UDS_MAP.get() {
                  if let Some(uds) = map.lock().unwrap().get(&new_id) {
                    uds.send("wt_logger", &args, Vec::new(), true);
                  }
                }
              }
            }
          }
        }
        None => {}
      }
    },
  );

  let uds: UnixDomainSocket = UnixDomainSocket::new(uds_opts);
  uds.on("wt_close", uds_wt_close);
  uds.on("wt_push", uds_wt_push);
  uds.on("wt_push_bytes", uds_wt_push_bytes);
  uds.on("wt_push_json", uds_wt_push_json);
  uds.on("wt_set_compression", uds_wt_set_compression);

  let uds_map: &Mutex<HashMap<u64, Arc<UnixDomainSocket>>> =
    WT_UDS_MAP.get_or_init(|| Mutex::new(HashMap::new()));
  {
    uds_map.lock().unwrap().insert(new_id as u64, Arc::new(uds));
  }

  let wt_opts: WebTransportOpts = WebTransportOpts {
    block_size_kb: get_usize(&opts, "block_size_kb"),
    compression: get_bool(&opts, "compression"),
    cert_pem: get_str(&opts, "cert_pem"),
    handshake_timeout: get_u64(&opts, "handshake_timeout"),
    key_pem: get_str(&opts, "key_pem"),
    max_message_size_kb: get_u64(&opts, "max_message_size_kb"),
    ping_timeout: get_u64(&opts, "ping_timeout"),
    port: get_u16(&opts, "port"),
    send_timeout: get_u64(&opts, "send_timeout"),
    thread_limit: get_u64(&opts, "thread_limit"),
  };

  let wt: WebTransport = WebTransport::new(wt_opts);
  let wt_map: &Mutex<HashMap<u64, Arc<WebTransport>>> =
    WT_MAP.get_or_init(|| Mutex::new(HashMap::new()));
  {
    wt_map.lock().unwrap().insert(new_id as u64, Arc::new(wt));
  }

  Ok(cx.number(new_id as f64))
}

fn wt_destroy(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  if let Some(map) = WT_MAP.get() {
    map.lock().unwrap().remove(&id);
  }

  if let Some(map) = WT_UDS_MAP.get() {
    map.lock().unwrap().remove(&id);
  }

  Ok(cx.undefined())
}

fn wt_logger(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  let wt_logger: Arc<WebTransportLogger> = Arc::new(move |level: &str, message: &str| -> () {
    let args: JSON = serde_json::json!([level, message]);
    let bytes: UnixDomainSocketBytes = Vec::new();

    if let Some(map) = WT_UDS_MAP.get() {
      if let Some(uds) = map.lock().unwrap().get(&id) {
        uds.send("wt_logger", &args, bytes, true);
      }
    }
  });

  if let Some(map) = HTTP1_UDS_MAP.get() {
    if let Some(uds) = map.lock().unwrap().get(&id) {
      uds.logger(Arc::clone(&wt_logger));
    }
  }

  if let Some(map) = WT_MAP.get() {
    if let Some(wt) = map.lock().unwrap().get(&id) {
      wt.logger(Arc::clone(&wt_logger));
    }
  }

  Ok(cx.undefined())
}

fn wt_on(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let topic: String = cx.argument::<JsString>(1)?.value(&mut cx);
  let id: u64 = js_id as u64;

  let topic_safe: String = topic.clone();
  let wt_handler: Arc<WebTransportHandler> = Arc::new(
    move |ctx: Arc<Mutex<WebTransportCtx>>,
          bytes: Arc<Mutex<WebTransportBytes>>,
          stream: Arc<Mutex<WebTransportStream>>|
          -> () {
      let stream_id: u64 = WT_STREAM_ID.fetch_add(1, Ordering::Relaxed);

      WT_STREAMS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .insert(stream_id, stream);

      let ctx: WebTransportCtx = ctx.lock().unwrap().clone();
      let args: JSON = serde_json::json!([stream_id, topic_safe, ctx]);
      let bytes: WebTransportBytes = bytes.lock().unwrap().clone();

      if let Some(map) = WT_UDS_MAP.get() {
        if let Some(uds) = map.lock().unwrap().get(&id) {
          uds.send("wt_on", &args, bytes, true);
        }
      }
    },
  );

  if let Some(map) = WT_MAP.get() {
    if let Some(wt) = map.lock().unwrap().get(&id) {
      wt.on(&topic, wt_handler);
    }
  }

  Ok(cx.undefined())
}

fn wt_start_ipc(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  let (tx, rx) = mpsc::channel::<u8>();

  if let Some(map) = WT_UDS_MAP.get() {
    if let Some(uds) = map.lock().unwrap().get(&id) {
      let uds_safe: Arc<UnixDomainSocket> = Arc::clone(uds);
      thread::spawn(move || {
        uds_safe.start(Arc::new(move || {
          let _ = tx.send(1);
        }));
      });
    }
  }

  loop {
    match rx.recv() {
      Ok(1) => break,
      Ok(_) => continue,
      Err(_) => break,
    }
  }

  Ok(cx.undefined())
}

fn wt_start(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  if let Some(map) = WT_MAP.get() {
    if let Some(wt) = map.lock().unwrap().get(&id) {
      let wt_safe: Arc<WebTransport> = Arc::clone(wt);
      thread::spawn(move || {
        wt_safe.start();
      });
    }
  }

  Ok(cx.undefined())
}

fn wt_stop(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let js_id: f64 = cx.argument::<JsNumber>(0)?.value(&mut cx);
  let id: u64 = js_id as u64;

  if let Some(map) = WT_UDS_MAP.get() {
    if let Some(uds) = map.lock().unwrap().get(&id) {
      uds.stop();
    }
  }

  if let Some(map) = WT_MAP.get() {
    if let Some(wt) = map.lock().unwrap().get(&id) {
      wt.stop();
    }
  }

  Ok(cx.undefined())
}

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {
  cx.export_function("http1_create", http1_create)?;
  cx.export_function("http1_destroy", http1_destroy)?;
  cx.export_function("http1_logger", http1_logger)?;
  cx.export_function("http1_on", http1_on)?;
  cx.export_function("http1_start_ipc", http1_start_ipc)?;
  cx.export_function("http1_start", http1_start)?;
  cx.export_function("http1_stop", http1_stop)?;

  cx.export_function("http2_create", http2_create)?;
  cx.export_function("http2_destroy", http2_destroy)?;
  cx.export_function("http2_logger", http2_logger)?;
  cx.export_function("http2_on", http2_on)?;
  cx.export_function("http2_start_ipc", http2_start_ipc)?;
  cx.export_function("http2_start", http2_start)?;
  cx.export_function("http2_stop", http2_stop)?;

  cx.export_function("ws_create", ws_create)?;
  cx.export_function("ws_destroy", ws_destroy)?;
  cx.export_function("ws_logger", ws_logger)?;
  cx.export_function("ws_on", ws_on)?;
  cx.export_function("ws_start_ipc", ws_start_ipc)?;
  cx.export_function("ws_start", ws_start)?;
  cx.export_function("ws_stop", ws_stop)?;

  cx.export_function("http3_create", http3_create)?;
  cx.export_function("http3_destroy", http3_destroy)?;
  cx.export_function("http3_logger", http3_logger)?;
  cx.export_function("http3_on", http3_on)?;
  cx.export_function("http3_start_ipc", http3_start_ipc)?;
  cx.export_function("http3_start", http3_start)?;
  cx.export_function("http3_stop", http3_stop)?;

  cx.export_function("wt_create", wt_create)?;
  cx.export_function("wt_destroy", wt_destroy)?;
  cx.export_function("wt_logger", wt_logger)?;
  cx.export_function("wt_on", wt_on)?;
  cx.export_function("wt_start_ipc", wt_start_ipc)?;
  cx.export_function("wt_start", wt_start)?;
  cx.export_function("wt_stop", wt_stop)?;

  Ok(())
}
