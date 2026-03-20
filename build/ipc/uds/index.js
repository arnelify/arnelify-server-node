"use strict";
// MIT LICENSE
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.UnixDomainSocketStream = exports.UnixDomainSocket = void 0;
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
const net_1 = __importDefault(require("net"));
;
class UnixDomainSocketReq {
    constructor(opts) {
        this.has_meta = false;
        this.has_body = false;
        this.json_length = 0;
        this.binary_length = 0;
        this.topic = '';
        this.buff = Buffer.alloc(0);
        this.binary = Buffer.alloc(0);
        this.ctx = [];
        this.opts = opts;
    }
    add(buff) {
        this.buff = Buffer.concat([
            this.buff,
            buff
        ]);
    }
    get_ctx() {
        return this.ctx;
    }
    get_bytes() {
        return this.binary;
    }
    get_topic() {
        return this.topic;
    }
    is_empty() {
        return this.buff.length === 0;
    }
    read_meta(meta_end) {
        const meta_bytes = this.buff.subarray(0, meta_end);
        const pos = meta_bytes.indexOf(43);
        if (pos === -1)
            return 'Missing \'+\' in meta';
        const json_length = Number(meta_bytes.subarray(0, pos));
        const binary_length = Number(meta_bytes.subarray(pos + 1));
        if (!Number.isInteger(json_length) || !Number.isInteger(binary_length)) {
            return 'Invalid meta.';
        }
        this.json_length = json_length;
        this.binary_length = binary_length;
        return 1;
    }
    read_body() {
        if (!this.has_meta) {
            const meta_end = this.buff.indexOf(58);
            if (meta_end === -1) {
                if (this.buff.length > 8192) {
                    this.buff = Buffer.alloc(0);
                    return 'The maximum size of the meta has been exceeded.';
                }
                return null;
            }
            const res = this.read_meta(meta_end);
            if (typeof res === 'string')
                return res;
            this.has_meta = true;
            this.buff = this.buff.subarray(meta_end + 1);
        }
        if (this.json_length !== 0 && this.buff.length >= this.json_length) {
            const raw = this.buff
                .subarray(0, this.json_length)
                .toString();
            let json = {};
            try {
                json = JSON.parse(raw);
            }
            catch {
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
    read_block() {
        if (!this.has_body) {
            const res = this.read_body();
            if (typeof res === 'string')
                return res;
            if (res === null)
                return null;
        }
        return 1;
    }
    reset() {
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
    constructor(opts) {
        this.topic = null;
        this.cb_send = async (bytes) => {
            console.log(bytes);
        };
        this.opts = opts;
    }
    on_send(cb) {
        this.cb_send = cb;
    }
    async push(payload, bytes) {
        const json = Buffer.from(JSON.stringify({
            topic: this.topic,
            payload
        }), 'utf8');
        bytes = bytes ? Buffer.from(bytes) : Buffer.alloc(0);
        const meta = Buffer.from(`${json.length}+${bytes.length}:`, 'utf8');
        const buff = Buffer.concat([meta, json, bytes]);
        this.cb_send(buff);
    }
    set_topic(topic) {
        this.topic = topic;
    }
}
exports.UnixDomainSocketStream = UnixDomainSocketStream;
class UnixDomainSocket {
    constructor(opts) {
        this.cb_handlers = {};
        this.cb_logger = async (_level, message) => {
            console.log(message);
        };
        this.opts = opts;
    }
    logger(cb) {
        this.cb_logger = cb;
    }
    on(topic, cb) {
        this.cb_handlers[topic] = cb;
    }
    async push(topic, payload, bytes) {
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
            this.client.write(buff, (err) => {
                if (err)
                    reject(err);
                else
                    resolve();
            });
        });
    }
    async start() {
        const req = new UnixDomainSocketReq(this.opts);
        const stream = new UnixDomainSocketStream(this.opts);
        stream.on_send(async (bytes) => {
            return new Promise((resolve, reject) => {
                this.client.write(bytes, (err) => {
                    if (err)
                        reject(err);
                    else
                        resolve();
                });
            });
        });
        this.client = net_1.default.createConnection(this.opts.socket_path);
        this.client.on('data', async (bytes) => {
            req.add(bytes);
            while (true) {
                const res = req.read_block();
                if (res === 1) {
                    const topic = req.get_topic();
                    const bytes = req.get_bytes();
                    const json = req.get_ctx();
                    stream.set_topic(topic);
                    req.reset();
                    if (this.cb_handlers.hasOwnProperty(topic)) {
                        const handler = this.cb_handlers[topic];
                        if (handler)
                            await handler(json, bytes);
                    }
                }
                else if (typeof res === 'string') {
                    await this.cb_logger("error", res);
                    process.exit(1);
                }
                if (req.is_empty())
                    break;
            }
        });
        this.client.on('error', async (e) => {
            await this.cb_logger('error', `Connection error: ${e.message}`);
        });
        this.client.on('close', async () => {
            await this.cb_logger('error', 'Connection closed');
        });
    }
    async stop() {
        this.client.end();
    }
}
exports.UnixDomainSocket = UnixDomainSocket;
