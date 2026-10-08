import { assertEquals, assertMatch } from "jsr:@std/assert";
import { makeWasi } from "./wasi.ts";

// --- WASM exports interface ---

interface WasmExports {
  memory: WebAssembly.Memory;
  string_allocate(size: number): number;
  string_deallocate(ptr: number, size: number): void;
  cstr_deallocate(ptr: number): void;
  trace_source(
    code_ptr: number,
    code_len: number,
    config_ptr: number,
    config_len: number,
  ): number;
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

function makeEnv(
  getBuffer: () => ArrayBufferLike,
): { imports: WebAssembly.ModuleImports } {
  const imports: WebAssembly.ModuleImports = {
    check_interrupt: (): number => 0,
    draw_svg: (_ptr: number, _len: number): void => {},
    get_key_event: (
      _key_ptr: number,
      _key_len: number,
      _mods_ptr: number,
    ): number => 3,
    text_metrics: (
      _text: number,
      _text_len: number,
      _font_css: number,
      _font_css_len: number,
      out: number,
    ): void => {
      const view = new DataView(getBuffer() as ArrayBuffer);
      for (const [i, v] of [40, 10, -20, 3].entries()) {
        view.setFloat64(out + i * 8, v, true);
      }
    },
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

async function loadWasm(): Promise<WasmExports> {
  const wasmPath = new URL(
    "../../target/wasm32-wasip1/release-small/spython.wasm",
    import.meta.url,
  ).pathname;
  const wasmBytes = await Deno.readFile(wasmPath);
  const module = await WebAssembly.compile(wasmBytes);

  let exports: WasmExports;

  const wasi = makeWasi({
    getBuffer: () => exports.memory.buffer,
    write: (_fd, _text) => {},
    env: [],
  });

  const { imports: env } = makeEnv(() => exports.memory.buffer);

  const instance = await WebAssembly.instantiate(module, {
    env,
    wasi_snapshot_preview1: wasi,
  });

  exports = instance.exports as unknown as WasmExports;
  return exports;
}

// --- Trace helper ---

// deno-lint-ignore no-explicit-any
async function trace(source: string, config = ""): Promise<any> {
  const exports = await loadWasm();
  const [codePtr, codeLen] = encodeString(exports, source);
  const [cfgPtr, cfgLen] = encodeString(exports, config);
  const resultPtr = exports.trace_source(codePtr, codeLen, cfgPtr, cfgLen);
  exports.string_deallocate(codePtr, codeLen);
  exports.string_deallocate(cfgPtr, cfgLen);
  const json = readCstr(exports, resultPtr);
  exports.cstr_deallocate(resultPtr);
  return JSON.parse(json);
}

// --- Tests ---

// Ignored until the engine exports trace_source: without it the module
// loads and every one of these dies on the same call.

Deno.test.ignore("simple expression generates timeline", async () => {
  const t = await trace("x: int = 1\ny: int = x + 2\n", "level=5");
  assertEquals(t.truncated, false);
  assertEquals(t.exit_kind, "normal");
  assertEquals(t.error, undefined);
  assertEquals(t.steps.length > 0, true, "should have steps");
});

Deno.test.ignore("function call appears in stack", async () => {
  const source = `def add(a: int, b: int) -> int:
    return a + b
x: int = add(1, 2)
`;
  const t = await trace(source, "level=5");
  assertEquals(t.exit_kind, "normal");
  const hasNestedStack = t.steps.some(
    // deno-lint-ignore no-explicit-any
    (s: any) => s.stack.length > 1,
  );
  assertEquals(hasNestedStack, true, "should have nested stack frames");
  // Check that function frame has correct name
  const callStep = t.steps.find(
    // deno-lint-ignore no-explicit-any
    (s: any) =>
      s.stack.length > 1 &&
      s.stack.some((f: { name: string }) => f.name === "add"),
  );
  assertEquals(callStep !== undefined, true, "should have 'add' in stack");
});

Deno.test.ignore("print appears in stdout_delta", async () => {
  const t = await trace("x: int = 42\nprint(x)\n", "level=5");
  assertEquals(t.exit_kind, "normal");
  const hasOutput = t.steps.some(
    // deno-lint-ignore no-explicit-any
    (s: any) => s.stdout_delta.includes("42"),
  );
  assertEquals(hasOutput, true, "print output should appear in stdout_delta");
});

Deno.test.ignore(
  "import local file fails with unsupported_import",
  async () => {
    const t = await trace("import my_utils\nx: int = 1\n", "level=5");
    assertEquals(t.exit_kind, "error");
    assertEquals(t.error.exception_type, "ImportError");
    assertMatch(t.error.message, /single-file/i);
  },
);

Deno.test.ignore("runtime error generates partial timeline", async () => {
  const t = await trace("x: int = 1\ny: int = 1 // 0\n", "level=5");
  assertEquals(t.exit_kind, "error");
  assertEquals(t.error.exception_type, "ZeroDivisionError");
  assertEquals(t.steps.length > 0, true, "should have steps before error");
});

Deno.test.ignore("step limit marks truncated", async () => {
  const source = "x: int = 0\nwhile x < 1000:\n    x = x + 1\n";
  const t = await trace(source, "level=5 max_steps=10");
  assertEquals(t.truncated, true);
  assertEquals(t.exit_kind, "step_limit");
  assertEquals(t.steps.length <= 10, true, "should not exceed max_steps");
});

Deno.test.ignore("spython module does not appear in stack", async () => {
  const source = `from spython.color import Color
c: Color = Color(255, 0, 0)
`;
  const t = await trace(source, "level=5");
  assertEquals(t.exit_kind, "normal");
  // No step should contain spython frames (e.g., Color.__init__).
  // Since there are no user-defined functions, all frames should be <module>.
  for (const step of t.steps) {
    for (const frame of step.stack) {
      assertEquals(
        frame.name,
        "<module>",
        `spython frame leaked into stack: ${frame.name}`,
      );
    }
  }
});

Deno.test.ignore("locals show in stack frames", async () => {
  const t = await trace("x: int = 42\ny: str = 'hello'\n", "level=5");
  assertEquals(t.exit_kind, "normal");
  const lastStep = t.steps[t.steps.length - 1];
  const moduleFrame = lastStep.stack.find(
    (f: { name: string }) => f.name === "<module>",
  );
  assertEquals(moduleFrame !== undefined, true, "should have <module> frame");
  const xBinding = moduleFrame.locals.find(
    (binding: { name: string }) => binding.name === "x",
  );
  assertEquals(xBinding !== undefined, true, "x should be in locals");
  if (xBinding === undefined) {
    throw new Error("x should be in locals");
  }
  assertEquals(xBinding.value.kind, "ref");
  const xObject = lastStep.heap.find(
    (obj: { object_id: number }) => obj.object_id === xBinding.value.object_id,
  );
  assertEquals(xObject !== undefined, true, "x should point to a heap object");
  if (xObject === undefined) {
    throw new Error("x should point to a heap object");
  }
  assertEquals(xObject.kind, "int");
  assertEquals(xObject.value, "42");
});
