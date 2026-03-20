import { 
  Http3, 
  Http3Opts, 
  Http3Ctx, 
  Http3Stream
} from "../../../build";

(async function main() {

  const http3_opts: Http3Opts = {
    allow_empty_files: true,
    block_size_kb: 64,
    cert_pem: "certs/cert.pem",
    charset: "utf-8",
    compression: true,
    keep_alive: 30,
    keep_extensions: true,
    key_pem: "certs/key.pem",
    max_fields: 60,
    max_fields_size_total_mb: 1,
    max_files: 3,
    max_files_size_total_mb: 60,
    max_file_size_mb: 60,
    port: 4433,
    storage_path: "/var/www/node/storage",
    thread_limit: 4
  };

  const http3: Http3 = new Http3(http3_opts);
  http3.logger(async (_level: string, message: string): Promise<void> => {
    console.log(`[Arnelify Server]: ${message}`);
  });

  http3.on("/", async (ctx: Http3Ctx, stream: Http3Stream): Promise<void> => {
    const bytes: Buffer = Buffer.from(JSON.stringify(ctx));
    stream.set_code(200);
    await stream.push_bytes(bytes, false);
    await stream.end();
  });

  await http3.start();

})();