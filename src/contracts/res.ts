class Res {

  #res: { [key: string]: any } = {};

  constructor() {
    this.#res = {
      body: '',
      code: 200,
      filePath: '',
      headers: {},
      isStatic: false
    };
  }

  /**
   * Logger
   * @param {string} message 
   * @param {boolean} isError 
   * @returns 
   */
  #logger: (message: string, isError: boolean) => void =
    (message: string, isError: boolean): void => {
      if (isError) {
        console.log("\x1b[31m" + `[Arnelify Server]: ${message}` + "\x1b[0m");
        return;
      }

      console.log("\x1b[32m" + `[Arnelify POD]: ${message}` + "\x1b[0m");
    };

  /**
   * Set Logger
   * @param {CallableFunction} logger 
   */
  setLogger(logger: (message: string, isErorr: boolean) => void): void {
    this.#logger = logger;
  }

  /**
   * Set Code
   * @param {number} code 
   */
  setCode(code: number): void {
    this.#res.code = code;
  }

  /**
   * Set File
   * @param {string} filePath 
   * @param {boolean} isStatic 
   */
  setFile(filePath: string, isStatic: boolean = false): void {
    const hasBody: boolean = !!this.#res.body.length;
    if (hasBody) {
      this.#logger("Can't add an attachment to a Response that contains a body.", true);
      process.exit(1);
    }

    this.#res.filePath = filePath;
    this.#res.isStatic = isStatic;
  }

  /**
   * Set Header
   * @param {string} key 
   * @param {string} value 
   */
  setHeader(key: string, value: string): void {
    this.#res.headers[key] = value;
  }

  /**
   * Add Body
   * @param {string} chunk
   */
  addBody(chunk: string): void {
    const hasFile = !!this.#res.filePath.length;
    if (hasFile) {
      this.#logger("Can't add body to a Response that contains a file.", true);
      process.exit(1);
    }

    this.#res.body += chunk;
  }

  /**
   * End
   * @returns 
   */
  end(): void {
    const hasFile = !!this.#res.filePath.length;
    if (hasFile) {
      this.#res.body = '';
      return;
    }

    const hasBody = !!this.#res.body.length;
    if (hasBody) {
      this.#res.filePath = '';
      this.#res.isStatic = false;
      return;
    }
  }

  /**
   * To JSON
   * @returns 
   */
  toJson(): { [key: string]: any } {
    return this.#res;
  }
}

export default Res;