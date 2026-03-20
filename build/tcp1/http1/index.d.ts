import { UnixDomainSocket } from "../../ipc/uds";
type Http1Opts = {
    allow_empty_files: boolean;
    block_size_kb: number;
    charset: string;
    compression: boolean;
    keep_alive: number;
    keep_extensions: boolean;
    max_fields: number;
    max_fields_size_total_mb: number;
    max_files: number;
    max_files_size_total_mb: number;
    max_file_size_mb: number;
    port: number;
    storage_path: string;
    thread_limit: number;
};
type Http1Ctx = Record<string, any>;
declare class Http1Stream {
    id: number;
    topic: string;
    cb_send: (topic: string, args: any[], bytes: Buffer) => Promise<void>;
    constructor(id: number);
    add_header(key: string, value: string): Promise<void>;
    end(): Promise<void>;
    on_send(cb: (topic: string, args: any[], bytes: Buffer) => Promise<void>): void;
    push_bytes(bytes: Buffer, is_attachment?: boolean): Promise<void>;
    push_file(file_path: string, is_attachment: boolean): Promise<void>;
    push_json(json: any, is_attachment?: boolean): Promise<void>;
    set_code(code: number): Promise<void>;
    set_compression(compression: null | string): Promise<void>;
    set_headers(headers: Record<string, string>[]): Promise<void>;
}
type Http1Handler = (ctx: Http1Ctx, stream: Http1Stream) => Promise<void>;
declare class Http1 {
    id: number;
    opts: Http1Opts;
    handlers: {
        [key: string]: Http1Handler;
    };
    socket_path: string;
    uds: UnixDomainSocket;
    constructor(opts: Http1Opts);
    logger(cb: any): void;
    on(path: string, cb: Http1Handler): void;
    start(): Promise<void>;
    stop(): Promise<void>;
}
export type { Http1Opts, Http1Ctx };
export { Http1, Http1Stream };
//# sourceMappingURL=index.d.ts.map