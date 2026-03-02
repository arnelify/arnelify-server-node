import { UnixDomainSocket } from "../../ipc/uds";
type WebTransportOpts = {
    block_size_kb: number;
    cert_pem: string;
    compression: true;
    handshake_timeout: number;
    key_pem: string;
    max_message_size_kb: number;
    ping_timeout: number;
    port: number;
    send_timeout: number;
    thread_limit: number;
};
type WebTransportCtx = [];
type WebTransportBytes = Buffer;
declare class WebTransportStream {
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
type WebTransportHandler = (ctx: WebTransportCtx, bytes: WebTransportBytes, stream: WebTransportStream) => void;
declare class WebTransport_ {
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
export { WebTransport_, WebTransportStream };
//# sourceMappingURL=index.d.ts.map