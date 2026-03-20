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

import net from "net";

type UnixDomainSocketBytes = Buffer;
interface UnixDomainSocketOpts {
  block_size_kb: number,
  socket_path: string,
  thread_limit: number,
};

type UnixDomainSocketCtx = any[];
type UnixDomainSocketRes = { [key: string]: any };

class UnixDomainSocketReq {
  opts: UnixDomainSocketOpts;
  has_meta: boolean = false;
  has_body: boolean = false;
  json_length: number = 0;
  binary_length: number = 0;
  topic: string = '';
  buff: Buffer = Buffer.alloc(0);
  binary: Buffer = Buffer.alloc(0);
  ctx: any[] = [];

  constructor(opts: UnixDomainSocketOpts) {
    this.opts = opts;
  }

  add(buff: Buffer): void {
    this.buff = Buffer.concat([
      this.buff,
      buff
    ]);
  }

  get_ctx(): UnixDomainSocketCtx {
    return this.ctx;
  }

  get_bytes(): Buffer {
    return this.binary;
  }

  get_topic(): string {
    return this.topic;
  }

  is_empty(): boolean {
    return this.buff.length === 0;
  }

  read_meta(meta_end: number): string | number {
    const meta_bytes: Buffer = this.buff.subarray(0, meta_end);
    const pos: number = meta_bytes.indexOf(43);
    if (pos === -1) return 'Missing \'+\' in meta';

    const json_length: number = Number(meta_bytes.subarray(0, pos));
    const binary_length: number = Number(meta_bytes.subarray(pos + 1));
    if (!Number.isInteger(json_length) || !Number.isInteger(binary_length)) {
      return 'Invalid meta.';
    }

    this.json_length = json_length;
    this.binary_length = binary_length;

    return 1;
  }

  read_body(): null | string | number {
    if (!this.has_meta) {
      const meta_end: number = this.buff.indexOf(58);
      if (meta_end === -1) {
        if (this.buff.length > 8192) {
          this.buff = Buffer.alloc(0);
          return 'The maximum size of the meta has been exceeded.';
        }

        return null;
      }

      const res: string | number = this.read_meta(meta_end);
      if (typeof res === 'string') return res;

      this.has_meta = true;
      this.buff = this.buff.subarray(meta_end + 1);
    }

    if (this.json_length !== 0 && this.buff.length >= this.json_length) {
      const raw: string = this.buff
        .subarray(0, this.json_length)
        .toString();

      let json: { [key: string]: any } = {};
      try {
        json = JSON.parse(raw);
      } catch {
        this.buff = Buffer.alloc(0);
        return 'Invalid JSON.';
      }

      if (!json.hasOwnProperty('topic')
        || !json.hasOwnProperty('payload')) {
        this.buff = Buffer.alloc(0);
        return 'Invalid message.';
      }

      this.topic = json.topic;
      this.ctx = json.payload;
      this.buff = this.buff.subarray(this.json_length);
      if (this.binary_length === 0) {
        this.has_body = true;
        return 1;
      }
    }

    if (this.binary_length !== 0 && this.buff.length >= this.binary_length) {
      this.binary = this.buff.subarray(0, this.binary_length);
      this.buff = this.buff.subarray(this.binary_length);
      this.has_body = true;
      return 1;
    }

    return null;
  }

  read_block(): null | string | number {
    if (!this.has_body) {
      const res: null | string | number = this.read_body();
      if (typeof res === 'string') return res;
      if (res === null) return null;
    }

    return 1;
  }

  reset(): void {
    this.has_meta = false;
    this.has_body = false;

    this.topic = "";
    this.binary = Buffer.alloc(0);

    this.json_length = 0;
    this.binary_length = 0;
    this.ctx = [];
  }
}

class UnixDomainSocketStream {
  opts: UnixDomainSocketOpts;
  topic: null | string = null;

  cb_send = async (bytes: Buffer): Promise<void> => {
    console.log(bytes);
  };

  constructor(opts: UnixDomainSocketOpts) {
    this.opts = opts;
  }

