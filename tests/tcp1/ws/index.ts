import { WebSocket_, WebSocketOpts, WebSocketBytes, WebSocketCtx, WebSocketStream } from "../../../build";

(function main() {

  const ws_opts: WebSocketOpts = {
    block_size_kb: 64,
    compression: true,
    handshake_timeout: 64,
    max_message_size_kb: 64,
    ping_timeout: 30,
    port: 4433,
    send_timeout: 30,
    thread_limit: 4
  };

  const ws = new WebSocket_(ws_opts);

  ws.on("connect", (ctx: WebSocketCtx, bytes: WebSocketBytes, stream: WebSocketStream): void => {
    stream.push(ctx, bytes);
    stream.close();
  });

  ws.start();

})();