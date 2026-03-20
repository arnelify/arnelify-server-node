import {
  WebSocketServer,
  WebSocketOpts,
  WebSocketBytes,
  WebSocketCtx,
  WebSocketStream
} from "../../../build";

(async function main() {

  const ws_opts: WebSocketOpts = {
    block_size_kb: 64,
    compression: false,
    handshake_timeout: 30,
    max_message_size_kb: 64,
    ping_timeout: 15,
    port: 4433,
    send_timeout: 30,
    thread_limit: 4
  };

  const ws: WebSocketServer = new WebSocketServer(ws_opts);
  ws.logger(async (_level: string, message: string): Promise<void> => {
    console.log(`[Arnelify Server]: ${message}`);
  });

  ws.on("connect", async (
    ctx: WebSocketCtx,
    bytes: WebSocketBytes,
    stream: WebSocketStream
  ): Promise<void> => {
    await stream.push(ctx, bytes);
    await stream.close();
  });

  await ws.start();

})();