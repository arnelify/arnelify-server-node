import { UnixDomainSocket } from "../../ipc/uds";
type Http2Opts = {
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
type Http2Ctx = [];
declare class Http2Stream {
    id: number;
    topic: string;
    cb_send: (topic: string, args: any[], bytes: Buffer) => void;
    constructor(id: number);
    add_header(key: string, value: string): void;
    end(): void;
    on_send(cb: (topic: string, args: any[], bytes: Buffer) => void): void;
    push_bytes(bytes: Buffer, is_attachment?: boolean): void;
    push_file(file_path: string, is_attachment: boolean): void;
    push_json(json: any, is_attachment?: boolean): void;
    set_code(code: number): void;
    set_compression(compression: null | string): void;
    set_headers(headers: Record<string, string>[]): void;
}
type Http2Handler = (ctx: Http2Ctx, stream: Http2Stream) => void;
declare class Http2 {
    id: number;
    opts: Http2Opts;
    handlers: {
        [key: string]: Http2Handler;
    };
    socket_path: string;
    uds: UnixDomainSocket;
    constructor(opts: Http2Opts);
    logger(cb: any): void;
    on(path: string, cb: (ctx: Http2Ctx, stream: Http2Stream) => void): void;
    start(): Promise<void>;
    stop(): Promise<void>;
}
export type { Http2Opts, Http2Ctx };
export { Http2, Http2Stream };
//# sourceMappingURL=index.d.ts.map