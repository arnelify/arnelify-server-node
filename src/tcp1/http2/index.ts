// MIT LICENSE

// COPYRIGHT (R) 2025 ARNELIFY. AUTHOR: TARON SARKISYAN

// PERMISSION IS HEREBY GRANTED, FREE OF CHARGE, TO ANY PERSON OBTAINING A COPY
// OF THIS SOFTWARE AND ASSOCIATED DOCUMENTATION FILES (THE "SOFTWARE"), TO DEAL
// IN THE SOFTWARE WITHOUT RESTRICTION, INCLUDING WITHOUT LIMITATION THE RIGHTS
// TO USE, COPY, MODIFY, MERGE, PUBLISH, DISTRIBUTE, SUBLICENSE, AND/OR SELL
// COPIES OF THE SOFTWARE, AND TO PERMIT PERSONS TO WHOM THE SOFTWARE IS
// FURNISHED TO DO SO, SUBJECT TO THE FOLLOWING CONDITIONS:

// THE ABOVE COPYRIGHT NOTICE AND THIS PERMISSION NOTICE SHALL BE INCLUDED IN ALL
// COPIES OR SUBSTANTIAL PORTIONS OF THE SOFTWARE.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

const native = require("../../../native");

import { 
  UnixDomainSocket, UnixDomainSocketBytes, 
  UnixDomainSocketCtx, UnixDomainSocketOpts 
} from "../../ipc/uds";

type Http2Opts = {
  allow_empty_files: boolean;
  block_size_kb: number;
  cert_pem: string;
  charset: string;
  compression: boolean;
  keep_alive: number;
  keep_extensions: boolean;
  key_pem: string;
  max_fields: number;
  max_fields_size_total_mb: number;
  max_files: number;
  max_files_size_total_mb: number;
  max_file_size_mb: number;
  port: number;
  storage_path: string;
  thread_limit: number;
};

type Http2Ctx = [];

class Http2Stream {
  id: number = 0;
  topic: string = "";

  cb_send: (topic: string, args: any[], bytes: Buffer) => void =
    (topic: string, args: any[], bytes: Buffer): void => {
      console.log(bytes);
    };

  constructor(id: number) {
    this.id = id;
  }

  add_header(key: string, value: string): void {
    const args: any[] = [
      this.id,
      key,
      value
    ];

    this.cb_send("http2_add_header", args, Buffer.alloc(0));
  }

  end(): void {
    const args: any[] = [
      this.id
    ];

    this.cb_send("http2_end", args, Buffer.alloc(0));
  }

  on_send(cb: (topic: string, args: any[], bytes: Buffer) => void): void {
    this.cb_send = cb;
  }

  push_bytes(bytes: Buffer, is_attachment: boolean = false): void {
    const args: any[] = [
      this.id,
      is_attachment ? 1 : 0
    ];

    this.cb_send("http2_push_bytes", args, bytes);
  }

  push_file(file_path: string, is_attachment: boolean): void {
    const args: any[] = [
      this.id,
      file_path,
      is_attachment ? 1 : 0
    ];

    this.cb_send("http2_push_file", args, Buffer.alloc(0));
  }

  push_json(json: any, is_attachment: boolean = false): void {
    const args: any[] = [
      this.id,
      json,
      is_attachment ? 1 : 0
    ];

    this.cb_send("http2_push_json", args, Buffer.alloc(0));
  }

  set_code(code: number): void {
    const args: any[] = [
      this.id,
      code
    ];

    this.cb_send("http2_set_code", args, Buffer.alloc(0));
  }

  set_compression(compression: null | string): void {
    const args: any[] = [
      this.id,
      compression ? compression : ""
    ];

    this.cb_send("http2_set_compression", args, Buffer.alloc(0));
  }

  set_headers(headers: Record<string, string>[]): void {
    const args: any[] = [
      this.id,
      headers
    ];

    this.cb_send("http2_set_headers", args, Buffer.alloc(0));
  }
}

type Http2Handler = (ctx: Http2Ctx, stream: Http2Stream) => void;

class Http2 {
  id: number = 0;
  opts: Http2Opts;
  handlers: { [key: string]: Http2Handler } = {};
  socket_path: string = '/var/run/arnelify_server.sock';
  uds: UnixDomainSocket;

  constructor(opts: Http2Opts) {
    this.opts = opts;

    const uds_opts: UnixDomainSocketOpts = {
      block_size_kb: opts.block_size_kb,
      keep_alive: opts.keep_alive,
      socket_path: this.socket_path,
      thread_limit: opts.thread_limit
    };

    this.uds = new UnixDomainSocket(uds_opts);
    this.id = native.http2_create(JSON.stringify({
      socket_path: this.socket_path,
      ...this.opts,
    }));
  }

  logger(cb: any): void {
    this.uds.on('http2_logger', (ctx: UnixDomainSocketCtx, _bytes: UnixDomainSocketBytes): void => {
      const [level, message] = ctx;
      cb(level, message);
    });
  }

  on(path: string, cb: Http2Handler): void {
    this.handlers[path] = cb;

    this.uds.on('http2_on', (ctx: UnixDomainSocketCtx, _bytes: UnixDomainSocketBytes): void => {
      const [stream_id, handler_path, handler_ctx] = ctx;

      const stream = new Http2Stream(stream_id);
      stream.on_send((topic, args, buffer): void => {
        this.uds.push(topic, args, buffer);
      });

      const cb = this.handlers[handler_path];
      cb(handler_ctx, stream);
    });

    native.http2_on(this.id, path);
  }

  async start(): Promise<void> {
    native.http2_start(this.id);
    this.uds.start();
  }

  async stop(): Promise<void> {
    native.http2_stop(this.id);
    this.uds.stop();
  }
}

export type { Http2Opts, Http2Ctx };
export { Http2, Http2Stream };