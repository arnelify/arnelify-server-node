import { UnixDomainSocket } from "../../ipc/uds";
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
declare class WebSocketStream {
    id: number;
    cb_send: (topic: string, args: any[], bytes: Buffer) => Promise<void>;
    constructor(id: number);
    close(): Promise<void>;
    on_send(cb: (topic: string, args: any[], bytes: Buffer) => Promise<void>): void;
    push(payload: any, bytes: Buffer): Promise<void>;
    push_bytes(bytes: Buffer): Promise<void>;
    push_json(json: any): Promise<void>;
    set_compression(compression: null | string): Promise<void>;
}
type WebSocketHandler = (ctx: WebSocketCtx, bytes: WebSocketBytes, stream: WebSocketStream) => Promise<void>;
declare class WebSocketServer {
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
export { WebSocketServer, WebSocketStream };
//# sourceMappingURL=index.d.ts.map