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

type WebSocketOpts = {
  block_size_kb: number;
  compression: boolean;
  handshake_timeout: number;
  max_message_size_kb: number;
  ping_timeout: number;
  port: number;
  send_timeout: number;
  thread_limit: number;
};

type WebSocketCtx = Record<string, any>;
type WebSocketBytes = Buffer;

class WebSocketStream {
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
    await this.cb_send("ws_close", args, Buffer.alloc(0));
  }

  on_send(cb: (topic: string, args: any[], bytes: Buffer) => Promise<void>): void {
    this.cb_send = cb;
  }

  async push(payload: any, bytes: Buffer): Promise<void> {
    const args: any[] = [this.id, payload];
    await this.cb_send("ws_push", args, bytes);
  }

  async push_bytes(bytes: Buffer): Promise<void> {
    const args: any[] = [this.id];
    await this.cb_send("ws_push_bytes", args, bytes);
  }

  async push_json(json: any): Promise<void> {
    const args: any[] = [this.id, json];
    await this.cb_send("ws_push_json", args, Buffer.alloc(0));
  }

  async set_compression(compression: null | string): Promise<void> {
    const args: any[] = [this.id, compression ? compression : ""];
    this.cb_send("ws_set_compression", args, Buffer.alloc(0));
  }
}

type WebSocketHandler = (ctx: WebSocketCtx, bytes: WebSocketBytes, stream: WebSocketStream) => Promise<void>;

class WebSocketServer {
  id: number = 0;
  opts: WebSocketOpts;
  handlers: { [key: string]: WebSocketHandler } = {};
  socket_path: string = '/var/run/arnelify_server.sock';
  uds: UnixDomainSocket;

  constructor(opts: WebSocketOpts) {
    this.opts = opts;

    const uds_opts: UnixDomainSocketOpts = {
      block_size_kb: opts.block_size_kb,
      socket_path: this.socket_path,
      thread_limit: opts.thread_limit
    };

    this.uds = new UnixDomainSocket(uds_opts);
    this.id = native.ws_create(JSON.stringify({
      socket_path: this.socket_path,
      ...this.opts,
    }));
  }

  logger(cb: any): void {
    this.uds.on('ws_logger', async (ctx: UnixDomainSocketCtx, _bytes: UnixDomainSocketBytes): Promise<void> => {
      const [level, message] = ctx;
      await cb(level, message);
    });

    native.ws_logger(this.id);
  }

  on(path: string, cb: WebSocketHandler): void {
    this.handlers[path] = cb;

    this.uds.on('ws_on', async (ctx: UnixDomainSocketCtx, bytes: UnixDomainSocketBytes): Promise<void> => {
      const [stream_id, handler_path, handler_ctx] = ctx;

      const stream = new WebSocketStream(stream_id);
      stream.on_send(async (topic, args, buffer): Promise<void> => {
        await this.uds.push(topic, args, buffer);
      });

      const handler = this.handlers[handler_path];
      if (handler) await handler(handler_ctx, bytes, stream);
    });

    native.ws_on(this.id, path);
  }

  async start(): Promise<void> {
    native.ws_start_ipc(this.id);
    await this.uds.start();
    native.ws_start(this.id);
  }

  async stop(): Promise<void> {
    native.ws_stop(this.id);
    await this.uds.stop();
  }
}

export type { WebSocketOpts, WebSocketCtx, WebSocketBytes };
export { WebSocketServer, WebSocketStream };