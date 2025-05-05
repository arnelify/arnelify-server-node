#!/usr/bin/env bun
import ArnelifyUDS from "./uds/index";

import Http1Opts from "./contracts/opts";
import Http1Req from "./contracts/req";
import Http1Res from "./contracts/res";

/**
 * ArnelifyServer
 */
class Http1 {

  #lib: any = null;
  #opts: Http1Opts = {};
  #uds: any = null;

  constructor(opts: Http1Opts) {
    this.#opts = opts;
    const socketPath: string =
      this.#opts.SERVER_SOCKET_PATH ?? "/tmp/arnelify.sock";
    this.#lib = require("../build/Release/arnelify-server.node");
    this.#lib.server_http1_create(JSON.stringify(this.#opts));
    this.#uds = new ArnelifyUDS({
      "UDS_BLOCK_SIZE_KB": this.#opts.SERVER_BLOCK_SIZE_KB,
      "UDS_SOCKET_PATH": socketPath
    });
  }

  /**
   * Logger
   * @param {string} message 
   * @param {boolean} isError 
   */
  #logger = (message: string, isError: boolean): void => {
    if (isError) {
      console.log(`[Arnelify Server]: NodeJS error: ${message}`);
      return;
    }

    console.log(`[Arnelify Server]: ${message}`);
  };

  /**
   * Handler
   * @param {Req} req
   * @param {Res} res
   */
  #callback: (req: Http1Req, res: Http1Res) => Promise<void> = 
    async (req: Http1Req, res: Http1Res): Promise<void> => {
    res.setCode(200);
    res.addBody(JSON.stringify({
      code: 200,
      success: "Welcome to Arnelify Server"
    }));

    res.end();
  };

  /**
   * Set Handler
   * @param {CallableFunction} cb
   */
  handler(cb: (req: Http1Req, res: Http1Res) => Promise<void>): void {
    this.#callback = cb;
  }

  /**
   * Start
   * @param {CallableFunction} logger
   */
  async start(logger: (message: string, isError: boolean) => void): Promise<void> {
    this.#logger = logger;

    this.#uds.setHandler(async (client: any, json: { [key: string]: any }): Promise<void> => {
      const { content, uuid } = json;
      const { _state } = content;
      if (_state) {
        const transmitter: Http1Res = new Http1Res();
        transmitter.setLogger(this.#logger);
        await this.#callback(content, transmitter);

        json.content = transmitter.toJson();
        client.write(JSON.stringify(json));
        return;
      }

      const { _stdout } = content;
      if (_stdout) {
        const { message, isError } = _stdout;
        this.#logger(message, isError);
      }
    });

    this.#lib.uds_start();
    await this.#uds.connect((message: string, isError: boolean): void => {
      if (isError) {
        console.log(
          "\x1b[31m" +
          `[Arnelify Unix Domain Socket]: NodeJS error: ${message}` +
          "\x1b[0m"
        );
      }
    });

    this.#lib.server_http1_start();
  }

  /**
   * Stop
   */
  stop() {
    this.#lib.server_http1_stop();
  }

  /**
   * Destroy
   */
  destroy() {
    this.#lib.server_http1_destroy();
  }
}

export type { Http1Opts, Http1Req, Http1Res };
export { Http1 };