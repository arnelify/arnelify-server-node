import { Http2, Http2Opts, Http2Ctx, Http2Stream } from "../../../build";

(function main() {

const http2_opts: Http2Opts = {
  allow_empty_files: true,
  block_size_kb: 64,
  cert_pem: "certs/cert.pem",
  charset: "utf-8",
  compression: true,
  keep_alive: 30,
  keep_extensions: true,
  key_pem: "certs/key.pem",
  max_fields: 10,
  max_fields_size_total_mb: 60,
  max_files: 10,
  max_files_size_total_mb: 60,
  max_file_size_mb: 60,
  port: 4433,
  storage_path: "/var/www/node/storage",
  thread_limit: 4
};

const http2 = new Http2(http2_opts);

http2.on("/", (ctx: Http2Ctx, stream: Http2Stream): void => {

  stream.set_code(200);
  stream.push_json(ctx);
  stream.end();

});

http2.start();

})();