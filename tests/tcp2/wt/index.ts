import { WebTransport_, WebTransportOpts, WebTransportBytes, WebTransportCtx, WebTransportStream } from "../../../build";

(function main() {

  const ws_opts: WebTransportOpts = {
    block_size_kb: 64,
    cert_pem: "certs/cert.pem",
    compression: true,
    handshake_timeout: 64,
    key_pem: "certs/key.pem",
    max_message_size_kb: 64,
    ping_timeout: 30,
    port: 4433,
    send_timeout: 30,
    thread_limit: 4
  };

  const ws = new WebTransport_(ws_opts);

  ws.on("connect", (ctx: WebTransportCtx, bytes: WebTransportBytes, stream: WebTransportStream): void => {
    stream.push(ctx, bytes);
    stream.close();
  });

  ws.start();

})();