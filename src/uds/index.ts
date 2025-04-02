import net from "net";
import { access } from "fs/promises";

/**
 * Arnelify Unix Domain Socket Server
 */
class ArnelifyUDS {

  #opts: { [key: string]: any } = {};
  #socket: any = null;

  constructor(opts: { [key: string]: any }) {
    this.#opts = opts;
    this.#socket = new net.Socket();
  }

  /**
   * Logger
   * @param {string} message
   * @param {boolean} isError
   */
  #logger: (message: string,
    isError: boolean) => void = (message: string, isError: boolean): void => {
      if (isError) console.log(`[Arnelify Unix Domain Socket]: NodeJS error: ${message}`);
    };

  /**
   * Connect
   * @param {CallableFunction} logger
   * @returns 
   */
  async connect(logger: (message: string, isErorr: boolean) => void): Promise<boolean> {
    if (!this.#opts) return false;
    this.#logger = logger;

    let isExists: boolean = await this.#exists();
    while (!isExists) isExists = await this.#exists();
    this.#socket.connect(this.#opts.UDS_SOCKET_PATH);
    this.#socket.on('data', async (data: Buffer<ArrayBufferLike>): Promise<void> => {
      await this.#on(data);
    });

    this.#socket.on('error', (err: Error) => {
      this.#logger(`Error occurred: ${err.message}`, true);
    });

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
   * Get Bytes
   * @param buffer 
   * @returns 
   */
  #getBytes(buffer: string): number {
    const utf8: Buffer<ArrayBuffer> = Buffer.from(buffer, 'utf8');
    return utf8.length;
  }

  /**
   * Handler
   * @param {object} json
   * @param {any} socket 
   */
  #handler: (client: any, json: { [key: string]: any }) => Promise<void> =
    async (client: any, json: { [key: string]: any }): Promise<void> => {
      const { content } = json;

      const { _state } = content;
      if (_state) {
        client.write(JSON.stringify(json));
        return;
      }

      const { _stdout } = content;
      if (_stdout) {
        const { message, isErorr } = _stdout;
        if (isErorr) {
          this.#logger(message, isErorr);
        }
      }
    };

  /**
   * On
   * @param {Buffer} block 
   */
  async #on(block: Buffer<ArrayBufferLike>): Promise<void> {
    let buffer: string = block.toString();
    let size: number = 0;

    while (this.#getBytes(buffer) > 0) {

      if (!size) {
        const sizeEnd: number = buffer.indexOf(":");
        const hasSizeEnd: boolean = sizeEnd != -1;
        if (hasSizeEnd) {
          size = Number(buffer.substring(0, sizeEnd));
          buffer = buffer.substring(sizeEnd + 1);
        }
      }

      if (size > this.#getBytes(buffer)) break;
      if (this.#getBytes(buffer) >= size) {
        const message: string = buffer.substring(0, size);
        let json: { [key: string]: any } = {};

        try {
          json = JSON.parse(message);

        } catch (err) {
          this.#logger("Message from UDS (Unix Domain Socket) must be in valid JSON format.", true);
          process.exit(1);
        }

        const { content, uuid } = json;
        if (!content || !uuid) {
          this.#logger("The 'uuid' or 'content' is missing in the message.", true);
          process.exit(1);
        }

        await this.#handler(this, json);
        buffer = buffer.substring(size);
        size = 0;
      }
    }
  }

  /**
   * Set Handler
   * @param {CallableFunction} handler
   */
  setHandler(handler: (client: any, json: { [key: string]: any }) => Promise<void>): void {
    this.#handler = handler;
  }

  /**
   * Stop
   */
  async stop(): Promise<void> {
    if (this.#socket) this.#socket.close();
  }

  /**
   * Write
   * @param {object} content 
   */
  write(content: string): void {
    const hasWritableSocket: boolean =
      this.#socket && this.#socket.writable;
    if (hasWritableSocket) {
      const message: string = `${this.#getBytes(content)}:${content}`;
      this.#socket.write(message, (err: any): void => {
        if (err) {
          this.#logger(`Failed to send message to UDS (Unix Domain Socket).`, true);
          return;
        }
      });
    }
  }
}

export default ArnelifyUDS;