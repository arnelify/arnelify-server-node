type UnixDomainSocketBytes = Buffer;
interface UnixDomainSocketOpts {
    block_size_kb: number;
    keep_alive: number;
    socket_path: string;
    thread_limit: number;
}
type UnixDomainSocketCtx = any[];
type UnixDomainSocketRes = {
    [key: string]: any;
};
declare class UnixDomainSocketStream {
    opts: UnixDomainSocketOpts;
    topic: null | string;
    cb_send: (bytes: Buffer) => void;
    constructor(opts: UnixDomainSocketOpts);
    on_send(cb: (bytes: Buffer) => void): void;
    push(payload: UnixDomainSocketRes, bytes: Buffer): void;
    set_topic(topic: string): void;
}
type UnixDomainSocketHandler = (ctx: UnixDomainSocketCtx, bytes: UnixDomainSocketBytes) => void;
type UnixDomainSocketLogger = (level: string, message: string) => void;
declare class UnixDomainSocket {
    client: any;
    cb_logger: (_level: string, message: string) => void;
    cb_handlers: {
        [key: string]: (ctx: UnixDomainSocketCtx, bytes: UnixDomainSocketBytes) => void;
    };
    opts: UnixDomainSocketOpts;
    constructor(opts: UnixDomainSocketOpts);
    logger(cb: (level: string, message: string) => void): void;
    on(topic: string, cb: (ctx: UnixDomainSocketCtx, bytes: UnixDomainSocketBytes) => void): void;
    push(topic: string, payload: any, bytes: Buffer): void;
    start(): Promise<void>;
    stop(): void;
}
export type { UnixDomainSocketBytes, UnixDomainSocketCtx, UnixDomainSocketHandler, UnixDomainSocketLogger, UnixDomainSocketOpts, UnixDomainSocketRes, };
export { UnixDomainSocket, UnixDomainSocketStream, };
//# sourceMappingURL=index.d.ts.map