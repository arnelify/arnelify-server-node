import { Http1, Http1Opts, Http1Ctx, Http1Stream } from "../../../build";

(function main() {

const http1_opts: Http1Opts = {
  allow_empty_files: true,
  block_size_kb: 64,
  charset: "utf-8",
  compression: true,
  keep_alive: 30,
  keep_extensions: true,
  max_fields: 10,
  max_fields_size_total_mb: 60,
  max_files: 10,
  max_files_size_total_mb: 60,
  max_file_size_mb: 60,
  port: 4433,
  storage_path: "/var/www/node/storage",
  thread_limit: 4
};

const http1 = new Http1(http1_opts);

http1.on("/", (ctx: Http1Ctx, stream: Http1Stream): void => {

  stream.set_code(200);
  stream.push_json(ctx);
  stream.end();

});

http1.start();

})();