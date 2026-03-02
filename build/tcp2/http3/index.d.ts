import { UnixDomainSocket } from "../../ipc/uds";
type Http3Opts = {
    allow_empty_files: boolean;
    block_size_kb: number;
    cert_pem: string;
    charset: string;
    compression: boolean;
    keep_alive: number;
    keep_extensions: boolean;
    key_pem: string;
    max_fields: number;
    max_fields_size_total_mb: number;
    max_files: number;
    max_files_size_total_mb: number;
    max_file_size_mb: number;
    port: number;
    storage_path: string;
    thread_limit: number;
};
type Http3Ctx = [];
declare class Http3Stream {
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
type Http3Handler = (ctx: Http3Ctx, stream: Http3Stream) => void;
declare class Http3 {
    id: number;
    opts: Http3Opts;
    handlers: {
        [key: string]: Http3Handler;
    };
    socket_path: string;
    uds: UnixDomainSocket;
    constructor(opts: Http3Opts);
    logger(cb: any): void;
    on(path: string, cb: Http3Handler): void;
    start(): Promise<void>;
    stop(): Promise<void>;
}
export type { Http3Opts, Http3Ctx };
export { Http3, Http3Stream };
//# sourceMappingURL=index.d.ts.map