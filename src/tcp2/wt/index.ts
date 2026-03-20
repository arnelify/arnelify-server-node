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
  compression: boolean;
  handshake_timeout: number;
  key_pem: string;
  max_message_size_kb: number;
  ping_timeout: number;
  port: number;
  send_timeout: number;
  thread_limit: number;
};

type WebTransportCtx = Record<string, any>;
type WebTransportBytes = Buffer;

class WebTransportStream {
  id: number = 0;

  cb_send: (topic: string, args: any[], bytes: Buffer) => Promise<void> =
    async (_topic: string, _args: any[], bytes: Buffer): Promise<void> => {
      console.log(bytes);
    };

  constructor(id: number) {
    this.id = id;
  }

  async close(): Promise<void> {
    const args: any[] = [this.id];
    await this.cb_send("wt_close", args, Buffer.alloc(0));
  }

  on_send(cb: (topic: string, args: any[], bytes: Buffer) => Promise<void>): void {
    this.cb_send = cb;
  }

  async push(payload: any, bytes: Buffer): Promise<void> {
    const args: any[] = [this.id, payload];
    await this.cb_send("wt_push", args, bytes);
  }

  async push_bytes(bytes: Buffer): Promise<void> {
    const args: any[] = [this.id];
    await this.cb_send("wt_push_bytes", args, bytes);
  }

  async push_json(json: any): Promise<void> {
    const args: any[] = [this.id, json];
    await this.cb_send("wt_push_json", args, Buffer.alloc(0));
  }

  set_compression(compression: null | string): void {
    const args: any[] = [this.id, compression ? compression : ""];
    this.cb_send("wt_set_compression", args, Buffer.alloc(0));
  }
}

type WebTransportHandler = (ctx: WebTransportCtx, bytes: WebTransportBytes, stream: WebTransportStream) => Promise<void>;

class WebTransportServer {
  id: number = 0;
  opts: WebTransportOpts;
  handlers: { [key: string]: WebTransportHandler } = {};
  socket_path: string = '/var/run/arnelify_server.sock';
  uds: UnixDomainSocket;

  constructor(opts: WebTransportOpts) {
    this.opts = opts;

    const uds_opts: UnixDomainSocketOpts = {
      block_size_kb: opts.block_size_kb,
      socket_path: this.socket_path,
      thread_limit: opts.thread_limit
    };

    this.uds = new UnixDomainSocket(uds_opts);
    this.id = native.wt_create(JSON.stringify({
      socket_path: this.socket_path,
      ...this.opts,
    }));
  }

  logger(cb: any): void {
    this.uds.on('wt_logger', async (ctx: UnixDomainSocketCtx, _bytes: UnixDomainSocketBytes): Promise<void> => {
      const [level, message] = ctx;
      await cb(level, message);
    });

    native.wt_logger(this.id);
  }

  on(path: string, cb: WebTransportHandler): void {
    this.handlers[path] = cb;

    this.uds.on('wt_on', async (ctx: UnixDomainSocketCtx, bytes: UnixDomainSocketBytes): Promise<void> => {
      const [stream_id, handler_path, handler_ctx] = ctx;

      const stream = new WebTransportStream(stream_id);
      stream.on_send(async (topic, args, buffer): Promise<void> => {
        await this.uds.push(topic, args, buffer);
      });

      const handler = this.handlers[handler_path];
      if (handler) await handler(handler_ctx, bytes, stream);
    });

    native.wt_on(this.id, path);
  }

  async start(): Promise<void> {
    native.wt_start_ipc(this.id);
    await this.uds.start();
    native.wt_start(this.id);
  }

  async stop(): Promise<void> {
    native.wt_stop(this.id);
    this.uds.stop();
  }
}

export type { WebTransportOpts, WebTransportCtx, WebTransportBytes };
export { WebTransportServer, WebTransportStream };