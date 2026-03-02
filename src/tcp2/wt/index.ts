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

type WebTransportOpts = {
  block_size_kb: number;
  cert_pem: string;
  compression: true;
  handshake_timeout: number;
  key_pem: string;
  max_message_size_kb: number;
  ping_timeout: number;
  port: number;
  send_timeout: number;
  thread_limit: number;
};

type WebTransportCtx = [];
type WebTransportBytes = Buffer;

class WebTransportStream {
  id: number = 0;
  topic: string = "";

  cb_send: (topic: string, args: any[], bytes: Buffer) => void =
    (_topic: string, _args: any[], bytes: Buffer): void => {
      console.log(bytes);
    };

  constructor(id: number) {
    this.id = id;
  }

  close(): void {
    const args: any[] = [
      this.id
    ];

    this.cb_send("wt_close", args, Buffer.alloc(0));
  }

  on_send(cb: (topic: string, args: any[], bytes: Buffer) => void): void {
    this.cb_send = cb;
  }

  push(json: any, bytes: Buffer): void {
    const args: any[] = [
      this.id,
      json
    ];

    this.cb_send("wt_push", args, bytes);
  }

  push_bytes(bytes: Buffer): void {
    const args: any[] = [
      this.id
    ];

    this.cb_send("wt_push_bytes", args, bytes);
  }

  push_json(json: any): void {
    const args: any[] = [
      this.id,
      json
    ];

    this.cb_send("wt_push_json", args, Buffer.alloc(0));
  }

  set_compression(compression: null | string): void {
    const args: any[] = [
      this.id,
      compression ? compression : ""
    ];

    this.cb_send("wt_set_compression", args, Buffer.alloc(0));
  }
}

type WebTransportHandler = (
  ctx: WebTransportCtx, 
  bytes: WebTransportBytes, 
  stream: WebTransportStream
) => void;

class WebTransport_ {
  id: number = 0;
  opts: WebTransportOpts;
  handlers: { [key: string]: WebTransportHandler } = {};
  socket_path: string = '/var/run/arnelify_server.sock';
  uds: UnixDomainSocket;

  constructor(opts: WebTransportOpts) {
    this.opts = opts;

    const uds_opts: UnixDomainSocketOpts = {
      block_size_kb: opts.block_size_kb,
      keep_alive: opts.send_timeout,
      socket_path: this.socket_path,
      thread_limit: opts.thread_limit
    };

    this.uds = new UnixDomainSocket(uds_opts);
    this.id = native.wt_create(JSON.stringify({
      keep_alive: opts.send_timeout,
      socket_path: this.socket_path,
      ...this.opts,
    }));
  }

  logger(cb: any): void {
    this.uds.on('wt_logger', (ctx: UnixDomainSocketCtx, _bytes: UnixDomainSocketBytes): void => {
      const [level, message] = ctx;
      cb(level, message);
    });
  }

  on(path: string, cb: WebTransportHandler): void {
    this.handlers[path] = cb;

    this.uds.on('wt_on', (ctx: UnixDomainSocketCtx, bytes: UnixDomainSocketBytes): void => {
      const [stream_id, handler_path, handler_ctx] = ctx;

      const stream = new WebTransportStream(stream_id);
      stream.on_send((topic, args, buffer): void => {
        this.uds.push(topic, args, buffer);
      });

      const cb = this.handlers[handler_path];
      cb(handler_ctx, bytes, stream);
    });

    native.wt_on(this.id, path);
  }

  async start(): Promise<void> {
    native.wt_start(this.id);
    this.uds.start();
  }

  async stop(): Promise<void> {
    native.wt_stop(this.id);
    this.uds.stop();
  }
}

export type { WebTransportOpts, WebTransportCtx, WebTransportBytes };
export { WebTransport_, WebTransportStream };