#!/usr/bin/env bun
import { Http1, Http1Opts, Http1Req, Http1Res } from "../index";

(function main(): number {

  const opts: Http1Opts = {
    "SERVER_ALLOW_EMPTY_FILES": true,
    "SERVER_BLOCK_SIZE_KB": 64,
    "SERVER_CHARSET": "UTF-8",
    "SERVER_GZIP": true,
    "SERVER_KEEP_EXTENSIONS": true,
    "SERVER_MAX_FIELDS": 1024,
    "SERVER_MAX_FIELDS_SIZE_TOTAL_MB": 20,
    "SERVER_MAX_FILES": 1,
    "SERVER_MAX_FILES_SIZE_TOTAL_MB": 60,
    "SERVER_MAX_FILE_SIZE_MB": 60,
    "SERVER_PORT": 3001,
    "SERVER_NET_CHECK_FREQ_MS": 50,
    "SERVER_THREAD_LIMIT": 5,
    "SERVER_QUEUE_LIMIT": 1024,
    "SERVER_UPLOAD_DIR": "storage/upload"
  };

  const http1: Http1 = new Http1(opts);
  http1.handler(async (req: Http1Req, res: Http1Res): Promise<void> => {
    res.setCode(200);
    res.addBody(JSON.stringify(req));
    res.end();
  });

  http1.start((message: string, isError: boolean): void => {
    if (isError) {
      console.log(`[Arnelify Server]: Error: ${message}`);
      return;
    }

    console.log(`[Arnelify Server]: ${message}`);
  });

  return 0;

})();