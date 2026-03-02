"use strict";
// MIT LICENSE
Object.defineProperty(exports, "__esModule", { value: true });
exports.Http1Stream = exports.Http1 = void 0;
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
class Http1Stream {
    constructor(id) {
        this.id = 0;
        this.topic = "";
        this.cb_send = (topic, args, bytes) => {
            console.log(bytes);
        };
        this.id = id;
    }
    add_header(key, value) {
        const args = [
            this.id,
            key,
            value
        ];
        this.cb_send("http1_add_header", args, Buffer.alloc(0));
    }
    end() {
        const args = [
            this.id
        ];
        this.cb_send("http1_end", args, Buffer.alloc(0));
    }
    on_send(cb) {
        this.cb_send = cb;
    }
    push_bytes(bytes, is_attachment = false) {
        const args = [
            this.id,
            is_attachment ? 1 : 0
        ];
        this.cb_send("http1_push_bytes", args, bytes);
    }
    push_file(file_path, is_attachment) {
        const args = [
            this.id,
            file_path,
            is_attachment ? 1 : 0
        ];
        this.cb_send("http1_push_file", args, Buffer.alloc(0));
    }
    push_json(json, is_attachment = false) {
        const args = [
            this.id,
            json,
            is_attachment ? 1 : 0
        ];
        this.cb_send("http1_push_json", args, Buffer.alloc(0));
    }
    set_code(code) {
        const args = [
            this.id,
            code
        ];
        this.cb_send("http1_set_code", args, Buffer.alloc(0));
    }
    set_compression(compression) {
        const args = [
            this.id,
            compression ? compression : ""
        ];
        this.cb_send("http1_set_compression", args, Buffer.alloc(0));
    }
    set_headers(headers) {
        const args = [
            this.id,
            headers
        ];
        this.cb_send("http1_set_headers", args, Buffer.alloc(0));
    }
}
exports.Http1Stream = Http1Stream;
class Http1 {
    constructor(opts) {
        this.id = 0;
        this.handlers = {};
        this.socket_path = '/var/run/arnelify_server.sock';
        this.opts = opts;
        const uds_opts = {
            block_size_kb: opts.block_size_kb,
            keep_alive: opts.keep_alive,
            socket_path: this.socket_path,
            thread_limit: opts.thread_limit
        };
        this.uds = new uds_1.UnixDomainSocket(uds_opts);
        this.id = native.http1_create(JSON.stringify({
            socket_path: this.socket_path,
            ...this.opts,
        }));
    }
    logger(cb) {
        this.uds.on('http1_logger', (ctx, _bytes) => {
            const [level, message] = ctx;
            cb(level, message);
        });
    }
    on(path, cb) {
        this.handlers[path] = cb;
        this.uds.on('http1_on', (ctx, _bytes) => {
            const [stream_id, handler_path, handler_ctx] = ctx;
            const stream = new Http1Stream(stream_id);
            stream.on_send((topic, args, buffer) => {
                this.uds.push(topic, args, buffer);
            });
            const cb = this.handlers[handler_path];
            cb(handler_ctx, stream);
        });
        native.http1_on(this.id, path);
    }
    async start() {
        native.http1_start(this.id);
        this.uds.start();
    }
    async stop() {
        native.http1_stop(this.id);
        this.uds.stop();
    }
}
exports.Http1 = Http1;
