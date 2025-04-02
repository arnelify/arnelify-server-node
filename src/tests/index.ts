#!/usr/bin/env bun
import { ArnelifyServer, ArnelifyServerOpts, Req, Res } from "../index";

(function main(): number {

  const opts: ArnelifyServerOpts = {
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
    "SERVER_QUEUE_LIMIT": 1024,
    "SERVER_UPLOAD_DIR": "./src/storage/upload"
  };

  const server: ArnelifyServer = new ArnelifyServer(opts);
  server.setHandler(async (req: Req, res: Res): Promise<void> => {
    res.setCode(200);
    res.addBody(JSON.stringify(req));
    res.end();
  });

  server.start((message: string, isError: boolean): void => {
    if (isError) {
      console.log(`[Arnelify Server]: Error: ${message}`);
      return;
    }

    console.log(`[Arnelify Server]: ${message}`);
  });

  return 0;

})();