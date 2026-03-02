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
use std::{
  borrow,
  collections::HashMap,
  fs::{File, Metadata, OpenOptions, metadata},
  io::{BufReader, Error, ErrorKind, Read, Write},
  path::Path,
  process,
  sync::{Arc, Mutex},
  time::{Duration, SystemTime, UNIX_EPOCH},
  vec,
};

use tokio::{
  net::{TcpListener, TcpStream},
  runtime::{Builder, Runtime},
  sync::{Notify, mpsc},
  time::timeout,
};

use tokio_rustls::{
  TlsAcceptor,
  rustls::{Certificate, PrivateKey, ServerConfig},
  server::TlsStream,
};

pub use serde_json::Value as Http2Ctx;
pub use serde_json::Value as JSON;

enum StreamEvent {
  Builder {
    code: u16,
    headers: Vec<(String, String)>,
  },
  BodyChunk {
    chunk: Vec<u8>,
    close: bool,
  },
}

#[derive(Clone, Default)]
pub struct Http2Opts {
  pub allow_empty_files: bool,
  pub block_size_kb: usize,
  pub cert_pem: String,
  pub charset: String,
  pub compression: bool,
  pub keep_alive: u8,
  pub keep_extensions: bool,
  pub key_pem: String,
  pub max_fields: u32,
  pub max_fields_size_total_mb: usize,
  pub max_files: u32,
  pub max_files_size_total_mb: usize,
  pub max_file_size_mb: usize,
  pub port: u16,
  pub storage_path: String,
  pub thread_limit: u64,
}

fn load_certs(path: &str) -> anyhow::Result<Vec<Certificate>> {
  let cert_file: File = File::open(path)?;
  let mut reader: BufReader<File> = BufReader::new(cert_file);
  let certs: Vec<Certificate> = rustls_pemfile::certs(&mut reader)?
    .into_iter()
    .map(Certificate)
    .collect();
  Ok(certs)
}

fn load_key(path: &str) -> anyhow::Result<PrivateKey> {
  let key_file: File = File::open(path)?;
  let mut reader: BufReader<File> = BufReader::new(key_file);
  let keys: Vec<Vec<u8>> = rustls_pemfile::pkcs8_private_keys(&mut reader)?;
  if keys.is_empty() {
    anyhow::bail!("No private keys found");
  }
  Ok(PrivateKey(keys[0].clone()))
}
struct Http2Req {
  opts: Http2Opts,

  has_body: bool,
  has_headers: bool,
  has_method: bool,
  has_path: bool,
  has_protocol: bool,

  path: String,
  buff: Vec<u8>,
  size: usize,

  compression: Option<String>,
  content_type: String,
  content_length: usize,
  skip: usize,

  boundary: Vec<u8>,
  endings: Vec<Vec<u8>>,

  fields: u32,
  fields_size_total: usize,

  keys: Vec<String>,
  body: String,

  files: u32,
  files_size_total: usize,
  file_size: usize,

  file_mime: String,
  file_path: String,
  file_real: String,

  is_write: bool,
  ctx: Http2Ctx,
}

