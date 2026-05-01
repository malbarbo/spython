import { assertEquals, assertMatch } from "jsr:@std/assert";
import { makeWasi } from "./wasi.ts";

// --- WASM exports interface ---

interface WasmExports {
  memory: WebAssembly.Memory;
  repl_new(
    code_ptr: number,
    code_len: number,
    config_ptr: number,
    config_len: number,
  ): number;
  repl_run(repl: number, ptr: number, len: number): number;
  repl_destroy(repl: number): void;
  repl_complete(
    repl: number,
    ptr: number,
    len: number,
    cursor_pos: number,
  ): number;
  string_allocate(size: number): number;
  string_deallocate(ptr: number, size: number): void;
  format(ptr: number, len: number): number;
  cstr_deallocate(ptr: number): void;
  version(): number;
}

// --- Memory helpers ---

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

// --- Mock env ---

interface InterruptControl {
  set: (on: boolean) => void;
}

function makeEnv(
  getBuffer: () => ArrayBufferLike,
  svgs: string[],
  interruptCtrl: InterruptControl,
): { imports: WebAssembly.ModuleImports } {
  let interrupted = false;

  interruptCtrl.set = (on: boolean) => {
    interrupted = on;
  };

  const imports: WebAssembly.ModuleImports = {
    check_interrupt: (): number => {
      return interrupted ? 1 : 0;
    },
    draw_svg: (ptr: number, len: number): void => {
      const b = new Uint8Array(getBuffer() as ArrayBuffer);
      svgs.push(decoder.decode(b.slice(ptr, ptr + len)));
    },
    get_key_event: (
      _key_ptr: number,
      _key_len: number,
      _mods_ptr: number,
    ): number => {
      return 3; // KEYNONE (no event)
    },
    text_width: (
      _text: number,
      _text_len: number,
      _font_css: number,
      _font_css_len: number,
    ): number => 10,
    text_height: (
      _text: number,
      _text_len: number,
      _font_css: number,
      _font_css_len: number,
    ): number => 16,
    text_x_offset: (
      _text: number,
      _text_len: number,
      _font_css: number,
      _font_css_len: number,
    ): number => 0,
    text_y_offset: (
      _text: number,
      _text_len: number,
      _font_css: number,
      _font_css_len: number,
    ): number => 0,
  };
  return { imports };
}

// --- WASM loader ---

interface WasmContext {
  exports: WasmExports;
  stdout: string[];
  stderr: string[];
  svgs: string[];
  interruptCtrl: InterruptControl;
}

async function loadWasm(): Promise<WasmContext> {
  const stdout: string[] = [];
  const stderr: string[] = [];
  const svgs: string[] = [];

  const wasmPath = new URL(
    "../../target/wasm32-wasip1/release-small/spython.wasm",
    import.meta.url,
  ).pathname;
  const wasmBytes = await Deno.readFile(wasmPath);
  const module = await WebAssembly.compile(wasmBytes);

  let exports: WasmExports;

  const wasi = makeWasi({
    getBuffer: () => exports.memory.buffer,
    write: (fd, text) => {
      if (fd === 2) stderr.push(text);
      else stdout.push(text);
    },
    env: ["RUST_BACKTRACE=1"],
  });

  const interruptCtrl: InterruptControl = {
    set: () => {},
  };
  const { imports: env } = makeEnv(
    () => exports.memory.buffer,
    svgs,
    interruptCtrl,
  );

  const instance = await WebAssembly.instantiate(module, {
    env,
    wasi_snapshot_preview1: wasi,
  });

  exports = instance.exports as unknown as WasmExports;

  return { exports, stdout, stderr, svgs, interruptCtrl };
}

// --- Helper ---

interface ReplContext extends WasmContext {
  repl: number;
}

async function newRepl(
  source = "",
  options: { level?: number } = {},
): Promise<ReplContext> {
  const ctx = await loadWasm();
  const [codePtr, codeLen] = encodeString(ctx.exports, source);
  const config = options.level !== undefined ? `level=${options.level}` : "";
  const [cfgPtr, cfgLen] = encodeString(ctx.exports, config);
  const repl = ctx.exports.repl_new(codePtr, codeLen, cfgPtr, cfgLen);
  ctx.exports.string_deallocate(codePtr, codeLen);
  ctx.exports.string_deallocate(cfgPtr, cfgLen);
  return { ...ctx, repl };
}

function run(
  ctx: ReplContext,
  input: string,
): { result: number; stdout: string; stderr: string; svgs: string[] } {
  ctx.stdout.length = 0;
  ctx.stderr.length = 0;
  ctx.svgs.length = 0;
  const [ptr, len] = encodeString(ctx.exports, input);
  const result = ctx.exports.repl_run(ctx.repl, ptr, len);
  ctx.exports.string_deallocate(ptr, len);
  return {
    result,
    stdout: ctx.stdout.join(""),
    stderr: ctx.stderr.join(""),
    svgs: [...ctx.svgs],
  };
}

