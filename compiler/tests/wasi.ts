// Runs the compiled module that the first argument names, as `wasmtime run`
// does. The module imports `fd_write` and `proc_exit` from WASI.

const STDOUT = 1;
const STDERR = 2;
const WASI_ESUCCESS = 0;
const WASI_EBADF = 8;

/** What `proc_exit` throws, so that `_start` stops where it calls it. */
class Exit {
  constructor(readonly code: number) {}
}

let memory: WebAssembly.Memory;

const wasi = {
  fd_write(fd: number, iovs: number, count: number, written: number): number {
    const out = fd === STDOUT
      ? Deno.stdout
      : fd === STDERR
      ? Deno.stderr
      : null;
    if (out === null) {
      return WASI_EBADF;
    }
    const view = new DataView(memory.buffer);
    let total = 0;
    for (let i = 0; i < count; i++) {
      const start = view.getUint32(iovs + 8 * i, true);
      const length = view.getUint32(iovs + 8 * i + 4, true);
      const bytes = new Uint8Array(memory.buffer, start, length);
      for (let done = 0; done < length;) {
        done += out.writeSync(bytes.subarray(done));
      }
      total += length;
    }
    view.setUint32(written, total, true);
    return WASI_ESUCCESS;
  },
  proc_exit(code: number): never {
    throw new Exit(code);
  },
};

const bytes = await Deno.readFile(Deno.args[0]);
const { instance } = await WebAssembly.instantiate(bytes, {
  wasi_snapshot_preview1: wasi,
});
memory = instance.exports.memory as WebAssembly.Memory;
try {
  (instance.exports._start as () => void)();
} catch (error) {
  if (error instanceof Exit) {
    Deno.exit(error.code);
  }
  throw error;
}
