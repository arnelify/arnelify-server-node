"use strict";
// MIT LICENSE
Object.defineProperty(exports, "__esModule", { value: true });
exports.WebSocketStream = exports.WebSocketServer = void 0;
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
const uds_1 = require("../../ipc/uds");
class WebSocketStream {
    constructor(id) {
        this.id = 0;
        this.cb_send = async (_topic, _args, bytes) => {
            console.log(bytes);
        };
        this.id = id;
    }
    async close() {
        const args = [this.id];
        await this.cb_send("ws_close", args, Buffer.alloc(0));
    }
    on_send(cb) {
        this.cb_send = cb;
    }
    async push(payload, bytes) {
        const args = [this.id, payload];
        await this.cb_send("ws_push", args, bytes);
    }
    async push_bytes(bytes) {
        const args = [this.id];
        await this.cb_send("ws_push_bytes", args, bytes);
    }
    async push_json(json) {
        const args = [this.id, json];
        await this.cb_send("ws_push_json", args, Buffer.alloc(0));
    }
    set_compression(compression) {
        const args = [this.id, compression ? compression : ""];
        this.cb_send("ws_set_compression", args, Buffer.alloc(0));
    }
}
exports.WebSocketStream = WebSocketStream;
class WebSocketServer {
    constructor(opts) {
        this.id = 0;
        this.handlers = {};
        this.socket_path = '/var/run/arnelify_server.sock';
        this.opts = opts;
        const uds_opts = {
            block_size_kb: opts.block_size_kb,
            socket_path: this.socket_path,
            thread_limit: opts.thread_limit
        };
        this.uds = new uds_1.UnixDomainSocket(uds_opts);
        this.id = native.ws_create(JSON.stringify({
            socket_path: this.socket_path,
            ...this.opts,
        }));
    }
    logger(cb) {
        this.uds.on('ws_logger', async (ctx, _bytes) => {
            const [level, message] = ctx;
            await cb(level, message);
        });
        native.ws_logger(this.id);
    }
    on(path, cb) {
        this.handlers[path] = cb;
        this.uds.on('ws_on', async (ctx, bytes) => {
            const [stream_id, handler_path, handler_ctx] = ctx;
            const stream = new WebSocketStream(stream_id);
            stream.on_send(async (topic, args, buffer) => {
                await this.uds.push(topic, args, buffer);
            });
            const handler = this.handlers[handler_path];
            if (handler)
                await handler(handler_ctx, bytes, stream);
        });
        native.ws_on(this.id, path);
    }
    async start() {
        native.ws_start_ipc(this.id);
        await this.uds.start();
        native.ws_start(this.id);
    }
    async stop() {
        native.ws_stop(this.id);
        this.uds.stop();
    }
}
exports.WebSocketServer = WebSocketServer;
