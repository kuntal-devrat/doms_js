/**
 * DOMS.js WebAssembly Host Loader & Wrapper
 * Provides an ergonomic JS API over the raw WebAssembly exports.
 */

export class DomsWasm {
  static async load(wasmUrl = './doms_wasm.wasm') {
    const response = await fetch(wasmUrl);
    const bytes = await response.arrayBuffer();
    const { instance } = await WebAssembly.instantiate(bytes, {
      env: {
        // Any imported host syscalls if needed
      }
    });
    return new DomsWasm(instance);
  }

  constructor(instance) {
    this.exports = instance.exports;
    this.memory = instance.exports.memory;
  }

  /**
   * Returns the DOMS.js WebAssembly version string.
   */
  version() {
    const ptr = this.exports.doms_wasm_version();
    return this.readCString(ptr);
  }

  /**
   * Evaluate JavaScript code in the global isolate.
   */
  eval(code) {
    return this.isolateEval(0, code);
  }

  /**
   * Create an independent isolate sandbox.
   */
  createIsolate() {
    return this.exports.doms_wasm_create_isolate();
  }

  /**
   * Destroy an isolate and free its resources.
   */
  destroyIsolate(isolateId) {
    this.exports.doms_wasm_destroy_isolate(isolateId);
  }

  /**
   * Evaluate JavaScript code inside a specific isolate sandbox.
   */
  isolateEval(isolateId, code) {
    const encoder = new TextEncoder();
    const encoded = encoder.encode(code);
    const ptr = this.exports.doms_wasm_alloc(encoded.length);

    new Uint8Array(this.memory.buffer).set(encoded, ptr);

    const resultPtr = this.exports.doms_wasm_isolate_eval(isolateId, ptr, encoded.length);
    const resultStr = this.readCString(resultPtr);

    this.exports.doms_wasm_free(ptr, encoded.length);
    this.exports.doms_wasm_free_string(resultPtr);

    return resultStr;
  }

  readCString(ptr) {
    if (!ptr) return '';
    const buffer = new Uint8Array(this.memory.buffer);
    let end = ptr;
    while (buffer[end] !== 0) {
      end++;
    }
    const decoder = new TextDecoder('utf-8');
    return decoder.decode(buffer.subarray(ptr, end));
  }
}
