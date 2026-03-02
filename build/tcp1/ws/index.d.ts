import { UnixDomainSocket } from "../../ipc/uds";
type WebSocketOpts = {
    block_size_kb: number;
    compression: true;
    handshake_timeout: number;
    max_message_size_kb: number;
    ping_timeout: number;
    port: number;
    send_timeout: number;
    thread_limit: number;
};
type WebSocketCtx = [];
type WebSocketBytes = Buffer;
declare class WebSocketStream {
    id: number;
    topic: string;
    cb_send: (topic: string, args: any[], bytes: Buffer) => void;
    constructor(id: number);
    close(): void;
    on_send(cb: (topic: string, args: any[], bytes: Buffer) => void): void;
    push(json: any, bytes: Buffer): void;
    push_bytes(bytes: Buffer): void;
    push_json(json: any): void;
    set_compression(compression: null | string): void;
}
type WebSocketHandler = (ctx: WebSocketCtx, bytes: WebSocketBytes, stream: WebSocketStream) => void;
declare class WebSocket_ {
    id: number;
    opts: WebSocketOpts;
    handlers: {
        [key: string]: WebSocketHandler;
    };
    socket_path: string;
    uds: UnixDomainSocket;
    constructor(opts: WebSocketOpts);
    logger(cb: any): void;
    on(path: string, cb: WebSocketHandler): void;
    start(): Promise<void>;
    stop(): Promise<void>;
}
export type { WebSocketOpts, WebSocketCtx, WebSocketBytes };
export { WebSocket_, WebSocketStream };
//# sourceMappingURL=index.d.ts.map