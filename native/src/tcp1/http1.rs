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
  borrow,
  collections::HashMap,
  fs::{File, Metadata, OpenOptions},
  io::{Error, ErrorKind, Read, Write},
  path::Path,
  process,
  sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard},
  time::{Duration, SystemTime, UNIX_EPOCH},
  vec,
};

use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::TcpListener,
  runtime::{Builder, Runtime},
  sync::{Notify, mpsc},
  time::timeout,
};

pub type JSON = serde_json::Value;
pub type Http1Ctx = serde_json::Value;

enum StreamEvent {
  BodyChunk { chunk: Vec<u8>, flush: bool },
}

#[derive(Clone, Default)]
pub struct Http1Opts {
  pub allow_empty_files: bool,
  pub block_size_kb: usize,
  pub charset: String,
  pub compression: bool,
  pub keep_alive: u8,
  pub keep_extensions: bool,
  pub max_fields: u32,
  pub max_fields_size_total_mb: usize,
  pub max_files: u32,
  pub max_files_size_total_mb: usize,
  pub max_file_size_mb: usize,
  pub port: u16,
  pub storage_path: String,
  pub thread_limit: u64,
}

struct Http1Req {
  opts: Http1Opts,

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
  ctx: Http1Ctx,
}

impl Http1Req {
  pub fn new(opts: Http1Opts) -> Self {
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
          "addr": JSON::Null,
          "cookie": {},
          "headers": {},
          "method": "GET",
          "path": "/",
          "protocol": "HTTP/1.1",
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

  pub fn get_ctx(&self) -> Http1Ctx {
    self.ctx.clone()
  }

  pub fn get_path(&self) -> String {
    self.path.clone()
  }

  pub fn is_empty(&self) -> bool {
    self.buff.is_empty()
  }

  pub fn set_addr(&mut self, addr: &str) -> () {
    self.ctx["_state"]["addr"] = JSON::String(String::from(addr));
  }

  fn set_body(&mut self) -> Result<u8, Error> {
    if self.body.trim().is_empty() {
      return Ok(1);
    }

    let mut current: &mut JSON = &mut self.ctx["params"]["body"];
    for key in &self.keys {
      current = &mut current[&key];
    }

    let arr: &mut Vec<JSON> = match current.as_array_mut() {
      Some(v) => v,
      None => {
        return Err(Error::new(ErrorKind::InvalidInput, "Invalid field name."));
      }
    };

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

    let arr: &mut Vec<JSON> = match current.as_array_mut() {
      Some(v) => v,
      None => {
        return Err(Error::new(ErrorKind::InvalidInput, "Invalid field name."));
      }
    };

    arr.push(file);

    Ok(1)
  }

  fn set_header(&mut self, key: String, value: String) -> Result<u8, Error> {
    self.ctx["_state"]["headers"][&key] = JSON::String(value.to_string());

    if key.eq_ignore_ascii_case("Transfer-Encoding") {
      if value.to_ascii_lowercase().contains("chunked") {
        return Err(Error::new(
          ErrorKind::InvalidData,
          "Chunked encoding is not supported.",
        ));
      }
    }

    if key.eq_ignore_ascii_case("Cookie") {
      return self.set_cookie(&value);
    }

    if key.eq_ignore_ascii_case("Content-Type") {
      return self.set_content_type(&value);
    }

    if key.eq_ignore_ascii_case("Content-Length") {
      self.content_length = match value.parse::<usize>() {
        Ok(v) => v,
        Err(_) => {
          return Err(Error::new(
            ErrorKind::InvalidData,
            "Content-Length must be a number.",
          ));
        }
      };

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

    let arr: &mut Vec<JSON> = match current.as_array_mut() {
      Some(v) => v,
      None => {
        return Err(Error::new(ErrorKind::InvalidInput, "Invalid query."));
      }
    };

    arr.push(JSON::String(String::from(value)));

    Ok(1)
  }

  fn read_meta(&mut self, meta: &[u8]) -> Result<u8, Error> {
    let mut start: usize = 0;
    while let Some(pos) = meta[start..].windows(2).position(|w| w == b"\r\n") {
      let me: usize = start + pos;
      let header: &[u8] = &meta[start..me];

      if header.starts_with(b"Content-Disposition") {
        let name_start: usize = match header.windows(6).position(|w| w == b"name=\"") {
          Some(pos) => pos,
          None => {
            return Err(Error::new(
              ErrorKind::InvalidInput,
              "Invalid Content-Disposition detected in multipart/form-data.",
            ));
          }
        };

        let name_end: usize = match header[name_start + 6..].iter().position(|&b| b == b'"') {
          Some(pos) => pos + name_start + 6,
          None => {
            return Err(Error::new(
              ErrorKind::InvalidInput,
              "Invalid Content-Disposition detected in multipart/form-data.",
            ));
          }
        };

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
          let filename_end: usize = match header[fs + 10..].iter().position(|&b| b == b'"') {
            Some(pos) => pos + fs + 10,
            None => {
              return Err(Error::new(
                ErrorKind::InvalidInput,
                "Invalid filename detected in multipart/form-data.",
              ));
            }
          };

          if filename_end - fs + 10 > 255 {
            return Err(Error::new(
              ErrorKind::InvalidInput,
              "The maximum filename length has been exceeded.",
            ));
          }

          let filename: &str = match str::from_utf8(&header[fs + 10..filename_end]) {
            Ok(s) => s,
            Err(_) => {
              return Err(Error::new(ErrorKind::InvalidInput, "Invalid UTF-8."));
            }
          };

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
        let mime_start: usize = match header.windows(2).position(|w| w == b": ") {
          Some(pos) => pos,
          None => {
            return Err(Error::new(
              ErrorKind::InvalidInput,
              "Invalid Content-Type detected in multipart/form-data.",
            ));
          }
        };

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
            "The maximum size of the body has been exceeded.",
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

      if !json.is_object() {
        self.buff.drain(..self.content_length);
        self.skip = self.content_length - self.size;
        return Err(Error::new(
          ErrorKind::InvalidInput,
          "Invalid application/json.",
        ));
      }

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

    if path.contains("..") || path.contains("\0") {
      self.path = String::from("_");
      self.ctx["_state"]["path"] = JSON::String(String::from("_"));
    } else {
      self.path = String::from(path);
      self.ctx["_state"]["path"] = JSON::String(String::from(path));
    }

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
        "addr": JSON::Null,
        "cookie": {},
        "headers": {},
        "method": "GET",
        "path": "/",
        "protocol": "HTTP/1.1",
        "topic": "_"
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

pub struct Http1Stream {
  opts: Http1Opts,
  body: Option<Vec<u8>>,
  cb_logger: Arc<Http1Logger>,
  cb_send: Arc<dyn Fn(Vec<u8>, bool) + Send + Sync>,
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

impl Http1Stream {
  pub fn new(opts: Http1Opts) -> Self {
    Self {
      opts,
      body: None,
      cb_logger: Arc::new(|_level: &str, message: &str| {
        println!("[Arnelify Server]: {}", message);
      }),
      cb_send: Arc::new(|_chunk: Vec<u8>, _flush: bool| {}),
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
    (self.cb_send)(Vec::new(), true);
    self.reset();
  }

  fn get_headers(&self) -> Vec<u8> {
    let mut buff: Vec<u8> = Vec::new();
    let status: &str = match self.code {
      100 => "Continue",
      101 => "Switching Protocols",
      102 => "Processing",
      200 => "OK",
      201 => "Created",
      202 => "Accepted",
      204 => "No Content",
      206 => "Partial Content",
      301 => "Moved Permanently",
      302 => "Found",
      304 => "Not Modified",
      400 => "Bad Request",
      401 => "Unauthorized",
      403 => "Forbidden",
      404 => "Not Found",
      409 => "Conflict",
      500 => "Internal Server Error",
      502 => "Bad Gateway",
      503 => "Service Unavailable",
      _ => {
        (self.cb_logger)("error", &format!("Unknown HTTP status code: {}", self.code));
        process::exit(1);
      }
    };

    buff.extend_from_slice(format!("HTTP/1.1 {} {}\r\n", self.code, status).as_bytes());
    buff.extend_from_slice(format!("Connection: {}\r\n", "keep-alive").as_bytes());
    for (key, value) in &self.headers {
      buff.extend_from_slice(format!("{}: {}\r\n", key, value).as_bytes());
    }

    buff.extend_from_slice(format!("Server: {}\r\n", "Arnelify Server").as_bytes());
    buff.extend_from_slice(b"\r\n");
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

  pub fn on_send(&mut self, cb: Arc<dyn Fn(Vec<u8>, bool) + Send + Sync>) -> () {
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
        let meta: Metadata = match std::fs::metadata(&path) {
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
        (self.cb_send)(self.get_headers(), false);
        self.headers_sent = true;
      }

      if self.opts.compression && self.compression.is_some() {
        if let Some(_compression) = &self.compression {
          //TODO: Brotli
        }
      }

      let block_size: usize = self.opts.block_size_kb * 1024;

      loop {
        let mut buff: Vec<u8> = vec![0u8; block_size];
        let bytes_read: usize = match file.read(&mut buff) {
          Ok(0) => break,
          Ok(n) => n,
          Err(_) => {
            (self.cb_logger)("warning", &format!("File not found."));
            process::exit(1);
          }
        };

        buff.truncate(bytes_read);
        (self.cb_send)(buff, false);
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
        (self.cb_send)(self.get_headers(), false);
        self.headers_sent = true;
      }

      if self.opts.compression && self.compression.is_some() {
        if let Some(_compression) = &self.compression {
          //TODO: Brotli
        }
      }

      if let Some(b) = self.body.take() {
        (self.cb_send)(b, false);
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

pub type Http1Handler = dyn Fn(Arc<Mutex<Http1Ctx>>, Arc<Mutex<Http1Stream>>) + Send + Sync;
pub type Http1Logger = dyn Fn(&str, &str) + Send + Sync;

async fn disconnect(
  handler: &Option<Arc<Http1Handler>>,
  ctx: &Option<Arc<Mutex<Http1Ctx>>>,
  opts: &Http1Opts,
) {
  if let (Some(handler), Some(ctx)) = (handler, ctx) {
    let ctx_close: Arc<Mutex<Http1Ctx>> = Arc::clone(ctx);
    let stream_close: Arc<Mutex<Http1Stream>> =
      Arc::new(Mutex::new(Http1Stream::new(opts.clone())));
    handler(ctx_close, stream_close);
  }
}

async fn acceptor(
  listener: &TcpListener,
  logger: Arc<RwLock<Arc<Http1Logger>>>,
  handlers: Arc<RwLock<HashMap<String, Arc<Http1Handler>>>>,
  opts: Arc<Http1Opts>,
) -> () {
  let keep_alive: Duration = Duration::from_secs((*opts).keep_alive as u64);

  match listener.accept().await {
    Ok((socket, addr)) => {
      let (mut reader, mut writer) = socket.into_split();
      {
        let logger_lock: RwLockReadGuard<'_, Arc<Http1Logger>> = logger.read().unwrap();
        (logger_lock)("info", &format!("Client {}: Connected", addr));
      }

      let logger_conn: Arc<RwLock<Arc<Http1Logger>>> = Arc::clone(&logger);
      let handlers_conn: Arc<RwLock<HashMap<String, Arc<Http1Handler>>>> = Arc::clone(&handlers);
      let opts_conn: Arc<Http1Opts> = Arc::clone(&opts);

      // CONNECTION TASK
      tokio::spawn(async move {
        let block_size: usize = (*opts_conn).block_size_kb * 1024;
        let mut buff: Vec<u8> = vec![0u8; block_size];
        let mut req: Http1Req = Http1Req::new((*opts_conn).clone());
        req.set_addr(&addr.to_string());

        let mut no_bytes: bool = true;
        let mut last_ctx: Option<Arc<Mutex<Http1Ctx>>> = None;
        let (on_connect, on_disconnect) = {
          let handlers_lock = handlers_conn.read().unwrap();
          (
            handlers_lock.get("_connect").cloned(),
            handlers_lock.get("_disconnect").cloned(),
          )
        };

        loop {
          // READ TASK
          let bytes_read: usize = match timeout(keep_alive, reader.read(&mut buff)).await {
            Ok(Ok(0)) => {
              disconnect(&on_disconnect, &last_ctx, &opts_conn).await;
              let logger_lock: RwLockReadGuard<'_, Arc<Http1Logger>> = logger_conn.read().unwrap();
              logger_lock("info", &format!("Client {}: Disconnected", addr));
              return;
            }
            Ok(Ok(n)) => n,
            Ok(Err(e)) => {
              disconnect(&on_disconnect, &last_ctx, &opts_conn).await;
              let logger_lock: RwLockReadGuard<'_, Arc<Http1Logger>> = logger_conn.read().unwrap();
              logger_lock(
                "warning",
                &format!("Client {}: Socket read error: {}", addr, e),
              );
              return;
            }
            Err(_) => {
              disconnect(&on_disconnect, &last_ctx, &opts_conn).await;
              let logger_lock: RwLockReadGuard<'_, Arc<Http1Logger>> = logger_conn.read().unwrap();
              if no_bytes {
                logger_lock("info", &format!("Client {}: Keep-alive timeout", addr));
                return;
              }

              logger_lock("warning", &format!("Client {}: Read timeout", addr));
              return;
            }
          };

          req.add(&buff[..bytes_read]);
          no_bytes = false;

          loop {
            match req.read_block() {
              Ok(Some(_)) => {
                let path: String = req.get_path();
                let ctx: Arc<Mutex<Http1Ctx>> = Arc::new(Mutex::new(req.get_ctx()));
                let compression: Option<String> = req.get_compression();
                let payload: String = serde_json::to_string(&req.get_ctx()).unwrap();
                req.reset();

                let (tx, mut rx) = mpsc::channel::<StreamEvent>(32);
                no_bytes = true;
                {
                  let logger_lock: RwLockReadGuard<'_, Arc<Http1Logger>> =
                    logger_conn.read().unwrap();
                  logger_lock(
                    "info",
                    &format!("Client {}: Sent payload: {}", addr, payload),
                  );
                }

                let stream: Arc<Mutex<Http1Stream>> =
                  Arc::new(Mutex::new(Http1Stream::new((*opts_conn).clone())));
                {
                  let mut stream_lock = stream.lock().unwrap();
                  stream_lock.on_send(Arc::new({
                    let tx_safe: mpsc::Sender<StreamEvent> = tx.clone();
                    move |chunk: Vec<u8>, flush: bool| {
                      let _ = tx_safe.try_send(StreamEvent::BodyChunk { chunk, flush });
                    }
                  }));
                  stream_lock.set_encoding((*opts_conn).charset.clone());
                  stream_lock.set_compression(compression);
                }

                // ON CONNECT
                match last_ctx {
                  Some(_) => {}
                  None => {
                    let ctx_conn: Arc<Mutex<Http1Ctx>> = Arc::clone(&ctx);
                    let stream_conn: Arc<Mutex<Http1Stream>> = Arc::clone(&stream);
                    last_ctx = Some(Arc::clone(&ctx));
                    if let Some(handler) = &on_connect {
                      handler(ctx_conn, stream_conn);
                    }
                  }
                }

                let handler_opt: Option<Arc<Http1Handler>> = {
                  let handlers_lock: RwLockReadGuard<'_, HashMap<String, Arc<Http1Handler>>> =
                    handlers_conn.read().unwrap();
                  handlers_lock
                    .get(&path)
                    .or_else(|| handlers_lock.get("_"))
                    .cloned()
                };

                let ctx_handler: Arc<Mutex<Http1Ctx>> = Arc::clone(&ctx);
                let stream_handler: Arc<Mutex<Http1Stream>> = Arc::clone(&stream);
                if let Some(handler) = handler_opt {
                  handler(ctx_handler, stream_handler);
                }

                drop(tx);

                loop {
                  // WRITE TASK
                  match timeout(keep_alive, rx.recv()).await {
                    Ok(Some(StreamEvent::BodyChunk { chunk, flush })) => {
                      if !chunk.is_empty() {
                        if let Err(e) = writer.write_all(&chunk).await {
                          disconnect(&on_disconnect, &last_ctx, &opts_conn).await;
                          let logger_lock: RwLockReadGuard<'_, Arc<Http1Logger>> =
                            logger_conn.read().unwrap();
                          logger_lock("warning", &format!("Client {}: Write error: {}", addr, e));
                          return;
                        }
                      }

                      if flush {
                        if let Err(e) = writer.flush().await {
                          disconnect(&on_disconnect, &last_ctx, &opts_conn).await;
                          let logger_lock: RwLockReadGuard<'_, Arc<Http1Logger>> =
                            logger_conn.read().unwrap();
                          logger_lock("warning", &format!("Client {}: Flush error: {}", addr, e));
                          return;
                        }

                        let logger_lock: RwLockReadGuard<'_, Arc<Http1Logger>> =
                          logger_conn.read().unwrap();
                        logger_lock("info", &format!("Client {}: Write task finished", addr));
                        break;
                      }
                    }
                    Ok(None) => break,
                    Err(_) => {
                      disconnect(&on_disconnect, &last_ctx, &opts_conn).await;
                      let logger_lock: RwLockReadGuard<'_, Arc<Http1Logger>> =
                        logger_conn.read().unwrap();
                      logger_lock("warning", &format!("Client {}: Write timeout", addr));
                      return;
                    }
                  }
                }
              }
              Ok(None) => break,
              Err(e) => match e.kind() {
                ErrorKind::InvalidData => {
                  disconnect(&on_disconnect, &last_ctx, &opts_conn).await;
                  let logger_lock: RwLockReadGuard<'_, Arc<Http1Logger>> =
                    logger_conn.read().unwrap();
                  logger_lock(
                    "warning",
                    &format!("Client {}: Block read error: {}", addr, e),
                  );

                  return;
                }
                _ => {
                  let (tx, mut rx) = mpsc::channel::<StreamEvent>(32);
                  let stream: Arc<Mutex<Http1Stream>> =
                    Arc::new(Mutex::new(Http1Stream::new((*opts_conn).clone())));
                  {
                    let mut stream_lock = stream.lock().unwrap();
                    stream_lock.on_send(Arc::new({
                      let tx_safe: mpsc::Sender<StreamEvent> = tx.clone();
                      move |chunk: Vec<u8>, flush: bool| {
                        let _ = tx_safe.try_send(StreamEvent::BodyChunk { chunk, flush });
                      }
                    }));

                    stream_lock.set_code(409);
                    stream_lock.push_json(
                      &serde_json::json!({
                          "code": 409,
                          "error": format!("{}", e)
                      }),
                      false,
                    );
                    stream_lock.end();
                  }

                  drop(tx);

                  // ERROR WRITE TASK
                  while let Ok(Some(StreamEvent::BodyChunk { chunk, flush })) =
                    timeout(keep_alive, rx.recv()).await
                  {
                    if !chunk.is_empty() {
                      if let Err(e) = writer.write_all(&chunk).await {
                        disconnect(&on_disconnect, &last_ctx, &opts_conn).await;
                        let logger_lock: RwLockReadGuard<'_, Arc<Http1Logger>> =
                          logger_conn.read().unwrap();
                        logger_lock("warning", &format!("Client {}: Write error: {}", addr, e));
                        return;
                      }
                    }

                    if flush {
                      if let Err(e) = writer.flush().await {
                        disconnect(&on_disconnect, &last_ctx, &opts_conn).await;
                        let logger_lock: RwLockReadGuard<'_, Arc<Http1Logger>> =
                          logger_conn.read().unwrap();
                        logger_lock("warning", &format!("Client {}: Flush error: {}", addr, e));
                        return;
                      }

                      let logger_lock: RwLockReadGuard<'_, Arc<Http1Logger>> =
                        logger_conn.read().unwrap();
                      logger_lock("info", &format!("Client {}: Write task finished", addr));
                      break;
                    }
                  }
                }
              },
            }

            if req.is_empty() {
              break;
            }
          }
        }
      });
    }

    Err(e) => {
      let logger_lock: RwLockReadGuard<'_, Arc<Http1Logger>> = logger.read().unwrap();
      logger_lock("warning", &format!("Acceptor error: {}", e));
    }
  }
}

pub struct Http1 {
  cb_logger: Arc<RwLock<Arc<Http1Logger>>>,
  handlers: Arc<RwLock<HashMap<String, Arc<Http1Handler>>>>,
  opts: Http1Opts,
  shutdown: Arc<Notify>,
}

impl Http1 {
  pub fn new(opts: Http1Opts) -> Self {
    let handlers: Arc<RwLock<HashMap<String, Arc<Http1Handler>>>> =
      Arc::new(RwLock::new(HashMap::new()));
    {
      let mut handlers_write: RwLockWriteGuard<'_, HashMap<String, Arc<Http1Handler>>> =
        handlers.write().unwrap();
      handlers_write.insert(
        String::from("_"),
        Arc::new(
          move |_ctx: Arc<Mutex<Http1Ctx>>, stream: Arc<Mutex<Http1Stream>>| {
            let mut stream_lock: std::sync::MutexGuard<'_, Http1Stream> = stream.lock().unwrap();
            stream_lock.set_code(404);
            stream_lock.push_json(
              &serde_json::json!({
                "code": 404,
                "error": "Not found."
              }),
              false,
            );
            stream_lock.end();
          },
        ),
      );

      handlers_write.insert(
        String::from("_connect"),
        Arc::new(move |_ctx: Arc<Mutex<Http1Ctx>>, _stream: Arc<Mutex<Http1Stream>>| {}),
      );

      handlers_write.insert(
        String::from("_disconnect"),
        Arc::new(move |_ctx: Arc<Mutex<Http1Ctx>>, _stream: Arc<Mutex<Http1Stream>>| {}),
      );
    }

    Self {
      opts,
      cb_logger: Arc::new(RwLock::new(Arc::new(move |_level: &str, message: &str| {
        println!("[Arnelify Server]: {}", message);
      }))),
      handlers,
      shutdown: Arc::new(Notify::new()),
    }
  }

  pub fn logger(&self, cb: Arc<Http1Logger>) {
    let mut logger_write: RwLockWriteGuard<'_, Arc<Http1Logger>> = self.cb_logger.write().unwrap();
    *logger_write = cb;
  }

  pub fn on(&self, path: &str, cb: Arc<Http1Handler>) {
    let mut handlers_write: RwLockWriteGuard<'_, HashMap<String, Arc<Http1Handler>>> =
      self.handlers.write().unwrap();
    handlers_write.insert(String::from(path), cb);
  }

  pub fn start(&self) {
    let logger_rt: Arc<RwLock<Arc<Http1Logger>>> = Arc::clone(&self.cb_logger);
    let handlers_rt: Arc<RwLock<HashMap<String, Arc<Http1Handler>>>> = Arc::clone(&self.handlers);
    let opts_rt: Arc<Http1Opts> = Arc::new(self.opts.clone());
    let shutdown_rt: Arc<Notify> = Arc::clone(&self.shutdown);

    let rt: Runtime = Builder::new_multi_thread()
      .worker_threads(self.opts.thread_limit as usize)
      .enable_all()
      .build()
      .unwrap();

    rt.block_on(async move {
      let addr: (&str, u16) = ("0.0.0.0", (*opts_rt).port);
      let listener: TcpListener = TcpListener::bind(&addr).await.unwrap_or_else(|_: Error| {
        let logger_lock: RwLockReadGuard<'_, Arc<Http1Logger>> = logger_rt.read().unwrap();
        logger_lock(
          "error",
          &format!("Port {} already in use.", (*opts_rt).port),
        );
        process::exit(1);
      });

      {
        let logger_lock: RwLockReadGuard<'_, Arc<Http1Logger>> = logger_rt.read().unwrap();
        logger_lock("success", &format!("HTTP/1.1 on port {}", (*opts_rt).port));
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
