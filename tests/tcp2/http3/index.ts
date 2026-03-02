import { Http3, Http3Opts, Http3Ctx, Http3Stream } from "../../../build";

(function main() {

const http3_opts: Http3Opts = {
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

const http3 = new Http3(http3_opts);

http3.on("/", (ctx: Http3Ctx, stream: Http3Stream): void => {

  stream.set_code(200);
  stream.push_json(ctx);
  stream.end();

});

http3.start();

})();