function destroy(ctx: ReplContext): void {
  ctx.exports.repl_destroy(ctx.repl);
}

// --- Constants ---

const REPL_OK = 0;
const REPL_ERROR = 1;

// --- Tests ---

Deno.test("version returns non-empty string", async () => {
  const ctx = await loadWasm();
  const ptr = ctx.exports.version();
  const ver = readCstr(ctx.exports, ptr);
  ctx.exports.cstr_deallocate(ptr);
  assertEquals(ver.length > 0, true, "version should be non-empty");
  assertEquals(
    ver.startsWith("spython"),
    true,
    `expected 'spython' prefix, got: ${ver}`,
  );
});

Deno.test("repl smoke test", async () => {
  const ctx = await newRepl("", { level: 5 });
  const r = run(ctx, "1 + 2");
  assertEquals(r.result, REPL_OK);
  assertEquals(r.stdout, "3\n");
  destroy(ctx);
});

Deno.test("multiple runs", async () => {
  const ctx = await newRepl("", { level: 5 });
  const r1 = run(ctx, "1 + 2");
  assertEquals(r1.stdout, "3\n");
  const r2 = run(ctx, "10 + 20");
  assertEquals(r2.stdout, "30\n");
  destroy(ctx);
});

Deno.test("error output contains ansi codes", async () => {
  const ctx = await newRepl("", { level: 5 });
  const r = run(ctx, "unknown_variable");
  assertEquals(r.result, REPL_ERROR);
  assertMatch(r.stderr, /\x1b\[/, "expected ANSI codes in error output");
  destroy(ctx);
});

Deno.test("load with type errors returns null", async () => {
  const ctx = await loadWasm();
  // Missing type annotations — should fail at level 0
  const source = "def f(x):\n    return x + 1";
  const [codePtr, codeLen] = encodeString(ctx.exports, source);
  const [cfgPtr, cfgLen] = encodeString(ctx.exports, "");
  const repl = ctx.exports.repl_new(codePtr, codeLen, cfgPtr, cfgLen);
  ctx.exports.string_deallocate(codePtr, codeLen);
  ctx.exports.string_deallocate(cfgPtr, cfgLen);
  assertEquals(
    repl,
    0,
    "repl_new should return null for code with type errors",
  );
});

Deno.test("load without errors", async () => {
  const ctx = await newRepl(
    "def f(x: int) -> int:\n    return x + 1",
    { level: 0 },
  );
  assertEquals(ctx.repl !== 0, true, "repl_new should return non-null");
  const r = run(ctx, "f(10)");
  assertEquals(r.result, REPL_OK);
  assertEquals(r.stdout, "11\n");
  destroy(ctx);
});

Deno.test("format code", async () => {
  const ctx = await loadWasm();
  const input = "def   f( x:int )->int:\n    return x+1\n";
  const [ptr, len] = encodeString(ctx.exports, input);
  const resultPtr = ctx.exports.format(ptr, len);
  ctx.exports.string_deallocate(ptr, len);
  assertEquals(resultPtr !== 0, true, "format should return non-null");
  const formatted = readCstr(ctx.exports, resultPtr);
  ctx.exports.cstr_deallocate(resultPtr);
  assertEquals(
    formatted.includes("def f"),
    true,
    "should contain formatted function",
  );
});

Deno.test("format invalid code returns null", async () => {
  const ctx = await loadWasm();
  const input = "def (";
  const [ptr, len] = encodeString(ctx.exports, input);
  const resultPtr = ctx.exports.format(ptr, len);
  ctx.exports.string_deallocate(ptr, len);
  assertEquals(resultPtr, 0, "format should return null for invalid code");
});

// --- Type checking ---

Deno.test("missing annotation is rejected at level 0", async () => {
  const ctx = await newRepl("", { level: 0 });
  const r = run(ctx, "def g(x):\n    return x");
  assertEquals(r.result, REPL_ERROR);
  destroy(ctx);
});

Deno.test("annotated function is accepted at level 0", async () => {
  const ctx = await newRepl("", { level: 0 });
  const r = run(ctx, "def g(x: int) -> int:\n    return x");
  assertEquals(r.result, REPL_OK);
  destroy(ctx);
});

// --- Level config ---

Deno.test("level 0 rejects if/else", async () => {
  const ctx = await newRepl("", { level: 0 });
  const r = run(
    ctx,
    "def g(x: int) -> int:\n    if x > 0:\n        return x\n    else:\n        return -x",
  );
  assertEquals(r.result, REPL_ERROR);
  destroy(ctx);
});

Deno.test("level 1 allows if/else", async () => {
  const ctx = await newRepl("", { level: 1 });
  const r = run(
    ctx,
    "def g(x: int) -> int:\n    if x > 0:\n        return x\n    else:\n        return -x",
  );
  assertEquals(r.result, REPL_OK);
  destroy(ctx);
});

// --- :type command ---

Deno.test(":type command returns type", async () => {
  const ctx = await newRepl("", { level: 5 });
  run(ctx, "x = 42");
  const r = run(ctx, ":type x");
  assertEquals(r.result, REPL_OK);
  assertEquals(r.stdout.trim(), "int");
  destroy(ctx);
});

// --- Completion ---

Deno.test("repl_complete returns candidates", async () => {
  const ctx = await newRepl("", { level: 5 });
  run(ctx, "def my_func() -> None:\n    pass");
  const input = "my_f";
  const [ptr, len] = encodeString(ctx.exports, input);
  const resultPtr = ctx.exports.repl_complete(ctx.repl, ptr, len, len);
  ctx.exports.string_deallocate(ptr, len);
  assertEquals(resultPtr !== 0, true, "repl_complete should return non-null");
  const result = readCstr(ctx.exports, resultPtr);
  ctx.exports.cstr_deallocate(resultPtr);
  assertEquals(result.startsWith("c "), true, "should start with 'c '");
  assertEquals(
    result.includes("my_func"),
    true,
    `should include my_func, got: ${result}`,
  );
  destroy(ctx);
});

Deno.test("repl_complete returns empty candidates for no match", async () => {
  const ctx = await newRepl("", { level: 5 });
  const input = "zzz_no_match";
  const [ptr, len] = encodeString(ctx.exports, input);
  const resultPtr = ctx.exports.repl_complete(ctx.repl, ptr, len, len);
  ctx.exports.string_deallocate(ptr, len);
  assertEquals(resultPtr !== 0, true, "repl_complete should return non-null");
  const result = readCstr(ctx.exports, resultPtr);
  ctx.exports.cstr_deallocate(resultPtr);
  assertEquals(result, "c 0", "should return empty completion list");
  destroy(ctx);
});

Deno.test("repl_complete on empty line returns completions", async () => {
  const ctx = await newRepl("", { level: 5 });
  const input = "";
  const [ptr, len] = encodeString(ctx.exports, input);
  const resultPtr = ctx.exports.repl_complete(ctx.repl, ptr, len, 0);
  ctx.exports.string_deallocate(ptr, len);
  assertEquals(resultPtr !== 0, true, "repl_complete should return non-null");
  const result = readCstr(ctx.exports, resultPtr);
  ctx.exports.cstr_deallocate(resultPtr);
  assertEquals(result.startsWith("c "), true, "should start with 'c '");
  assertEquals(
    result.includes("print"),
    true,
    `should include print, got: ${result.substring(0, 100)}...`,
  );
  destroy(ctx);
});

Deno.test("repl_complete indents under-indented line", async () => {
  const ctx = await newRepl("", { level: 5 });
  run(ctx, "def f() -> None:\n    pass");
  const input = "def g() -> None:\n";
  const [ptr, len] = encodeString(ctx.exports, input);
  const resultPtr = ctx.exports.repl_complete(
    ctx.repl,
    ptr,
    len,
    input.length,
  );
  ctx.exports.string_deallocate(ptr, len);
  assertEquals(resultPtr !== 0, true, "repl_complete should return non-null");
  const result = readCstr(ctx.exports, resultPtr);
  ctx.exports.cstr_deallocate(resultPtr);
  assertEquals(result, "i     ", "should indent by 4 spaces");
  destroy(ctx);
});

// --- Interrupt ---

Deno.test("interrupt stops infinite loop", async () => {
  const ctx = await newRepl("", { level: 3 });
  ctx.interruptCtrl.set(true);
  const r = run(ctx, "while True:\n    pass");
  assertEquals(r.result, REPL_ERROR);
  assertMatch(r.stderr, /KeyboardInterrupt/);
  destroy(ctx);
});

// --- Doctest ---

Deno.test("load with doctests runs them", async () => {
  const source = `def add(a: int, b: int) -> int:
    """
    >>> add(1, 2)
    3
    >>> add(0, 0)
    0
    """
    return a + b
`;
  const ctx = await newRepl(source, { level: 0 });
  assertEquals(ctx.repl !== 0, true, "repl_new should return non-null");
  const stderr = ctx.stderr.join("");
  assertEquals(
    stderr.includes("2 tests"),
    true,
    `expected test output in stderr, got: ${stderr}`,
  );
  destroy(ctx);
});

Deno.test("panic handler intercepts Rust panics", async () => {
  const ctx = await newRepl();
  try {
    // string_deallocate(null, _) trips `assert!(!ptr.is_null())` in wasm/src/lib.rs,
    // which panics. The handler should format the message and write it to stderr
    // before the module aborts.
    ctx.exports.string_deallocate(0, 10);
  } catch (_e) {
    // Expected: WASM execution aborts after the panic.
  }
  const stderr = ctx.stderr.join("");
  assertEquals(
    stderr.includes("spython: internal error"),
    true,
    `expected formatted panic message in stderr, got: ${stderr}`,
  );
});
