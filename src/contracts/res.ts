class Res {

  #res: {[key: string]: any} = {};

  constructor() {
    this.#res = {
      body: '',
      code: 200,
      filePath: '',
      headers: {},
      isStatic: false
    };
  }

  #callback: (message: string, isError: boolean) => void = (message: string, isError): void => {
    if (isError) {
      console.log("\x1b[31m" + `[Arnelify Server]: ${message}` + "\x1b[0m");
      return;
    }

    console.log("\x1b[32m" + `[Arnelify POD]: ${message}` + "\x1b[0m");
  };

  setCallback(callback: (message: string, isErorr: boolean) => void) {
    this.#callback = callback;
  }

  setCode(code: number): void {
    this.#res.code = code;
  }

  setFile(filePath: string, isStatic: boolean = false): void {
    const hasBody: boolean = !!this.#res.body.length;
    if (hasBody) {
      this.#callback("Can't add an attachment to a Response that contains a body.", true);
      process.exit(1);
    }

    this.#res.filePath = filePath;
    this.#res.isStatic = isStatic;
  }

  setHeader(key: string, value: string): void {
    this.#res.headers[key] = value;
  }

  addBody(chunk: string) {
    const hasFile = !!this.#res.filePath.length;
    if (hasFile) {
      this.#callback("Can't add body to a Response that contains a file.", true);
      process.exit(1);
    }

    this.#res.body += chunk;
  }

  end() {
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

    this.#callback("Add the body or set the file.", true);
    process.exit(1);
  }

  toJson() {
    return this.#res;
  }
}

export default Res;