#!/usr/bin/env bun

import ArnelifyUDS from "./uds/index";

import Req from "./contracts/req";
import Res from "./contracts/res";

/**
 * ArnelifyServer
 */
class ArnelifyServer {

  #lib: any = null;
  #uds: any = null;

  constructor(opts: { [key: string]: any }) {
    const socketPath: string = opts.SERVER_SOCKET_PATH ?? "/tmp/arnelify.sock";
    this.#lib = require("../build/Release/arnelify-server.node");
    this.#uds = new ArnelifyUDS({
      "UDS_BLOCK_SIZE_KB": opts.SERVER_BLOCK_SIZE_KB,
      "UDS_SOCKET_PATH": socketPath
    });

    this.#lib.server_create(JSON.stringify(opts));
  }

  /**
   * Callback
   * @param {string} message 
   * @param {boolean} isError 
   */
  #callback = (message: string, isError: boolean): void => {
    if (isError) {
      console.log(`[Arnelify Server]: NodeJS error: ${message}`);
      return;
    }

    console.log(`[Arnelify Server]: ${message}`);
  };

  #handler: (req: Req, res: Res) => Promise<void> = async (req: Req, res: Res): Promise<void> => {
    res.setCode(200);
    res.addBody(JSON.stringify({
      code: 200,
      success: "Welcome to Arnelify Server"
    }));

    res.end();
  };

  /**
   * Set Handler
   * @param {CallableFunction} handler
   */
  setHandler(handler: (req: Req, res: Res) => Promise<void>): void {
    this.#handler = handler;
  }

  /**
   * Start
   * @param {CallableFunction} callback
   */
  async start(callback: (message: string, isError: boolean) => void): Promise<void> {
    this.#callback = callback;

    this.#uds.setHandler(async (json: { [key: string]: any }, socket: any): Promise<void> => {
      const { content } = json;

      const { _state } = content;
      if (_state) {
        const transmitter: Res = new Res();
        transmitter.setCallback(this.#callback);
        await this.#handler(content, transmitter);

        json.content = transmitter.toJson();
        const res: string = JSON.stringify(json);
        socket.write(`${res.length}:${res}`);
        return;
      }

      const { _stdout } = content;
      if (_stdout) {
        const { message, isError } = _stdout;
        this.#callback(message, isError);
      }
    });

    await this.#uds.connect((message: string, isError: boolean): void => {
      if (isError) {
        console.log(
          "\x1b[31m" +
          `[Arnelify Unix Domain Socket]: NodeJS error: ${message}` +
          "\x1b[0m"
        );
      }
    });

    this.#lib.server_start();
  }

  /**
   * Stop
   */
  stop() {
    this.#lib.server_stop();
  }

  /**
   * Destroy
   */
  destroy() {
    this.#lib.server_destroy();
  }
}

export default ArnelifyServer;