impl Http2Req {
  pub fn new(opts: Http2Opts) -> Self {
    Self {
      opts,

      has_body: true,
      has_headers: false,
      has_method: false,
      has_path: false,
      has_protocol: false,

      path: String::new(),
      buff: Vec::new(),
      size: 0,

      compression: None,
      content_type: String::new(),
      content_length: 0,
      skip: 0,

      fields: 0,
      fields_size_total: 0,

      boundary: Vec::new(),
      endings: Vec::new(),

      keys: Vec::new(),
      body: String::new(),

      files: 0,
      files_size_total: 0,
      file_size: 0,

      file_mime: String::new(),
      file_path: String::new(),
      file_real: String::new(),

      is_write: false,
      ctx: serde_json::json!({
        "_state": {
          "cookie": {},
          "headers": {},
          "method": "GET",
          "path": "/",
          "protocol": "HTTP/2.0",
          "topic": JSON::Null
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

  fn create_file_path(&self) -> String {
    let storage_path: &str = self.opts.storage_path.trim_end_matches('/');

    let now: Duration = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("Time went backwards");
    let millis: u128 = now.as_millis();
    let nanos: u32 = now.subsec_nanos();
    let timestamp: String = format!("{}{}", millis, nanos);

    if self.opts.keep_extensions {
      let extension: String = Path::new(&self.file_real)
        .extension()
        .map(|ext| {
          let ext_str: borrow::Cow<'_, str> = ext.to_string_lossy();
          let sanitized: String = ext_str
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
          sanitized
        })
        .unwrap_or_default();

      if !extension.is_empty() {
        return format!("{}/{}.{}", storage_path, timestamp, extension);
      }
    }

    format!("{}/{}", storage_path, timestamp)
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

  pub fn get_compression(&self) -> Option<String> {
    self.compression.clone()
  }

  pub fn get_ctx(&self) -> Http2Ctx {
    self.ctx.clone()
  }

  pub fn get_path(&self) -> String {
    self.path.clone()
  }

  fn set_body(&mut self) -> Result<u8, Error> {
    if self.body.trim().is_empty() {
      return Ok(1);
    }

    let mut current: &mut JSON = &mut self.ctx["params"]["body"];
    for key in &self.keys {
      current = &mut current[&key];
    }

    let arr: &mut Vec<JSON> = current
      .as_array_mut()
      .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "Invalid field name."))?;

    arr.push(JSON::String(self.body.clone()));

    Ok(1)
  }

  fn set_boundary(&mut self, value: &str) -> Result<u8, Error> {
    let bs: Option<usize> = value.find("boundary=");
    if bs.is_none() {
      return Ok(1);
    }

    let start: usize = bs.unwrap();
    let boundary: String = format!("--{}", &value[start + 9..]);
    self.boundary = boundary.as_bytes().to_vec();

    self.endings.clear();
    for i in 1..self.boundary.len() {
      self.endings.push(self.boundary[..i].to_vec());
    }

    Ok(1)
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

  fn set_content_type(&mut self, value: &str) -> Result<u8, Error> {
    if !self.has_body {
      if let Some(pos) = value.find(';') {
        self.content_type.push_str(&value[..pos]);
        return self.set_boundary(value);
      }
    }

    self.content_type.push_str(&value);
    Ok(1)
  }

  fn set_file(&mut self) -> Result<u8, Error> {
    if self.file_real.is_empty() {
      return Ok(1);
    }

    let path: &Path = Path::new(&self.file_path);
    let ext: String = path
      .extension()
      .and_then(|s| s.to_str())
      .unwrap_or("")
      .to_string();

    let file_name: String = path
      .file_stem()
      .and_then(|s| s.to_str())
      .unwrap_or("")
      .to_string();

    let file: JSON = serde_json::json!({
        "ext": ext,
        "mime": self.file_mime,
        "name": file_name,
        "path": self.file_path,
        "real": self.file_real,
        "size": self.file_size
    });

    let mut current: &mut JSON = &mut self.ctx["params"]["files"];
    for key in &self.keys {
      current = &mut current[&key];
    }

    let arr: &mut Vec<JSON> = current
      .as_array_mut()
      .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "Invalid field name."))?;
    arr.push(file);

    Ok(1)
  }

  fn set_header(&mut self, key: String, value: String) -> Result<u8, Error> {
    self.ctx["_state"]["headers"][&key] = JSON::String(value.to_string());

    if key.eq_ignore_ascii_case("Cookie") {
      return self.set_cookie(&value);
    }

    if key.eq_ignore_ascii_case("Content-Type") {
      return self.set_content_type(&value);
    }

    if key.eq_ignore_ascii_case("Content-Length") {
      let len: usize = value
        .parse()
        .map_err(|_| Error::new(ErrorKind::InvalidData, "Content-Length must be a number."))?;

      self.content_length = len;
      return Ok(1);
    }

    Ok(1)
  }

  fn set_keys(&mut self, key: &str, pattern: &str) -> Result<u8, Error> {
    self.keys.clear();

    if pattern.contains('[') {
      self.keys = pattern
        .split(|c| c == '[' || c == ']')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    } else {
      self.keys.push(String::from(pattern));
    }

    let mut current: &mut JSON = &mut self.ctx["params"][key];
    for key in &self.keys[..self.keys.len() - 1] {
      current = current
        .as_object_mut()
        .unwrap()
        .entry(key.clone())
        .or_insert_with(|| JSON::Object(serde_json::Map::new()));
    }

    let last_key: &str = self.keys.last().unwrap();
    current
      .as_object_mut()
      .unwrap()
      .entry(last_key)
      .or_insert_with(|| JSON::Array(vec![]));

    Ok(1)
  }

  fn set_query(&mut self, value: &str) -> Result<u8, Error> {
    let mut current: &mut JSON = &mut self.ctx["params"]["query"];
    for key in &self.keys {
      current = &mut current[&key];
    }

    let arr: &mut Vec<JSON> = current
      .as_array_mut()
      .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "Invalid query."))?;
    arr.push(JSON::String(String::from(value)));

    Ok(1)
  }

  fn read_meta(&mut self, meta: &[u8]) -> Result<u8, Error> {
    let mut start: usize = 0;
    while let Some(pos) = meta[start..].windows(2).position(|w| w == b"\r\n") {
      let me: usize = start + pos;
      let header: &[u8] = &meta[start..me];

      if header.starts_with(b"Content-Disposition") {
        let name_start: usize =
          header
            .windows(6)
            .position(|w| w == b"name=\"")
            .ok_or_else(|| {
              Error::new(
                ErrorKind::InvalidInput,
                "Invalid Content-Disposition detected in multipart/form-data.",
              )
            })?;

        let name_end: usize = header[name_start + 6..]
          .iter()
          .position(|&b| b == b'"')
          .map(|v| v + name_start + 6)
          .ok_or_else(|| {
            Error::new(
              ErrorKind::InvalidInput,
              "Invalid Content-Disposition detected in multipart/form-data.",
            )
          })?;

        let name_size: usize = name_end - name_start + 6;
        if name_size > 2048 {
          return Err(Error::new(
            ErrorKind::InvalidInput,
            "The maximum size of the field name has been exceeded.",
          ));
        }

        self.fields += 1;
        if self.fields > self.opts.max_fields {
          return Err(Error::new(
            ErrorKind::PermissionDenied,
            "The maximum number of fields has been exceeded.",
          ));
        }

        let pattern: &str = str::from_utf8(&header[name_start + 6..name_end])
          .map_err(|_| Error::new(ErrorKind::InvalidInput, "Invalid UTF-8."))?;
        self.body.clear();

        if let Some(fs) = header.windows(10).position(|w| w == b"filename=\"") {
          let filename_end: usize = header[fs + 10..]
            .iter()
            .position(|&b| b == b'"')
            .map(|v| v + fs + 10)
            .ok_or_else(|| {
              Error::new(
                ErrorKind::InvalidInput,
                "Invalid filename detected in multipart/form-data.",
              )
            })?;

          if filename_end - fs + 10 > 255 {
            return Err(Error::new(
              ErrorKind::InvalidInput,
              "The maximum filename length has been exceeded.",
            ));
          }

          let filename: &str = str::from_utf8(&header[fs + 10..filename_end])
            .map_err(|_| Error::new(ErrorKind::InvalidInput, "Invalid UTF-8."))?;

          self.files += 1;
          if self.files > self.opts.max_files {
            return Err(Error::new(
              ErrorKind::PermissionDenied,
              "The maximum number of files has been exceeded.",
            ));
          }

          self.file_real = filename.to_string();
          self.file_path = self.create_file_path();
          self.file_size = 0;
          self.is_write = true;

          self.set_keys("files", pattern)?;
          start = me + 2;
          continue;
        }

        self.set_keys("body", pattern)?;
        start = me + 2;
        continue;
      }

      if header.starts_with(b"Content-Type") {
        let mime_start: usize = header.windows(2).position(|w| w == b": ").ok_or_else(|| {
          Error::new(
            ErrorKind::InvalidInput,
            "Invalid Content-Type detected in multipart/form-data.",
          )
        })?;

        let mime: &str = str::from_utf8(&header[mime_start + 2..])
          .map_err(|_| Error::new(ErrorKind::InvalidInput, "Invalid UTF-8."))?;
        self.file_mime = mime.to_string();

        start = me + 2;
        continue;
      }

      start = me + 2;
    }

    Ok(1)
  }

  fn read_multipart(&mut self) -> Result<Option<u8>, Error> {
    for ending in &self.endings {
      if self.buff.ends_with(ending) {
        return Ok(None);
      }
    }

    let boundary_len: usize = self.boundary.len();
    let mut boundary_start: Option<usize> = self
      .buff
      .windows(boundary_len)
      .position(|w| w == self.boundary);

    while let Some(bs) = boundary_start {
      let before: &[u8] = &self.buff[..bs];
      let has_before: bool = bs > 0;

      if has_before {
        if self.size + bs >= self.content_length {
          self.buff.clear();
          return Err(Error::new(
            ErrorKind::InvalidData,
            "The maximum size of body has been exceeded.",
          ));
        }

        self.size += bs;

        let chunk_len: usize = bs - 2;
        let chunk_bytes: Vec<u8> = before[..chunk_len].to_vec();
        if self.is_write {
          if !self.file_real.is_empty() && !self.opts.allow_empty_files && chunk_len == 0 {
            self.buff.drain(..bs);
            self.skip = self.content_length - self.size;
            return Err(Error::new(
              ErrorKind::PermissionDenied,
              "Empty files are not allowed.",
            ));
          }

          self.file_size += chunk_len;
          self.files_size_total += chunk_len;

          if self.file_size > self.opts.max_file_size_mb * 1024 * 1024 {
            self.buff.drain(..bs);
            self.skip = self.content_length - self.size;
            return Err(Error::new(
              ErrorKind::PermissionDenied,
              "The maximum size of the file has been exceeded.",
            ));
          }

          if self.files_size_total > self.opts.max_files_size_total_mb * 1024 * 1024 {
            self.buff.drain(..bs);
            self.skip = self.content_length - self.size;
            return Err(Error::new(
              ErrorKind::PermissionDenied,
              "The maximum size of files has been exceeded.",
            ));
          }

          match self.write(&chunk_bytes) {
            Ok(_) => {}
            Err(e) => {
              self.buff.drain(..bs);
              self.skip = self.content_length - self.size;
              return Err(e);
            }
          }

          match self.set_file() {
            Ok(_) => {}
            Err(e) => {
              self.buff.drain(..bs);
              self.skip = self.content_length - self.size;
              return Err(e);
            }
          }

          self.is_write = false;
        } else {
          self.fields_size_total += chunk_len;
          if self.fields_size_total > self.opts.max_fields_size_total_mb * 1024 * 1024 {
            self.buff.drain(..bs);
            self.skip = self.content_length - self.size;
            return Err(Error::new(
              ErrorKind::PermissionDenied,
              "The maximum size of fields has been exceeded.",
            ));
          }

          let chunk: &str = match std::str::from_utf8(&chunk_bytes) {
            Ok(v) => v,
            Err(_) => {
              self.buff.drain(..bs);
              self.skip = self.content_length - self.size;
              return Err(Error::new(ErrorKind::InvalidInput, "Invalid UTF-8."));
            }
          };

          self.body.push_str(chunk);
          match self.set_body() {
            Ok(_) => {}
            Err(e) => {
              self.buff.drain(..bs);
              self.skip = self.content_length - self.size;
              return Err(e);
            }
          }
        }

        self.buff.drain(..bs);
        boundary_start = self
          .buff
          .windows(boundary_len)
          .position(|w| w == self.boundary);
      }

      if self.buff.starts_with(&self.boundary) && self.buff[boundary_len..].starts_with(b"--\r\n") {
        if self.size + boundary_len + 4 > self.content_length {
          self.buff.clear();
          return Err(Error::new(
            ErrorKind::InvalidData,
            "The maximum size of the body has been exceeded.",
          ));
        }

        self.size += boundary_len + 4;
        self.has_body = true;

        self.buff.drain(..boundary_len + 4);
        return Ok(Some(1));
      }

      let bs: usize = match boundary_start {
        Some(v) => v,
        None => return Ok(None),
      };

      let boundary_end: usize = match self.buff[bs..].windows(2).position(|w| w == b"\r\n") {
        Some(v) => v + bs,
        None => return Ok(None),
      };

      let meta_end: usize = match self.buff[boundary_end + 2..]
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
      {
        Some(v) => v + boundary_end + 2,
        None => return Ok(None),
      };

      if self.size + meta_end + 4 >= self.content_length {
        self.buff.clear();
        return Err(Error::new(
          ErrorKind::InvalidData,
          "The maximum size of the body has been exceeded.",
        ));
      }

      self.size += meta_end + 4;
      let meta_bytes: Vec<u8> = self.buff[boundary_end + 2..meta_end + 2].to_vec();
      match self.read_meta(&meta_bytes) {
        Ok(_) => {}
        Err(e) => {
          self.buff.drain(..meta_end + 4);
          self.skip = self.content_length - self.size;
          return Err(e);
        }
      }

      self.buff.drain(..meta_end + 4);
      boundary_start = self
        .buff
        .windows(self.boundary.len())
        .position(|w| w == self.boundary);
    }

    if self.buff.is_empty() {
      return Ok(None);
    }

    let len: usize = self.buff.len();
    if self.size + len >= self.content_length {
      self.buff.clear();
      return Err(Error::new(
        ErrorKind::InvalidData,
        "The maximum size of the body has been exceeded.",
      ));
    }

    self.size += len;

    if self.is_write {
      self.file_size += len;
      if self.file_size > self.opts.max_file_size_mb * 1024 * 1024 {
        self.buff.clear();
        self.skip = self.content_length - self.size;
        return Err(Error::new(
          ErrorKind::PermissionDenied,
          "The maximum size of the file has been exceeded.",
        ));
      }

      self.files_size_total += len;
      if self.files_size_total > self.opts.max_files_size_total_mb * 1024 * 1024 {
        self.buff.clear();
        self.skip = self.content_length - self.size;
        return Err(Error::new(
          ErrorKind::PermissionDenied,
          "The maximum size of files has been exceeded.",
        ));
      }

      let chunk_bytes: Vec<u8> = self.buff.to_vec();
      self.write(&chunk_bytes)?;
      self.buff.clear();

      return Ok(None);
    }

    self.fields_size_total += len;
    if self.fields_size_total > self.opts.max_fields_size_total_mb * 1024 * 1024 {
      self.buff.clear();
      self.skip = self.content_length - self.size;
      return Err(Error::new(
        ErrorKind::PermissionDenied,
        "The maximum size of fields has been exceeded.",
      ));
    }

    let body: &str = match std::str::from_utf8(&self.buff) {
      Ok(v) => v,
      Err(_) => {
        self.skip = self.content_length - self.size;
        return Err(Error::new(ErrorKind::InvalidInput, "Invalid UTF-8."));
      }
    };

    self.body.push_str(body);
    self.buff.clear();

    Ok(None)
  }

  fn read_json(&mut self) -> Result<Option<u8>, Error> {
    if self.content_length >= self.opts.max_fields_size_total_mb * 1024 {
      self.buff.clear();
      return Err(Error::new(
        ErrorKind::InvalidData,
        "The maximum size of fields has been exceeded.",
      ));
    }

    if self.buff.len() >= self.content_length {
      self.size = self.content_length;

      let body: &str = match std::str::from_utf8(&self.buff[..self.content_length]) {
        Ok(v) => v,
        Err(_) => {
          self.buff.drain(..self.content_length);
          self.skip = self.content_length - self.size;
          return Err(Error::new(ErrorKind::InvalidInput, "Invalid UTF-8."));
        }
      };

      let json: JSON = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => {
          self.buff.drain(..self.content_length);
          self.skip = self.content_length - self.size;
          return Err(Error::new(
            ErrorKind::InvalidInput,
            "Invalid application/json.",
          ));
        }
      };

      self.ctx["params"]["body"] = json;

      self.has_body = true;
      self.buff.drain(..self.content_length);
      return Ok(Some(1));
    }

    Ok(None)
  }

