import { UnixDomainSocket } from "../../ipc/uds";
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
declare class WebTransportStream {
    id: number;
    cb_send: (topic: string, args: any[], bytes: Buffer) => Promise<void>;
    constructor(id: number);
    close(): Promise<void>;
    on_send(cb: (topic: string, args: any[], bytes: Buffer) => Promise<void>): void;
    push(payload: any, bytes: Buffer): Promise<void>;
    push_bytes(bytes: Buffer): Promise<void>;
    push_json(json: any): Promise<void>;
    set_compression(compression: null | string): void;
}
type WebTransportHandler = (ctx: WebTransportCtx, bytes: WebTransportBytes, stream: WebTransportStream) => Promise<void>;
declare class WebTransportServer {
    id: number;
    opts: WebTransportOpts;
    handlers: {
        [key: string]: WebTransportHandler;
    };
    socket_path: string;
    uds: UnixDomainSocket;
    constructor(opts: WebTransportOpts);
    logger(cb: any): void;
    on(path: string, cb: WebTransportHandler): void;
    start(): Promise<void>;
    stop(): Promise<void>;
}
export type { WebTransportOpts, WebTransportCtx, WebTransportBytes };
export { WebTransportServer, WebTransportStream };
//# sourceMappingURL=index.d.ts.map