  on_send(cb: (bytes: Buffer) => Promise<void>): void {
    this.cb_send = cb;
  }

  async push(payload: UnixDomainSocketRes, bytes: Buffer): Promise<void> {
    const json = Buffer.from(JSON.stringify({
      topic: this.topic,
      payload
    }), 'utf8');

    bytes = bytes ? Buffer.from(bytes) : Buffer.alloc(0);
    const meta = Buffer.from(`${json.length}+${bytes.length}:`, 'utf8');
    const buff = Buffer.concat([meta, json, bytes]);
    this.cb_send(buff);
  }

  set_topic(topic: string): void {
    this.topic = topic;
  }
}

type UnixDomainSocketHandler = (ctx: UnixDomainSocketCtx, bytes: UnixDomainSocketBytes) => Promise<void>;
type UnixDomainSocketLogger = (level: string, message: string) => Promise<void>;

class UnixDomainSocket {
  opts: UnixDomainSocketOpts;
  client: any;
  cb_handlers: Record<string, UnixDomainSocketHandler> = {};

  cb_logger = async (_level: string, message: string): Promise<void> => {
    console.log(message);
  };

  constructor(opts: UnixDomainSocketOpts) {
    this.opts = opts;
  }

  logger(cb: UnixDomainSocketLogger): void {
    this.cb_logger = cb;
  }

  on(topic: string, cb: UnixDomainSocketHandler): void {
    this.cb_handlers[topic] = cb;
  }

  async push(topic: string, payload: any, bytes: Buffer): Promise<void> {
    if (!this.client) {
      await this.cb_logger("error", `No client connected for push: ${topic}`);
      return;
    }

    const json = JSON.stringify({ topic, payload });
    const json_bytes = Buffer.from(json, "utf-8");
    const json_length = json_bytes.length;
    const bytes_length = bytes.length;
    const meta = Buffer.from(`${json_length}+${bytes_length}:`, "utf-8");
    const buff = Buffer.concat([meta, json_bytes, bytes]);

    return new Promise((resolve, reject) => {
      this.client.write(buff, (err: any): void => {
        if (err) reject(err);
        else resolve();
      });
    });
  }

  async start(): Promise<void> {
    const req: UnixDomainSocketReq = new UnixDomainSocketReq(this.opts);
    const stream: UnixDomainSocketStream = new UnixDomainSocketStream(this.opts);
    stream.on_send(async (bytes: Buffer): Promise<void> => {
      return new Promise((resolve, reject) => {
        this.client.write(bytes, (err: any): void => {
          if (err) reject(err);
          else resolve();
        });
      });
    });

    this.client = net.createConnection(this.opts.socket_path);
    this.client.on('data', async (bytes: Buffer): Promise<void> => {
      req.add(bytes);

      while (true) {
        const res: null | string | number = req.read_block();
        if (res === 1) {
          const topic: string = req.get_topic();
          const bytes: Buffer = req.get_bytes();
          const json: UnixDomainSocketCtx = req.get_ctx();

          stream.set_topic(topic);
          req.reset();

          if (this.cb_handlers.hasOwnProperty(topic)) {
            const handler: UnixDomainSocketHandler = this.cb_handlers[topic];
            if (handler) await handler(json, bytes);
          }
        } else if (typeof res === 'string') {
          await this.cb_logger("error", res);
          process.exit(1);
        }

        if (req.is_empty()) break;
      }
    });

    this.client.on('error', async (e: any): Promise<void> => {
      await this.cb_logger('error', `Connection error: ${e.message}`);
    });

    this.client.on('close', async (): Promise<void> => {
      await this.cb_logger('error', 'Connection closed');
    });
  }

  async stop(): Promise<void> {
    this.client.end();
  }
}

export type {
  UnixDomainSocketBytes, UnixDomainSocketCtx, UnixDomainSocketHandler,
  UnixDomainSocketLogger, UnixDomainSocketOpts, UnixDomainSocketRes,
}

export {
  UnixDomainSocket, UnixDomainSocketStream,
}