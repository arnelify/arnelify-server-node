<img src="https://static.wikia.nocookie.net/arnelify/images/c/c8/Arnelify-logo-2024.png/revision/latest?cb=20240701012515" style="width:336px;" alt="Arnelify Logo" />

![Arnelify Server for NodeJS](https://img.shields.io/badge/Arnelify%20Server%20for%20NodeJS-1.0.6-yellow) ![NodeJS](https://img.shields.io/badge/NodeJS-22.13.1-green) ![Bun](https://img.shields.io/badge/Bun-1.2.0-blue)

## 🚀 About

**Arnelify® Server for NodeJS** — a multi-language server with HTTP 3.0 and WebTransport support.

All supported protocols:
| **#** | **Protocol** | **Transport** |
| - | - | - |
| 1 | TCP2 | WebTransport |
| 2 | TCP2 | HTTP 3.0 |
| 3 | TCP1 | WebSocket |
| 4 | TCP1 | HTTP 2.0 |
| 5 | TCP1 | HTTP 1.1 |

## 📋 Minimal Requirements
> Important: It's strongly recommended to use in a container that has been built from the gcc v15.2.0 image.
* CPU: Apple M1 / Intel Core i7 / AMD Ryzen 7
* OS: Debian 11 / MacOS 15 / Windows 10 with <a href="https://learn.microsoft.com/en-us/windows/wsl/install">WSL2</a>.
* RAM: 4 GB

## 📦 Installation
Install Neon v0.10.1
```
yarn global add neon-cli@0.10.1
```
Run in terminal:
```bash
yarn add arnelify-server
```
## 🎉 TCP2 / WebTransport

### 📚 Configuration

| **Option** | **Description** |
| - | - |
| **BLOCK_SIZE_KB**| The size of the allocated memory used for processing large packets. |
| **CERT_PEM**| Path to the TLS cert-file in PEM format. |
| **COMPRESSION**| If this option is enabled, the server will use BROTLI compression if the client application supports it. This setting increases CPU resource consumption. The server will not use compression if the data size exceeds the value of **BLOCK_SIZE_KB**. |
| **HANDSHAKE_TIMEOUT**| Maximum time in seconds to complete the TLS handshake. |
| **KEY_PEM**| Path to the TLS private key-file in PEM format. |
| **MAX_MESSAGE_SIZE_MB**| Maximum size of a single message the server will accept from a client. |
| **PING_TIMEOUT**| Maximum time the server will wait for a ping from the client. |
| **PORT**| Defines which port the server will listen on. |
| **SEND_TIMEOUT**| Maximum time for the client to receive a response from the server. |
| **THREAD_LIMIT**| Defines the maximum number of threads that will handle requests.|

### 📚 Examples

```typescript
import {
  WebTransportServer,
  WebTransportOpts,
  WebTransportBytes,
  WebTransportCtx,
  WebTransportStream
} from "arnelify-server";

(async function main() {

  const wt_opts: WebTransportOpts = {
    block_size_kb: 64,
    cert_pem: "certs/cert.pem",
    compression: false,
    handshake_timeout: 30,
    key_pem: "certs/key.pem",
    max_message_size_kb: 64,
    ping_timeout: 15,
    port: 4433,
    send_timeout: 30,
    thread_limit: 4
  };

  const wt: WebTransportServer = new WebTransportServer(wt_opts);
  wt.logger(async (level: string, message: string): Promise<void> => {
    console.log(`[Arnelify Server]: ${message}`);
  });

  wt.on("connect", async (
    ctx: WebTransportCtx,
    bytes: WebTransportBytes,
    stream: WebTransportStream
  ): Promise<void> => {
    await stream.push(ctx, bytes);
    await stream.close();
  });

  await wt.start();

})();
```

## 🎉 TCP2 / HTTP 3.0

### 📚 Configuration

| **Option** | **Description** |
| - | - |
| **ALLOW_EMPTY_FILES**| If this option is enabled, the server will not reject empty files. |
| **BLOCK_SIZE_KB**| The size of the allocated memory used for processing large packets. |
| **CERT_PEM**| Path to the TLS cert-file in PEM format. |
| **CHARSET**| Defines the encoding that the server will recommend to all client applications. |
| **COMPRESSION**| If this option is enabled, the server will use BROTLI compression if the client application supports it. This setting increases CPU resource consumption. The server will not use compression if the data size exceeds the value of **BLOCK_SIZE_KB**. |
| **KEEP_ALIVE**| defines how long the HTTP server keeps a connection. |
| **KEEP_EXTENSIONS**| If this option is enabled, file extensions will be preserved. |
| **KEY_PEM**| Path to the TLS private key-file in PEM format. |
| **MAX_FIELDS**| Defines the maximum number of fields in the received form. |
| **MAX_FIELDS_SIZE_TOTAL_MB**| Defines the maximum total size of all fields in the form. This option does not include file sizes. |
| **MAX_FILES**| Defines the maximum number of files in the form. |
| **MAX_FILES_SIZE_TOTAL_MB** | Defines the maximum total size of all files in the form. |
| **MAX_FILE_SIZE_MB**| Defines the maximum size of a single file in the form. |
| **PORT**| Defines which port the server will listen on. |
| **STORAGE_PATH**| Specifies the upload directory for storage. |
| **THREAD_LIMIT**| Defines the maximum number of threads that will handle requests. |

### 📚 Examples

```typescript
import { 
  Http3, 
  Http3Opts, 
  Http3Ctx, 
  Http3Stream
} from "arnelify-server";

(async function main() {

  const http3_opts: Http3Opts = {
    allow_empty_files: true,
    block_size_kb: 64,
    cert_pem: "certs/cert.pem",
    charset: "utf-8",
    compression: true,
    keep_alive: 30,
    keep_extensions: true,
    key_pem: "certs/key.pem",
    max_fields: 60,
    max_fields_size_total_mb: 1,
    max_files: 3,
    max_files_size_total_mb: 60,
    max_file_size_mb: 60,
    port: 4433,
    storage_path: "/var/www/node/storage",
    thread_limit: 4
  };

  const http3: Http3 = new Http3(http3_opts);
  http3.logger(async (_level: string, message: string): Promise<void> => {
    console.log(`[Arnelify Server]: ${message}`);
  });

  http3.on("/", async (ctx: Http3Ctx, stream: Http3Stream): Promise<void> => {
    const bytes: Buffer = Buffer.from(JSON.stringify(ctx));
    await stream.set_code(200);
    await stream.push_bytes(bytes, false);
    await stream.end();
  });

  await http3.start();

})();
```

## 🎉 TCP1 / WebSocket

### 📚 Configuration

| **Option** | **Description** |
| - | - |
| **BLOCK_SIZE_KB**| The size of the allocated memory used for processing large packets. |
| **COMPRESSION**| If this option is enabled, the server will use BROTLI compression if the client application supports it. This setting increases CPU resource consumption. The server will not use compression if the data size exceeds the value of **BLOCK_SIZE_KB**. |
| **MAX_MESSAGE_SIZE_MB**| Maximum size of a single message the server will accept from a client. |
| **PING_TIMEOUT**| Maximum time the server will wait for a ping from the client. |
| **PORT**| Defines which port the server will listen on. |
| **RATE_LIMIT**| Defines the maximum number of connections allowed from a single IP address. |
| **READ_TIMEOUT**| Maximum time allowed for a client to send a request to the server. |
| **SEND_TIMEOUT**| Maximum time allowed for the client to receive a response from the server. |
| **THREAD_LIMIT**| Defines the maximum number of threads that will handle requests.|

### 📚 Examples

```typescript
import {
  WebSocketServer,
  WebSocketOpts,
  WebSocketBytes,
  WebSocketCtx,
  WebSocketStream
} from "arnelify-server";

(async function main() {

  const ws_opts: WebSocketOpts = {
    block_size_kb: 64,
    compression: false,
    max_message_size_kb: 64,
    ping_timeout: 15,
    port: 4433,
    rate_limit: 5,
    read_timeout: 30,
    send_timeout: 30,
    thread_limit: 4
  };

  const ws: WebSocketServer = new WebSocketServer(ws_opts);
  ws.logger(async (_level: string, message: string): Promise<void> => {
    console.log(`[Arnelify Server]: ${message}`);
  });

  ws.on("connect", async (
    ctx: WebSocketCtx,
    bytes: WebSocketBytes,
    stream: WebSocketStream
  ): Promise<void> => {
    await stream.push(ctx, bytes);
    await stream.close();
  });

  await ws.start();

})();
```

## 🎉 TCP1 / HTTP 2.0

### 📚 Configuration

| **Option** | **Description** |
| - | - |
| **ALLOW_EMPTY_FILES**| If this option is enabled, the server will not reject empty files. |
| **BLOCK_SIZE_KB**| The size of the allocated memory used for processing large packets. |
| **CERT_PEM**| Path to the TLS cert-file in PEM format. |
| **CHARSET**| Defines the encoding that the server will recommend to all client applications. |
| **COMPRESSION**| If this option is enabled, the server will use BROTLI compression if the client application supports it. This setting increases CPU resource consumption. The server will not use compression if the data size exceeds the value of **BLOCK_SIZE_KB**. |
| **KEEP_ALIVE**| defines how long the HTTP server keeps a connection. |
| **KEEP_EXTENSIONS**| If this option is enabled, file extensions will be preserved. |
| **KEY_PEM**| Path to the TLS private key-file in PEM format. |
| **MAX_FIELDS**| Defines the maximum number of fields in the received form. |
| **MAX_FIELDS_SIZE_TOTAL_MB**| Defines the maximum total size of all fields in the form. This option does not include file sizes. |
| **MAX_FILES**| Defines the maximum number of files in the form. |
| **MAX_FILES_SIZE_TOTAL_MB** | Defines the maximum total size of all files in the form. |
| **MAX_FILE_SIZE_MB**| Defines the maximum size of a single file in the form. |
| **PORT**| Defines which port the server will listen on. |
| **STORAGE_PATH**| Specifies the upload directory for storage. |
| **THREAD_LIMIT**| Defines the maximum number of threads that will handle requests. |

### 📚 Examples

```typescript
import { 
  Http2, 
  Http2Opts, 
  Http2Ctx, 
  Http2Stream
} from "arnelify-server";

(async function main() {

  const http2_opts: Http2Opts = {
    allow_empty_files: true,
    block_size_kb: 64,
    cert_pem: "certs/cert.pem",
    charset: "utf-8",
    compression: true,
    keep_alive: 30,
    keep_extensions: true,
    key_pem: "certs/key.pem",
    max_fields: 60,
    max_fields_size_total_mb: 1,
    max_files: 3,
    max_files_size_total_mb: 60,
    max_file_size_mb: 60,
    port: 4433,
    storage_path: "/var/www/node/storage",
    thread_limit: 4
  };

  const http2: Http2 = new Http2(http2_opts);
  http2.logger(async (_level: string, message: string): Promise<void> => {
    console.log(`[Arnelify Server]: ${message}`);
  });

  http2.on("/", async (ctx: Http2Ctx, stream: Http2Stream): Promise<void> => {
    const bytes: Buffer = Buffer.from(JSON.stringify(ctx));
    await stream.set_code(200);
    await stream.push_bytes(bytes, false);
    await stream.end();
  });

  await http2.start();

})();
```

## 🎉 TCP1 / HTTP 1.1

### 📚 Configuration

| **Option** | **Description** |
| - | - |
| **ALLOW_EMPTY_FILES**| If this option is enabled, the server will not reject empty files. |
| **BLOCK_SIZE_KB**| The size of the allocated memory used for processing large packets. |
| **CHARSET**| Defines the encoding that the server will recommend to all client applications. |
| **COMPRESSION**| If this option is enabled, the server will use BROTLI compression if the client application supports it. This setting increases CPU resource consumption. The server will not use compression if the data size exceeds the value of **BLOCK_SIZE_KB**. |
| **KEEP_ALIVE**| defines how long the HTTP server keeps a connection. |
| **KEEP_EXTENSIONS**| If this option is enabled, file extensions will be preserved. |
| **MAX_FIELDS**| Defines the maximum number of fields in the received form. |
| **MAX_FIELDS_SIZE_TOTAL_MB**| Defines the maximum total size of all fields in the form. This option does not include file sizes. |
| **MAX_FILES**| Defines the maximum number of files in the form. |
| **MAX_FILES_SIZE_TOTAL_MB** | Defines the maximum total size of all files in the form. |
| **MAX_FILE_SIZE_MB**| Defines the maximum size of a single file in the form. |
| **PORT**| Defines which port the server will listen on. |
| **STORAGE_PATH**| Specifies the upload directory for storage. |
| **THREAD_LIMIT**| Defines the maximum number of threads that will handle requests. |

### 📚 Examples

```typescript
import {
  Http1,
  Http1Opts,
  Http1Ctx,
  Http1Stream
} from "arnelify-server";

(async function main() {

  const http1_opts: Http1Opts = {
    allow_empty_files: true,
    block_size_kb: 64,
    charset: "utf-8",
    compression: true,
    keep_alive: 30,
    keep_extensions: true,
    max_fields: 60,
    max_fields_size_total_mb: 1,
    max_files: 3,
    max_files_size_total_mb: 60,
    max_file_size_mb: 60,
    port: 4433,
    storage_path: "/var/www/node/storage",
    thread_limit: 4
  };

  const http1: Http1 = new Http1(http1_opts);
  http1.logger(async (_level: string, message: string): Promise<void> => {
    console.log(`[Arnelify Server]: ${message}`);
  });

  http1.on("/", async (ctx: Http1Ctx, stream: Http1Stream): Promise<void> => {
    const bytes: Buffer = Buffer.from(JSON.stringify(ctx));
    await stream.set_code(200);
    await stream.push_bytes(bytes, false);
    await stream.end();
  });

  await http1.start();

})();
```

## ⚖️ MIT License
This software is licensed under the <a href="https://github.com/arnelify/arnelify-server-node/blob/main/LICENSE">MIT License</a>. The original author's name, logo, and the original name of the software must be included in all copies or substantial portions of the software.

## 🛠️ Contributing
Join us to help improve this software, fix bugs or implement new functionality. Active participation will help keep the software up-to-date, reliable, and aligned with the needs of its users.

Run in terminal:
```bash
docker compose up -d --build
docker ps
docker exec -it <CONTAINER ID> bash
```
For TCP2 / WebTransport:
```bash
yarn test_wt
```
For TCP2 / HTTP 3.0:
```bash
yarn test_http3
```
For TCP1 / WebSocket:
```bash
yarn test_ws
```
For TCP1 / HTTP 2.0:
```bash
yarn test_http2
```
For TCP1 / HTTP 1.1:
```bash
yarn test_http1
```

## ⭐ Release Notes
Version 1.0.6 — a multi-language server with HTTP 3.0 and WebTransport support.

We are excited to introduce the Arnelify Server for NodeJS! Please note that this version is raw and still in active development.

Change log:

* Compatible with Bun and V8.
* HTTP 3.0 + WebTransport.
* Security-aware logging with attack detection.
* Async Runtime & Multi-Threading.
* Large file upload and download support.
* BROTLI compression (still in development).
* FFI, PYO3 and NEON support.
* Significant refactoring and optimizations.

Please use this version with caution, as it may contain bugs and unfinished features. We are actively working on improving and expanding the server's capabilities, and we welcome your feedback and suggestions.

## 🔗 Links

* <a href="https://github.com/arnelify/arnelify-pod-cpp">Arnelify POD for C++</a>
* <a href="https://github.com/arnelify/arnelify-pod-node">Arnelify POD for NodeJS</a>
* <a href="https://github.com/arnelify/arnelify-pod-python">Arnelify POD for Python</a>
* <a href="https://github.com/arnelify/arnelify-pod-rust">Arnelify POD for Rust</a>
* <a href="https://github.com/arnelify/arnelify-react-native">Arnelify React Native</a>