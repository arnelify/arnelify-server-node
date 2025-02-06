import net from "net";
import { access, unlink } from "fs/promises";

/**
 * Arnelify Unix Domain Socket Server
 */
class ArnelifyUDS {

  #opts: { [key: string]: any } = {};
  #server: any = null;

  constructor(opts: { [key: string]: any }) {
    this.#opts = opts;
    this.#server = net.createServer((socket: any): void => {
      socket.on('data', async (block: Buffer<ArrayBuffer>): Promise<void> => {
        let buffer: string = block.toString();
        let size: number = 0;

        while (buffer.length > 0) {

          if (!size) {
            const sizeEnd: number = buffer.indexOf(":");
            const hasSizeEnd: boolean = sizeEnd != -1;
            if (hasSizeEnd) {
              size = Number(buffer.substring(0, sizeEnd));
              buffer = buffer.substring(sizeEnd + 1);
            }
          }

          if (size > buffer.length) break;

          if (buffer.length >= size) {
            const message: string = buffer.substring(0, size);
            let json: { [key: string]: any } = {};

            try {
              json = JSON.parse(message);

            } catch (err) {
              this.#callback("Message from UDS (Unix Domain Socket) must be in valid JSON format.", true);
              process.exit(1);
            }

            const { content, uuid } = json;

            if (!uuid) {
              this.#callback("The 'uuid' is missing in the message.", true);
              process.exit(1);
            }

            if (!content) {
              this.#callback("The 'content' is missing in the message.", true);
              process.exit(1);
            }

            await this.#handler(json, socket);
            buffer = buffer.substring(size);
            size = 0;
          }
        }
      });
    });
  }

  /**
   * Callback
   * @param {string} message
   * @param {boolean} isError
   */
  #callback: (message: string, isError: boolean) => void = (message: string, isError: boolean): void => {
    if (isError) console.log(`[Arnelify Unix Domain Socket]: NodeJS error: ${message}`);
  };

  /**
   * Connect
   * @param {CallableFunction} callback
   * @returns 
   */
  async connect(callback: (message: string, isErorr: boolean) => void): Promise<boolean> {
    if (!this.#opts) return false;
    this.#callback = callback;

    const isExists: boolean = await this.#exists();
    if (isExists) await unlink(this.#opts.UDS_SOCKET_PATH);

    this.#server.listen(this.#opts.UDS_SOCKET_PATH);
    return true;
  }

  /**
   * Exists
   * @returns
   */
  async #exists(): Promise<boolean> {
    try {
      await access(this.#opts.UDS_SOCKET_PATH);
      return true;

    } catch (err: any) {
      return false;
    }
  }

  /**
   * Handler
   * @param {object} json
   * @param {any} socket 
   */
  #handler: (json: { [key: string]: any }, socket: any) => Promise<void> = async (json: { [key: string]: any }, socket: any): Promise<void> => {
    const { content } = json;

    const { _state } = content;
    if (_state) {
      const res: string = JSON.stringify(json);
      socket.write(`${res.length}:${res}`);
      return;
    }

    const { _stdout } = content;
    if (_stdout) {
      const { message, isErorr } = _stdout;
      if (isErorr) {
        this.#callback(message, isErorr);
      }
    }
  };

  /**
   * Set Handler
   * @param {CallableFunction} handler
   */
  setHandler(handler: (req: { [key: string]: any }, socket: any) => Promise<void>): void {
    this.#handler = handler;
  }

  /**
   * Stop
   */
  async stop(): Promise<void> {
    this.#server.close();
  }
}

export default ArnelifyUDS;