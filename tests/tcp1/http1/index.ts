import {
  Http1,
  Http1Opts,
  Http1Ctx,
  Http1Stream
} from "../../../build";

(async function main() {

  const http1_opts: Http1Opts = {
    allow_empty_files: true,
    block_size_kb: 64,
    charset: "utf-8",
    compression: true,
    keep_alive: 30,
    keep_extensions: true,
    max_fields: 60,
    max_fields_size_total_mb: 1,
    max_files: 3,
    max_files_size_total_mb: 60,
    max_file_size_mb: 60,
    port: 4433,
    storage_path: "/var/www/node/storage",
    thread_limit: 4
  };

  const http1: Http1 = new Http1(http1_opts);
  http1.logger(async (_level: string, message: string): Promise<void> => {
    console.log(`[Arnelify Server]: ${message}`);
  });

  http1.on("/", async (ctx: Http1Ctx, stream: Http1Stream): Promise<void> => {
    const bytes: Buffer = Buffer.from(JSON.stringify(ctx));
    stream.set_code(200);
    await stream.push_bytes(bytes, false);
    await stream.end();
  });

  await http1.start();

})();