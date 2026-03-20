type UnixDomainSocketBytes = Buffer;
interface UnixDomainSocketOpts {
    block_size_kb: number;
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
    cb_send: (bytes: Buffer) => Promise<void>;
    constructor(opts: UnixDomainSocketOpts);
    on_send(cb: (bytes: Buffer) => Promise<void>): void;
    push(payload: UnixDomainSocketRes, bytes: Buffer): Promise<void>;
    set_topic(topic: string): void;
}
type UnixDomainSocketHandler = (ctx: UnixDomainSocketCtx, bytes: UnixDomainSocketBytes) => Promise<void>;
type UnixDomainSocketLogger = (level: string, message: string) => Promise<void>;
declare class UnixDomainSocket {
    opts: UnixDomainSocketOpts;
    client: any;
    cb_handlers: Record<string, UnixDomainSocketHandler>;
    cb_logger: (_level: string, message: string) => Promise<void>;
    constructor(opts: UnixDomainSocketOpts);
    logger(cb: UnixDomainSocketLogger): void;
    on(topic: string, cb: UnixDomainSocketHandler): void;
    push(topic: string, payload: any, bytes: Buffer): Promise<void>;
    start(): Promise<void>;
    stop(): Promise<void>;
}
export type { UnixDomainSocketBytes, UnixDomainSocketCtx, UnixDomainSocketHandler, UnixDomainSocketLogger, UnixDomainSocketOpts, UnixDomainSocketRes, };
export { UnixDomainSocket, UnixDomainSocketStream, };
//# sourceMappingURL=index.d.ts.map