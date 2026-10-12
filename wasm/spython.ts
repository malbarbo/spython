// Deno CLI wrapper around the spython WASM binary.
// Provides the same stdin/stdout interface as the native spython binary so
// the Rust integration tests in cli/tests/cli.rs can run both backends.
//
// Current scope: `run`, `check` and `test`, each `[--level N] FILE.py`, and
// `--version`. `repl` and `format` remain native-only for now.
//
// Each command goes through the export of the same name, which the WASM binary
// only has under the `wasm-backend` feature. The page's own entry point,
// `repl_new`, is not what is driven here: it checks the file, runs it and runs
// its doctests, all three at once, which is none of these commands.

import { makeWasi } from "./tests/wasi.ts";

interface WasmExports {
  memory: WebAssembly.Memory;
  string_allocate(size: number): number;
  string_deallocate(ptr: number, size: number): void;
  cstr_deallocate(ptr: number): void;
  version(): number;
  check_source(
    code_ptr: number,
    code_len: number,
    config_ptr: number,
    config_len: number,
  ): number;
  run_source(
    code_ptr: number,
    code_len: number,
    filename_ptr: number,
    filename_len: number,
    config_ptr: number,
    config_len: number,
  ): number;
  test_source(
    code_ptr: number,
    code_len: number,
    filename_ptr: number,
    filename_len: number,
    config_ptr: number,
    config_len: number,
  ): number;
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function encodeString(exports: WasmExports, str: string): [number, number] {
  const encoded = encoder.encode(str);
  const ptr = exports.string_allocate(encoded.length);
  new Uint8Array(exports.memory.buffer, ptr, encoded.length).set(encoded);
  return [ptr, encoded.length];
}

function readCstr(exports: WasmExports, ptr: number): string {
  const buffer = new Uint8Array(exports.memory.buffer);
  let end = ptr;
  while (buffer[end] !== 0) end++;
  return decoder.decode(buffer.slice(ptr, end));
}

function writeOut(fd: number, text: string) {
  const bytes = encoder.encode(text);
  const out = fd === 2 ? Deno.stderr : Deno.stdout;
  let written = 0;
  while (written < bytes.length) {
    written += out.writeSync(bytes.subarray(written));
  }
}

function wasmPath(): string {
  return new URL(
    "../target/wasm32-wasip1/release-small/spython.wasm",
    import.meta.url,
  ).pathname;
}

async function loadWasm(args: string[]): Promise<WasmExports> {
  const bytes = await Deno.readFile(wasmPath());
  const module = await WebAssembly.compile(bytes);
  let exports: WasmExports;

  const wasi = makeWasi({
    getBuffer: () => exports.memory.buffer,
    write: (fd, text) => writeOut(fd, text),
    args,
    env: [],
  });

  // Stub env imports. None of these are exercised by these commands today,
  // but the WASM module still declares them as required imports.
  const env: WebAssembly.ModuleImports = {
    check_interrupt: (): number => 0,
    draw_svg: (): void => {},
  };

  const instance = await WebAssembly.instantiate(module, {
    env,
    wasi_snapshot_preview1: wasi,
  });

  exports = instance.exports as unknown as WasmExports;
  return exports;
}

// `run` and `test` both name the file they were given, which is what a
// traceback and a doctest header show; `check` runs nothing and needs no name.
async function runFile(
  kind: "run" | "test",
  file: string,
  level: number,
): Promise<number> {
  const source = await Deno.readTextFile(file);
  const exports = await loadWasm(["spython", kind, file]);
  const [codePtr, codeLen] = encodeString(exports, source);
  const [nmPtr, nmLen] = encodeString(exports, file);
  const [cfgPtr, cfgLen] = encodeString(exports, `level=${level}`);
  const call = kind === "run" ? exports.run_source : exports.test_source;
  const status = call(codePtr, codeLen, nmPtr, nmLen, cfgPtr, cfgLen);
  exports.string_deallocate(codePtr, codeLen);
  exports.string_deallocate(nmPtr, nmLen);
  exports.string_deallocate(cfgPtr, cfgLen);
  return status === 0 ? 0 : 1;
}

async function checkFile(file: string, level: number): Promise<number> {
  const source = await Deno.readTextFile(file);
  const exports = await loadWasm(["spython", "check", file]);
  const [codePtr, codeLen] = encodeString(exports, source);
  const [cfgPtr, cfgLen] = encodeString(exports, `level=${level}`);
  const status = exports.check_source(codePtr, codeLen, cfgPtr, cfgLen);
  exports.string_deallocate(codePtr, codeLen);
  exports.string_deallocate(cfgPtr, cfgLen);
  return status === 0 ? 0 : 1;
}

async function runVersion(): Promise<number> {
  const exports = await loadWasm(["spython", "--version"]);
  const ptr = exports.version();
  const ver = readCstr(exports, ptr);
  exports.cstr_deallocate(ptr);
  writeOut(1, `${ver}\n`);
  return 0;
}

interface ParsedFile {
  kind: "run" | "check" | "test";
  file: string;
  level: number;
}

interface ParsedVersion {
  kind: "version";
}

interface ParsedError {
  kind: "error";
  message: string;
}

type Parsed = ParsedFile | ParsedVersion | ParsedError;

function parseArgs(argv: string[]): Parsed {
  if (argv.length === 0) {
    return { kind: "error", message: "error: missing subcommand\n" };
  }
  const first = argv[0];
  if (first === "--version" || first === "-V") {
    return { kind: "version" };
  }
  if (first !== "run" && first !== "check" && first !== "test") {
    return {
      kind: "error",
      message: `error: unsupported subcommand: ${first}\n`,
    };
  }
  let level = 5;
  let file: string | null = null;
  let i = 1;
  while (i < argv.length) {
    const a = argv[i];
    if (a === "--level" || a === "-l") {
      const v = argv[i + 1];
      if (v === undefined) {
        return { kind: "error", message: "error: --level requires a value\n" };
      }
      const n = Number.parseInt(v, 10);
      if (Number.isNaN(n) || n < 0 || n > 5) {
        return {
          kind: "error",
          message: `error: invalid level: ${v}\n`,
        };
      }
      level = n;
      i += 2;
      continue;
    }
    if (a.startsWith("-")) {
      return { kind: "error", message: `error: unknown flag: ${a}\n` };
    }
    if (file === null) {
      file = a;
      i++;
      continue;
    }
    return { kind: "error", message: `error: unexpected argument: ${a}\n` };
  }
  if (file === null) {
    return { kind: "error", message: `error: \`${first}\` requires a FILE\n` };
  }
  return { kind: first, file, level };
}

async function main(): Promise<number> {
  const parsed = parseArgs(Deno.args);
  switch (parsed.kind) {
    case "version":
      return await runVersion();
    case "check":
      return await checkFile(parsed.file, parsed.level);
    case "run":
    case "test":
      return await runFile(parsed.kind, parsed.file, parsed.level);
    case "error":
      writeOut(2, parsed.message);
      return 1;
  }
}

Deno.exit(await main());
