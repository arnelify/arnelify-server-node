"use strict";
// MIT LICENSE
Object.defineProperty(exports, "__esModule", { value: true });
exports.WebTransportStream = exports.WebTransportServer = void 0;
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
class WebTransportStream {
    constructor(id) {
        this.id = 0;
        this.cb_send = async (_topic, _args, bytes) => {
            console.log(bytes);
        };
        this.id = id;
    }
    async close() {
        const args = [this.id];
        await this.cb_send("wt_close", args, Buffer.alloc(0));
    }
    on_send(cb) {
        this.cb_send = cb;
    }
    async push(payload, bytes) {
        const args = [this.id, payload];
        await this.cb_send("wt_push", args, bytes);
    }
    async push_bytes(bytes) {
        const args = [this.id];
        await this.cb_send("wt_push_bytes", args, bytes);
    }
    async push_json(json) {
        const args = [this.id, json];
        await this.cb_send("wt_push_json", args, Buffer.alloc(0));
    }
    async set_compression(compression) {
        const args = [this.id, compression ? compression : ""];
        await this.cb_send("wt_set_compression", args, Buffer.alloc(0));
    }
}
exports.WebTransportStream = WebTransportStream;
class WebTransportServer {
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
        this.id = native.wt_create(JSON.stringify({
            socket_path: this.socket_path,
            ...this.opts,
        }));
    }
    logger(cb) {
        this.uds.on('wt_logger', async (ctx, _bytes) => {
            const [level, message] = ctx;
            await cb(level, message);
        });
        native.wt_logger(this.id);
    }
    on(path, cb) {
        this.handlers[path] = cb;
        this.uds.on('wt_on', async (ctx, bytes) => {
            const [stream_id, handler_path, handler_ctx] = ctx;
            const stream = new WebTransportStream(stream_id);
            stream.on_send(async (topic, args, buffer) => {
                await this.uds.push(topic, args, buffer);
            });
            const handler = this.handlers[handler_path];
            if (handler)
                await handler(handler_ctx, bytes, stream);
        });
        native.wt_on(this.id, path);
    }
    async start() {
        native.wt_start_ipc(this.id);
        await this.uds.start();
        native.wt_start(this.id);
    }
    async stop() {
        native.wt_stop(this.id);
        await this.uds.stop();
    }
}
exports.WebTransportServer = WebTransportServer;