  fn read_url_encoded(&mut self) -> Result<Option<u8>, Error> {
    if self.content_length >= self.opts.max_fields_size_total_mb * 1024 {
      self.buff.clear();
      return Err(Error::new(
        ErrorKind::InvalidData,
        "The maximum size of fields has been exceeded.",
      ));
    }

    if self.buff.len() >= self.content_length {
      self.size = self.content_length;

      let query_bytes: Vec<u8> = self.decode(&self.buff[..self.content_length]);
      for pair_bytes in query_bytes.split(|&b| b == b'&') {
        let mut split_iter = pair_bytes.split(|&b| b == b'=');
        if let Some(pattern_bytes) = split_iter.next() {
          let pattern: &str = match std::str::from_utf8(pattern_bytes) {
            Ok(v) => v,
            Err(_) => {
              self.buff.drain(..self.content_length);
              self.skip = self.content_length - self.size;
              return Err(Error::new(ErrorKind::InvalidInput, "Invalid UTF-8."));
            }
          };

          let value_bytes: Option<&[u8]> = split_iter.next();
          let value: &str = if let Some(vb) = value_bytes {
            match str::from_utf8(vb) {
              Ok(v) => v,
              Err(_) => {
                self.buff.drain(..self.content_length);
                self.skip = self.content_length - self.size;
                return Err(Error::new(ErrorKind::InvalidInput, "Invalid UTF-8."));
              }
            }
          } else {
            ""
          };

          self.body = value.to_string();
          self.set_keys("body", pattern)?;
          match self.set_body() {
            Ok(_) => {}
            Err(e) => {
              self.buff.drain(..self.content_length);
              self.skip = self.content_length;
              return Err(e);
            }
          }
        }
      }

      self.has_body = true;
      self.buff.drain(..self.content_length);
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
    let is_support: bool = matches!(
      method_bytes,
      b"GET"
        | b"POST"
        | b"PUT"
        | b"DELETE"
        | b"HEAD"
        | b"OPTIONS"
        | b"PATCH"
        | b"CONNECT"
        | b"TRACE"
    );

    if !is_support {
      self.buff.clear();
      return Err(Error::new(
        ErrorKind::InvalidData,
        "Unknown request method.",
      ));
    }

    if matches!(method_bytes, b"PATCH" | b"POST" | b"PUT" | b"DELETE") {
      self.has_body = false;
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

    let (path_encoded, query_encoded) = match query_pos {
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

    self.path = String::from(path);
    self.ctx["_state"]["path"] = JSON::String(String::from(path));

    if let Some(qs_encoded) = query_encoded {
      let qs_bytes: Vec<u8> = qs_encoded.to_vec();
      let query_bytes: Vec<u8> = self.decode(&qs_bytes);

      for pair_bytes in query_bytes.split(|&b| b == b'&') {
        let mut split_iter = pair_bytes.split(|&b| b == b'=');
        if let Some(pattern_bytes) = split_iter.next() {
          let pattern: &str = match std::str::from_utf8(&pattern_bytes) {
            Ok(v) => v,
            Err(_) => {
              self.buff.clear();
              return Err(Error::new(ErrorKind::InvalidData, "Invalid UTF-8."));
            }
          };

          let value_bytes: Option<&[u8]> = split_iter.next();
          let value: &str = if let Some(vb) = value_bytes {
            match str::from_utf8(vb) {
              Ok(v) => v,
              Err(_) => {
                self.buff.clear();
                return Err(Error::new(ErrorKind::InvalidData, "Invalid UTF-8."));
              }
            }
          } else {
            ""
          };

          match self.set_keys("query", pattern) {
            Ok(_) => {}
            Err(e) => return Err(e),
          }

          match self.set_query(value) {
            Ok(_) => {}
            Err(e) => return Err(e),
          }
        }
      }
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

  fn read_body(&mut self) -> Result<Option<u8>, Error> {
    if self.size > self.content_length {
      return Err(Error::new(
        ErrorKind::InvalidData,
        "The maximum size of the body has been exceeded.",
      ));
    }

    if self.content_type == "application/json" {
      return self.read_json();
    }

    if self.content_type == "multipart/form-data" && !self.boundary.is_empty() {
      return self.read_multipart();
    }

    if self.content_type == "application/x-www-form-urlencoded" {
      return self.read_url_encoded();
    }

    Ok(Some(1))
  }

  pub fn read_block(&mut self) -> Result<Option<u8>, Error> {
    if self.skip > 0 {
      let buff_len: usize = self.buff.len();
      if buff_len >= self.skip {
        self.buff.drain(..self.skip);
        self.skip = 0;

        return Ok(Some(1));
      }

      self.buff.clear();
      self.skip = self.skip - buff_len;
      return Ok(None);
    }

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
    self.has_body = true;
    self.has_headers = false;
    self.has_method = false;
    self.has_path = false;
    self.has_protocol = false;

    self.path.clear();
    self.size = 0;

    self.compression = None;
    self.content_type.clear();
    self.content_length = 0;
    self.skip = 0;

    self.boundary.clear();
    self.endings.clear();

    self.fields = 0;
    self.fields_size_total = 0;

    self.keys.clear();
    self.body.clear();

    self.files = 0;
    self.files_size_total = 0;
    self.file_size = 0;

    self.file_mime.clear();
    self.file_path.clear();
    self.file_real.clear();

    self.is_write = false;
    self.ctx = serde_json::json!({
      "_state": {
        "cookie": {},
        "headers": {},
        "method": "GET",
        "path": "/",
        "protocol": "HTTP/2.0",
        "topic": JSON::Null
      },
      "params": {
        "files": {},
        "body": {},
        "query": {}
      }
    });
  }

  pub fn write(&mut self, block: &[u8]) -> Result<u8, Error> {
    if self.file_real.is_empty() {
      return Ok(1);
    }

    let mut file: File = match OpenOptions::new()
      .create(true)
      .append(true)
      .open(&self.file_path)
    {
      Ok(f) => f,
      Err(_) => {
        return Err(Error::new(ErrorKind::PermissionDenied, "File save error."));
      }
    };

    if let Err(_) = file.write_all(block) {
      return Err(Error::new(ErrorKind::WriteZero, "File write error."));
    }

    Ok(1)
  }
}

pub struct Http2Stream {
  opts: Http2Opts,
  body: Option<Vec<u8>>,
  cb_logger: Arc<Http2Logger>,
  cb_builder: Arc<dyn Fn(u16, &[(String, String)]) + Send + Sync>,
  cb_send: Arc<dyn Fn(&[u8], bool) + Send + Sync>,
  code: u16,
  compression: Option<String>,
  content_length: usize,
  content_type: String,
  encoding: String,
  file_path: Option<String>,
  headers: Vec<(String, String)>,
  headers_sent: bool,
  is_attachment: bool,
}

impl Http2Stream {
  pub fn new(opts: Http2Opts) -> Self {
    Self {
      opts,
      body: None,
      cb_logger: Arc::new(|_level: &str, message: &str| {
        println!("[Arnelify Server]: {}", message);
      }),
      cb_builder: Arc::new(|code: u16, headers: &[(String, String)]| {
        println!("code: {}", code);
        println!("headers: {:?}", headers);
      }),
      cb_send: Arc::new(|chunk: &[u8], _flush: bool| {
        let data: String = match str::from_utf8(&chunk) {
          Ok(s) => String::from(s),
          Err(_) => String::from("<bytes>"),
        };

        println!("{}", data);
      }),
      code: 200,
      compression: None,
      content_length: 0,
      content_type: String::from("application/json"),
      encoding: String::from("utf-8"),
      file_path: None,
      headers: Vec::new(),
      headers_sent: false,
      is_attachment: false,
    }
  }

  pub fn add_header(&mut self, key: &str, value: &str) -> () {
    for (k, _v) in &self.headers {
      if key.eq_ignore_ascii_case("Content-Disposition")
        && k.eq_ignore_ascii_case("Content-Disposition")
      {
        return;
      }

      if key.eq_ignore_ascii_case("Content-Length") && k.eq_ignore_ascii_case("Content-Length") {
        return;
      }

      if key.eq_ignore_ascii_case("Content-Type") && k.eq_ignore_ascii_case("Content-Type") {
        return;
      }
    }

    self.headers.push((key.to_string(), value.to_string()));
  }

  pub fn end(&mut self) -> () {
    (self.cb_send)(&[], true);
    self.reset();
  }

  fn get_headers(&self) -> Vec<(String, String)> {
    let mut buff: Vec<(String, String)> = Vec::new();
    for (key, value) in &self.headers {
      buff.push((String::from(key), String::from(value)));
    }
    buff.push((String::from("Server"), String::from("Arnelify Server")));
    buff
  }

  fn get_mime(&self, ext: &str) -> String {
    match ext {
      ".avi" => String::from("video/x-msvideo"),
      ".css" => format!("text/css; charset={}", self.encoding),
      ".csv" => format!("text/csv; charset={}", self.encoding),
      ".eot" => String::from("font/eot"),
      ".gif" => String::from("image/gif"),
      ".htm" | ".html" => format!("text/html; charset={}", self.encoding),
      ".ico" => String::from("image/x-icon"),
      ".jpeg" | ".jpg" => String::from("image/jpeg"),
      ".js" => format!("application/javascript; charset={}", self.encoding),
      ".json" => format!("application/json; charset={}", self.encoding),
      ".mkv" => String::from("video/x-matroska"),
      ".mov" => String::from("video/quicktime"),
      ".mp3" => String::from("audio/mpeg"),
      ".mp4" => String::from("video/mp4"),
      ".otf" => String::from("font/otf"),
      ".png" => String::from("image/png"),
      ".svg" => format!("image/svg+xml; charset={}", self.encoding),
      ".ttf" => String::from("font/ttf"),
      ".txt" => format!("text/plain; charset={}", self.encoding),
      ".wasm" => String::from("application/wasm"),
      ".wav" => String::from("audio/wav"),
      ".weba" => String::from("audio/webm"),
      ".webp" => String::from("image/webp"),
      ".woff" => String::from("font/woff"),
      ".woff2" => String::from("font/woff2"),
      ".xml" => format!("application/xml; charset={}", self.encoding),
      _ => String::from("application/octet-stream"),
    }
  }

  pub fn on_builder(&mut self, cb: Arc<dyn Fn(u16, &[(String, String)]) + Send + Sync>) {
    self.cb_builder = cb;
  }

  pub fn on_send(&mut self, cb: Arc<dyn Fn(&[u8], bool) + Send + Sync>) {
    self.cb_send = cb;
  }

  fn send(&mut self) -> () {
    if let Some(ref file_path) = self.file_path {
      let path: &Path = Path::new(file_path);
      let mut file: File = match File::open(path) {
        Ok(f) => f,
        Err(_) => {
          (self.cb_logger)("error", "File not found.");
          process::exit(1);
        }
      };

      if !self.headers_sent {
        let meta: Metadata = match metadata(&path) {
          Ok(m) => m,
          Err(_) => {
            (self.cb_logger)("error", "File not found.");
            process::exit(1);
          }
        };

        self.content_length = meta.len() as usize;
        let file_ext: &str = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        self.content_type = self.get_mime(&format!(".{}", file_ext));
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
          Some(name) => name,
          None => "file",
        };

        if self.is_attachment {
          self.add_header(
            "Content-Disposition",
            &format!("attachment; filename=\"{}.{}\"", file_name, file_ext),
          );
        }

        self.add_header("Content-Length", &self.content_length.to_string());
        self.add_header("Content-Type", &self.content_type.to_string());
        (self.cb_builder)(self.code, &self.get_headers());
        self.headers_sent = true;
      }

      let block_size: usize = self.opts.block_size_kb * 1024;
      let mut buff: Vec<u8> = vec![0u8; block_size];

      if self.opts.compression && self.compression.is_some() {
        if let Some(_compression) = &self.compression {
          //TODO: Brotli
        }
      }

      loop {
        let bytes_read: usize = match file.read(&mut buff) {
          Ok(0) => break,
          Ok(n) => n,
          Err(_) => {
            (self.cb_logger)("error", &format!("File not found."));
            process::exit(1);
          }
        };

        (self.cb_send)(&buff[..bytes_read], false);
      }

      return;
    }

    if self.body.is_some() {
      if !self.headers_sent {
        if self.is_attachment {
          let now: SystemTime = SystemTime::now();
          let unixtime: u64 = now
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64;

          self.add_header(
            "Content-Disposition",
            &format!("attachment; filename=\"{}\"", unixtime),
          );
        }

        self.add_header("Content-Length", &self.content_length.to_string());
        self.add_header("Content-Type", &self.content_type.to_string());
        (self.cb_builder)(self.code, &self.get_headers());
        self.headers_sent = true;
      }

      if self.opts.compression && self.compression.is_some() {
        if let Some(_compression) = &self.compression {
          //TODO: Brotli
        }
      }

      if let Some(ref b) = self.body {
        (self.cb_send)(&b, false);
      }
    }
  }

  pub fn set_encoding(&mut self, encoding: String) -> () {
    self.encoding = encoding;
  }

  pub fn set_code(&mut self, code: u16) -> () {
    self.code = code;
  }

  pub fn set_compression(&mut self, compression: Option<String>) -> () {
    self.compression = compression;
  }

  pub fn set_headers(&mut self, headers: Vec<(String, String)>) -> () {
    self.headers = headers;
  }

  pub fn push_bytes(&mut self, bytes: &[u8], is_attachment: bool) -> () {
    if self.headers_sent {
      if self.is_attachment != is_attachment {
        (self.cb_logger)("error", "Can't push body in attachment mode.");
        process::exit(1);
      }
    } else {
      self.is_attachment = is_attachment;
    }

    if self.file_path.is_some() {
      (self.cb_logger)("error", "Can't push body after file.");
      process::exit(1);
    }

    self.body = Some(bytes.to_vec());
    self.content_length = bytes.len();
    self.send();
  }

  pub fn push_file(&mut self, file_path: &str, is_attachment: bool) -> () {
    if self.headers_sent {
      if self.is_attachment != is_attachment {
        (self.cb_logger)("error", "Can't push file in attachment mode.");
        process::exit(1);
      }
    } else {
      self.is_attachment = is_attachment;
    }

    if self.body.is_some() {
      (self.cb_logger)("error", "Can't push file after body.");
      process::exit(1);
    }

    self.file_path = Some(String::from(file_path));
    self.send();
  }

  pub fn push_json(&mut self, json: &JSON, is_attachment: bool) -> () {
    if self.headers_sent {
      if self.is_attachment != is_attachment {
        (self.cb_logger)("error", "Can't push body in attachment mode.");
        process::exit(1);
      }
    } else {
      self.is_attachment = is_attachment;
    }

    if self.file_path.is_some() {
      (self.cb_logger)("error", "Can't push body after file.");
      process::exit(1);
    }

    let chunk: String = serde_json::to_string(&json).unwrap();
    let bytes_len: usize = chunk.len();

    self.body = Some(chunk.into_bytes());
    self.content_length = bytes_len;
    self.send();
  }

  pub fn reset(&mut self) -> () {
    self.body = None;
    self.code = 200;
    self.compression = None;
    self.content_length = 0;
    self.content_type.clear();
    self.content_type.push_str("application/json");
    self.encoding.clear();
    self.encoding.push_str("utf-8");
    self.file_path = None;
    self.headers = Vec::new();
    self.headers_sent = false;
    self.is_attachment = false;
  }
}

pub type Http2Handler = dyn Fn(Arc<Mutex<Http2Ctx>>, Arc<Mutex<Http2Stream>>) + Send + Sync;
pub type Http2Logger = dyn Fn(&str, &str) + Send + Sync;

pub struct Http2 {
  cb_logger: Arc<Mutex<Arc<Http2Logger>>>,
  handlers: Arc<Mutex<HashMap<String, Arc<Http2Handler>>>>,
  opts: Http2Opts,
  shutdown: Arc<Notify>,
}

impl Http2 {
  pub fn new(opts: Http2Opts) -> Self {
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
    listener: &TcpListener,
    tls_acceptor: &TlsAcceptor,
    logger_rt: Arc<Mutex<Arc<Http2Logger>>>,
    handlers_rt: Arc<Mutex<HashMap<String, Arc<Http2Handler>>>>,
    opts_rt: Arc<Http2Opts>,
  ) -> () {
    let keep_alive: Duration = Duration::from_secs((*opts_rt).keep_alive as u64);
    match listener.accept().await {
      Ok((socket, addr_bytes)) => {
        let acceptor: TlsAcceptor = tls_acceptor.clone();
        let addr: String = addr_bytes.to_string();

        let logger_accept: Arc<Mutex<Arc<Http2Logger>>> = Arc::clone(&logger_rt);
        let handlers_accept: Arc<Mutex<HashMap<String, Arc<Http2Handler>>>> =
          Arc::clone(&handlers_rt);
        let opts_accept: Arc<Http2Opts> = Arc::clone(&opts_rt);

        {
          let logger_lock: std::sync::MutexGuard<'_, Arc<Http2Logger>> =
            logger_accept.lock().unwrap();
          logger_lock("warning", &format!("New connection from {:?}", addr));
        }

        // ACCEPT TASK
        tokio::spawn(async move {
          let tls: TlsStream<TcpStream> = match acceptor.accept(socket).await {
            Ok(s) => s,
            Err(e) => {
              let logger_lock: std::sync::MutexGuard<'_, Arc<Http2Logger>> =
                logger_accept.lock().unwrap();
              logger_lock("warning", &format!("TLS error: {}", e));
              return;
            }
          };

          let mut conn: h2::server::Connection<TlsStream<TcpStream>, Bytes> =
            match h2::server::handshake(tls).await {
              Ok(c) => c,
              Err(e) => {
                let logger_lock: std::sync::MutexGuard<'_, Arc<Http2Logger>> =
                  logger_accept.lock().unwrap();
                logger_lock("warning", &format!("Handshake error: {}", e));
                return;
              }
            };

          loop {
            match timeout(keep_alive, conn.accept()).await {
              Ok(Some(Ok((h, mut s)))) => {
                let (tx, mut rx) = mpsc::channel::<StreamEvent>(32);

                // WRITE TASK
                tokio::spawn(async move {
                  let mut stream: Option<h2::SendStream<Bytes>> = None;

                  while let Some(event) = rx.recv().await {
                    match event {
                      StreamEvent::Builder { code, headers } => {
                        let mut builder: http::response::Builder = http::Response::builder();
                        let status: http::StatusCode = http::StatusCode::from_u16(code).unwrap();
                        builder = builder.status(status);
                        for (k, v) in headers {
                          if let (Ok(name), Ok(value)) = (
                            http::HeaderName::from_bytes(k.as_bytes()),
                            http::HeaderValue::from_str(&v),
                          ) {
                            builder = builder.header(name, value);
                          }
                        }

                        let response: http::Response<()> = builder.body(()).unwrap();
                        stream = Some(s.send_response(response, false).unwrap());
                      }
                      StreamEvent::BodyChunk { chunk, close } => {
                        if let Some(writer) = stream.as_mut() {
                          writer.reserve_capacity(chunk.len());
                          let _ = writer.send_data(Bytes::from(chunk), close);
                          if close {
                            break;
                          }
                        }
                      }
                    }
                  }
                });

                let block_size: usize = (*opts_accept).block_size_kb * 1024;
                let mut buff: Vec<u8> = Vec::with_capacity(block_size);
                let mut req: Http2Req = Http2Req::new((*opts_accept).clone());
                let stream: Arc<Mutex<Http2Stream>> =
                  Arc::new(Mutex::new(Http2Stream::new((*opts_accept).clone())));
                {
                  let mut stream_lock: std::sync::MutexGuard<'_, Http2Stream> =
                    stream.lock().unwrap();
                  stream_lock.on_builder(Arc::new({
                    let tx = tx.clone();
                    move |code: u16, headers: &[(String, String)]| {
                      let _ = tx.try_send(StreamEvent::Builder {
                        code,
                        headers: headers.to_vec(),
                      });
                    }
                  }));

                  stream_lock.on_send(Arc::new({
                    let tx: mpsc::Sender<StreamEvent> = tx.clone();
                    move |chunk: &[u8], close: bool| {
                      let _ = tx.try_send(StreamEvent::BodyChunk {
                        chunk: chunk.to_vec(),
                        close,
                      });
                    }
                  }));
                }

                let location: &str = h
                  .uri()
                  .path_and_query()
                  .map(|pq| pq.as_str())
                  .unwrap_or(h.uri().path());

                buff.extend_from_slice(h.method().as_str().as_bytes());
                buff.extend_from_slice(b" ");
                buff.extend_from_slice(location.as_bytes());
                buff.extend_from_slice(b" HTTP/1.1\r\n");

                for (k, v) in h.headers() {
                  buff.extend_from_slice(k.as_str().as_bytes());
                  buff.extend_from_slice(b": ");
                  buff.extend_from_slice(v.as_bytes());
                  buff.extend_from_slice(b"\r\n");
                }

                buff.extend_from_slice(b"\r\n");
                req.add(&buff);

                let mut res: Result<Option<u8>, Error> = req.read_block();
                let mut recv: h2::RecvStream = h.into_body();
                while let Some(chunk) = recv.data().await {
                  match chunk {
                    Ok(bytes) => {
                      req.add(&bytes[..bytes.len()]);
                      match req.read_block() {
                        Ok(Some(_)) => {
                          res = Ok(Some(1));
                          break;
                        }
                        Ok(None) => {}
                        Err(e) => match e.kind() {
                          ErrorKind::InvalidData => {
                            break;
                          }
                          _ => {}
                        },
                      }
                    }
                    Err(e) => {
                      let logger_lock: std::sync::MutexGuard<'_, Arc<Http2Logger>> =
                        logger_accept.lock().unwrap();
                      logger_lock("warning", &format!("Socket read error: {}", e));
                      break;
                    }
                  }
                }

                match res {
                  Ok(Some(_)) => {
                    let path: String = req.get_path();
                    let ctx_handler: Arc<Mutex<Http2Ctx>> = Arc::new(Mutex::new(req.get_ctx()));
                    let stream_handler: Arc<Mutex<Http2Stream>> = Arc::clone(&stream);
                    {
                      let mut stream_lock: std::sync::MutexGuard<'_, Http2Stream> =
                        stream.lock().unwrap();
                      stream_lock.set_encoding((*opts_accept).charset.clone());
                      stream_lock.set_compression(req.get_compression());
                    };

                    req.reset();
                    let handler_opt: Option<Arc<Http2Handler>> = {
                      let handlers_lock: std::sync::MutexGuard<
                        '_,
                        HashMap<String, Arc<Http2Handler>>,
                      > = handlers_accept.lock().unwrap();
                      handlers_lock.get(&path).cloned()
                    };

                    if let Some(handler) = handler_opt {
                      handler(ctx_handler, stream_handler);

                      let mut stream_lock: std::sync::MutexGuard<'_, Http2Stream> =
                        stream.lock().unwrap();
                      stream_lock.reset();
                    } else {
                      let mut stream_lock: std::sync::MutexGuard<'_, Http2Stream> =
                        stream.lock().unwrap();
                      stream_lock.set_code(404);
                      stream_lock.push_json(
                        &serde_json::json!({
                          "code": 404,
                          "error": "Not Found."
                        }),
                        false,
                      );

                      stream_lock.end();
                    }
                  }
                  Ok(None) => {}
                  Err(e) => match e.kind() {
                    ErrorKind::InvalidData => {
                      let logger_lock: std::sync::MutexGuard<'_, Arc<Http2Logger>> =
                        logger_accept.lock().unwrap();
                      logger_lock("warning", &format!("Block read error: {}", e));
                      break;
                    }
                    _ => {
                      let mut stream_lock: std::sync::MutexGuard<'_, Http2Stream> =
                        stream.lock().unwrap();
                      stream_lock.set_code(409);
                      stream_lock.set_encoding((*opts_accept).charset.clone());
                      stream_lock.push_json(
                        &serde_json::json!({
                          "code": 409,
                          "error": format!("{}", e)
                        }),
                        false,
                      );

                      stream_lock.end();
                    }
                  },
                }
              }
              Ok(Some(Err(e))) => {
                let logger_lock: std::sync::MutexGuard<'_, Arc<Http2Logger>> =
                  logger_accept.lock().unwrap();
                logger_lock("warning", &format!("Stream accept error: {}", e));
                break;
              }
              Ok(None) => {
                let logger_lock: std::sync::MutexGuard<'_, Arc<Http2Logger>> =
                  logger_accept.lock().unwrap();
                logger_lock("success", &format!("Client {:?} disconnected", addr));
                break;
              }
              Err(_) => {
                let logger_lock: std::sync::MutexGuard<'_, Arc<Http2Logger>> =
                  logger_accept.lock().unwrap();
                logger_lock("success", &format!("Keep-Alive timeout for {:?}", addr));
                break;
              }
            }
          }
        });
      }
      Err(e) => {
        let logger_lock: std::sync::MutexGuard<'_, Arc<Http2Logger>> = logger_rt.lock().unwrap();
        logger_lock("error", &format!("Acceptor error: {}", e));
      }
    }
  }

  pub fn logger(&self, cb: Arc<Http2Logger>) {
    let mut logger_lock: std::sync::MutexGuard<'_, Arc<Http2Logger>> =
      self.cb_logger.lock().unwrap();
    *logger_lock = cb;
  }

  pub fn on(&self, path: &str, cb: Arc<Http2Handler>) {
    let mut map: std::sync::MutexGuard<'_, HashMap<String, Arc<Http2Handler>>> =
      self.handlers.lock().unwrap();
    map.insert(String::from(path), cb);
  }

  pub fn start(&self) {
    let logger_rt: Arc<Mutex<Arc<Http2Logger>>> = Arc::clone(&self.cb_logger);
    let handlers_rt: Arc<Mutex<HashMap<String, Arc<Http2Handler>>>> = Arc::clone(&self.handlers);
    let opts_rt: Arc<Http2Opts> = Arc::new(self.opts.clone());
    let shutdown_rt: Arc<Notify> = Arc::clone(&self.shutdown);

    let rt: Runtime = Builder::new_multi_thread()
      .worker_threads(self.opts.thread_limit as usize)
      .enable_all()
      .build()
      .unwrap();

    rt.block_on(async move {
      let certs: Vec<Certificate> = match load_certs(&(*opts_rt).cert_pem) {
        Ok(c) => c,
        Err(e) => {
          let logger_lock: std::sync::MutexGuard<'_, Arc<Http2Logger>> = logger_rt.lock().unwrap();
          logger_lock("error", &format!("Failed to load \"cert_pem\": {}", e));
          process::exit(1);
        }
      };

      let key: PrivateKey = match load_key(&(*opts_rt).key_pem) {
        Ok(k) => k,
        Err(e) => {
          let logger_lock: std::sync::MutexGuard<'_, Arc<Http2Logger>> = logger_rt.lock().unwrap();
          logger_lock("error", &format!("Failed to load \"key_pem\": {}", e));
          process::exit(1);
        }
      };

      let mut tls_config: ServerConfig = match ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
      {
        Ok(cfg) => cfg,
        Err(e) => {
          let logger_lock: std::sync::MutexGuard<'_, Arc<Http2Logger>> = logger_rt.lock().unwrap();
          logger_lock("error", &format!("TLS config error: {}", e));
          process::exit(1);
        }
      };

      tls_config.alpn_protocols = vec![b"h2".to_vec()];
      let tls_acceptor: TlsAcceptor = TlsAcceptor::from(Arc::new(tls_config));

      let addr: (&str, u16) = ("0.0.0.0", (*opts_rt).port);
      let listener: TcpListener = match TcpListener::bind(&addr).await {
        Ok(v) => v,
        Err(_) => {
          let logger_lock: std::sync::MutexGuard<'_, Arc<Http2Logger>> = logger_rt.lock().unwrap();
          logger_lock(
            "error",
            &format!("Port {} already in use.", (*opts_rt).port),
          );
          process::exit(1);
        }
      };

      {
        let logger_lock: std::sync::MutexGuard<'_, Arc<Http2Logger>> = logger_rt.lock().unwrap();
        logger_lock("success", &format!("HTTP/2.0 on port {}", (*opts_rt).port));
      }

      loop {
        tokio::select! {
            _ = shutdown_rt.notified() => {
                break;
            }
            _ = self.acceptor(
                &listener,
                &tls_acceptor,
                Arc::clone(&logger_rt),
                Arc::clone(&handlers_rt),
                Arc::clone(&opts_rt)
            ) => {}
        }
      }
    });
  }

  pub fn stop(&self) {
    self.shutdown.notify_waiters();
  }
}
