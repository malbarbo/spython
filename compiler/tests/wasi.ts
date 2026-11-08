// Runs the compiled module that the first argument names, as `wasmtime run`
// does. The module imports `fd_write` and `proc_exit` from WASI.

const STDOUT = 1;
const STDERR = 2;
const WASI_ESUCCESS = 0;
const WASI_EBADF = 8;
const WASI_EIO = 29;
const WASI_ENOSPC = 51;
const WASI_FILETYPE_CHARACTER_DEVICE = 2;

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
      try {
        for (let done = 0; done < length;) {
          done += out.writeSync(bytes.subarray(done));
        }
      } catch (error) {
        return wasiError(error);
      }
      total += length;
    }
    view.setUint32(written, total, true);
    return WASI_ESUCCESS;
  },
  fd_fdstat_get(fd: number, stat: number): number {
    const out = fd === STDOUT
      ? Deno.stdout
      : fd === STDERR
      ? Deno.stderr
      : null;
    if (out === null) {
      return WASI_EBADF;
    }
    // wasmtime gives a terminal as a character device, and anything else
    // as a file of unknown type, which is 0.
    new Uint8Array(memory.buffer, stat, 24).fill(0);
    if (out.isTerminal()) {
      new DataView(memory.buffer).setUint8(
        stat,
        WASI_FILETYPE_CHARACTER_DEVICE,
      );
    }
    return WASI_ESUCCESS;
  },
  proc_exit(code: number): never {
    throw new Exit(code);
  },
};

/**
 * The error of WASI that wasmtime gives for `error`, of the ones the tests
 * make. It gives `ENOSPC` for a full device, and `EIO` for a pipe that the
 * reader closed.
 */
function wasiError(error: unknown): number {
  if (error instanceof Error && "code" in error && error.code === "ENOSPC") {
    return WASI_ENOSPC;
  }
  return WASI_EIO;
}

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
