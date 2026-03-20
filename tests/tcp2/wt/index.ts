import {
  WebTransportServer,
  WebTransportOpts,
  WebTransportBytes,
  WebTransportCtx,
  WebTransportStream
} from "../../../build";

(async function main() {

  const wt_opts: WebTransportOpts = {
    block_size_kb: 64,
    cert_pem: "certs/cert.pem",
    compression: false,
    handshake_timeout: 30,
    key_pem: "certs/key.pem",
    max_message_size_kb: 64,
    ping_timeout: 15,
    port: 4433,
    send_timeout: 30,
    thread_limit: 4
  };

  const wt: WebTransportServer = new WebTransportServer(wt_opts);
  wt.logger(async (level: string, message: string): Promise<void> => {
    console.log(`[Arnelify Server]: ${message}`);
  });

  wt.on("connect", async (
    ctx: WebTransportCtx,
    bytes: WebTransportBytes,
    stream: WebTransportStream
  ): Promise<void> => {
    await stream.push(ctx, bytes);
    await stream.close();
  });

  await wt.start();

})();