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

type Http3Opts = {
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

type Http3Ctx = Record<string, any>;

class Http3Stream {
  id: number = 0;
  topic: string = "";

  cb_send: (topic: string, args: any[], bytes: Buffer) => Promise<void> =
    async (_topic: string, _args: any[], bytes: Buffer): Promise<void> => {
      console.log(bytes);
    };

  constructor(id: number) {
    this.id = id;
  }

  async add_header(key: string, value: string): Promise<void> {
    const args: any[] = [this.id, key, value];
    await this.cb_send("http3_add_header", args, Buffer.alloc(0));
  }

  async end(): Promise<void> {
    const args: any[] = [this.id];
    await this.cb_send("http3_end", args, Buffer.alloc(0));
  }

  on_send(cb: (topic: string, args: any[], bytes: Buffer) => Promise<void>): void {
    this.cb_send = cb;
  }

  async push_bytes(bytes: Buffer, is_attachment: boolean = false): Promise<void> {
    const args: any[] = [this.id, is_attachment ? 1 : 0];
    await this.cb_send("http3_push_bytes", args, bytes);
  }

  async push_file(file_path: string, is_attachment: boolean): Promise<void> {
    const args: any[] = [this.id, file_path, is_attachment ? 1 : 0];
    await this.cb_send("http3_push_file", args, Buffer.alloc(0));
  }

  async push_json(json: any, is_attachment: boolean = false): Promise<void> {
    const args: any[] = [this.id, json, is_attachment ? 1 : 0];
    await this.cb_send("http3_push_json", args, Buffer.alloc(0));
  }

  async set_code(code: number): Promise<void> {
    const args: any[] = [this.id, code];
    await this.cb_send("http3_set_code", args, Buffer.alloc(0));
  }

  async set_compression(compression: null | string): Promise<void> {
    const args: any[] = [this.id, compression ? compression : ""];
    await this.cb_send("http3_set_compression", args, Buffer.alloc(0));
  }

  async set_headers(headers: Record<string, string>[]): Promise<void> {
    const args: any[] = [this.id, headers];
    await this.cb_send("http3_set_headers", args, Buffer.alloc(0));
  }
}

type Http3Handler = (ctx: Http3Ctx, stream: Http3Stream) => Promise<void>;

class Http3 {
  id: number = 0;
  opts: Http3Opts;
  handlers: { [key: string]: Http3Handler } = {};
  socket_path: string = '/var/run/arnelify_server.sock';
  uds: UnixDomainSocket;

  constructor(opts: Http3Opts) {
    this.opts = opts;

    const uds_opts: UnixDomainSocketOpts = {
      block_size_kb: opts.block_size_kb,
      socket_path: this.socket_path,
      thread_limit: opts.thread_limit
    };

    this.uds = new UnixDomainSocket(uds_opts);
    this.id = native.http3_create(JSON.stringify({
      socket_path: this.socket_path,
      ...this.opts,
    }));
  }

  logger(cb: any): void {
    this.uds.on('http3_logger', async (ctx: UnixDomainSocketCtx, _bytes: UnixDomainSocketBytes): Promise<void> => {
      const [level, message] = ctx;
      await cb(level, message);
    });

    native.http3_logger(this.id);
  }

  on(path: string, cb: Http3Handler): void {
    this.handlers[path] = cb;

    this.uds.on('http3_on', async (ctx: UnixDomainSocketCtx, _bytes: UnixDomainSocketBytes): Promise<void> => {
      const [stream_id, handler_path, handler_ctx] = ctx;

      const stream = new Http3Stream(stream_id);
      stream.on_send(async (topic, args, buffer): Promise<void> => {
        await this.uds.push(topic, args, buffer);
      });

      const handler = this.handlers[handler_path];
      if (handler) await handler(handler_ctx, stream);
    });

    native.http3_on(this.id, path);
  }

  async start(): Promise<void> {
    native.http3_start_ipc(this.id);
    await this.uds.start();
    native.http3_start(this.id);
  }

  async stop(): Promise<void> {
    native.http3_stop(this.id);
    await this.uds.stop();
  }
}

export type { Http3Opts, Http3Ctx };
export { Http3, Http3Stream };