import {
  assertEquals,
  assertMatch,
  assertStringIncludes,
} from "jsr:@std/assert";
import { FakeRoom, type Step } from "./room.ts";
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
  repl_ready(
    repl: number,
    ptr: number,
    len: number,
    cursor: number,
  ): number;
  string_allocate(size: number): number;
  string_deallocate(ptr: number, size: number): void;
  format(ptr: number, len: number): number;
  cstr_deallocate(ptr: number): void;
  version(): number;
  font_face(i: number): number;
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
  // The frames of the worlds, which go to the room of a session.
  room: FakeRoom;
}

// `steps` is what the player does in a world.
async function loadWasm(steps: Step[] = []): Promise<WasmContext> {
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

  // The engine reads the time through WASI, and a wait moves it on, so a
  // world runs its ticks without burning wall-clock time.
  const room = new FakeRoom(steps);

  const wasi = makeWasi({
    getBuffer: () => exports.memory.buffer,
    write: (fd, text) => {
      if (fd === 2) stderr.push(text);
      else stdout.push(text);
    },
    env: ["RUST_BACKTRACE=1"],
    now: () => room.ms,
    advance: (until) => room.advance(until),
    session: room.pipe,
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

  return { exports, stdout, stderr, svgs, interruptCtrl, room };
}

// --- Helper ---

interface ReplContext extends WasmContext {
  repl: number;
}

async function newRepl(
  source = "",
  options: { level?: number; steps?: Step[] } = {},
): Promise<ReplContext> {
  const ctx = await loadWasm(options.steps);
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
): {
  result: number;
  stdout: string;
  stderr: string;
  svgs: string[];
  frames: Uint8Array[];
} {
  ctx.stdout.length = 0;
  ctx.stderr.length = 0;
  ctx.svgs.length = 0;
  ctx.room.frames.length = 0;
  const [ptr, len] = encodeString(ctx.exports, input);
  const result = ctx.exports.repl_run(ctx.repl, ptr, len);
  ctx.exports.string_deallocate(ptr, len);
  return {
    result,
    stdout: ctx.stdout.join(""),
    stderr: ctx.stderr.join(""),
    svgs: [...ctx.svgs],
    frames: [...ctx.room.frames],
  };
}

function destroy(ctx: ReplContext): void {
  ctx.exports.repl_destroy(ctx.repl);
}

// --- Constants ---

const REPL_OK = 0;
const REPL_ERROR = 1;
const REPL_QUIT = 2;

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

Deno.test("a bitmap from a data URI takes the size of its image", async () => {
  const ctx = await newRepl("", { level: 5 });
  // The signature and the start of the IHDR chunk of a PNG of 3 by 2.
  const png = "\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR\0\0\0\x03\0\0\0\x02";
  const uri = `data:image/png;base64,${btoa(png)}`;
  run(ctx, "from spython.image import bitmap_data_uri, width, height");
  run(ctx, `b = bitmap_data_uri("${uri}")`);
  const r = run(ctx, "print(width(b), height(b))");
  assertEquals(r.stdout, "3.0 2.0\n");
  destroy(ctx);
});

Deno.test("an image at the prompt goes to the page as an svg", async () => {
  const ctx = await newRepl("", { level: 5 });
  run(ctx, "from spython.image import beside, circle, fill, red, text");
  const r = run(ctx, 'beside(circle(5, fill(red)), text("a < b & c"))');
  assertEquals(r.result, REPL_OK);
  assertEquals(r.svgs.length, 1);
  const svg = r.svgs[0];
  assertEquals(svg.startsWith("<svg "), true, svg);
  // The text goes in as the outlines of its glyphs, so nothing in it needs
  // an escape.
  assertEquals(svg.includes("a < b"), false, svg);
  destroy(ctx);
});

Deno.test("the prompt draws an image with one more column and row", async () => {
  const ctx = await newRepl("", { level: 5 });
  run(ctx, "from spython.image import rectangle, stroke, black");
  const r = run(ctx, "rectangle(40, 20, stroke(black))");
  assertEquals(r.result, REPL_OK);
  assertEquals(r.svgs[0].includes('width="41" height="21"'), true, r.svgs[0]);
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

Deno.test(":help lists the commands, on the page too", async () => {
  const ctx = await newRepl("", { level: 5 });
  const r = run(ctx, ":help");
  assertEquals(r.result, REPL_OK);
  assertStringIncludes(r.stdout, ":type <expression>");
  assertStringIncludes(r.stdout, ":quit");
  // :theme is the terminal's, and is not registered here.
  assertEquals(r.stdout.includes(":theme"), false);
  destroy(ctx);
});

Deno.test(":quit asks the page to end the session", async () => {
  const ctx = await newRepl("", { level: 5 });
  assertEquals(run(ctx, ":quit").result, REPL_QUIT);
  destroy(ctx);
});

Deno.test("an unknown command is refused, not run as Python", async () => {
  const ctx = await newRepl("", { level: 5 });
  const r = run(ctx, ":nope");
  assertEquals(r.result, REPL_ERROR);
  assertStringIncludes(r.stderr, "Unknown command :nope");
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

// --- Ready ---

Deno.test("repl_ready says what the input still needs", async () => {
  const ctx = await newRepl("", { level: 5 });
  // What the answers are is tested in engine/src/parser.rs; what this crosses
  // is the export: text in, an i32 out, with no session asked.
  const cases: [string, number][] = [
    // Finished: run it.
    ["1 + 2", -1],
    // A block asks for a body, one level in, and a bracket for a level of
    // its own.
    ["def f() -> None:\n    print(", 8],
    // A command is asked about what it carries.
    [":time [1,", 4],
  ];
  for (const [input, expected] of cases) {
    const [ptr, len] = encodeString(ctx.exports, input);
    const result = ctx.exports.repl_ready(ctx.repl, ptr, len, len);
    ctx.exports.string_deallocate(ptr, len);
    assertEquals(result, expected, JSON.stringify(input));
  }
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

Deno.test("a file that calls exit() opens an empty session", async () => {
  const ctx = await newRepl("x = 1\nexit('bye')\nx = 2\n", { level: 5 });
  assertEquals(ctx.repl !== 0, true, "repl_new should return non-null");
  assertEquals(ctx.stderr.join(""), "bye\n");
  assertEquals(run(ctx, "x").result, REPL_ERROR);
  assertEquals(run(ctx, "y = 1").result, REPL_OK);
  destroy(ctx);
});

Deno.test("a file that raises opens an empty session", async () => {
  const ctx = await newRepl("x = 1\nint('z')\ny = 2\n", { level: 5 });
  assertEquals(ctx.repl !== 0, true, "repl_new should return non-null");
  assertEquals(run(ctx, "x").result, REPL_ERROR);
  assertEquals(run(ctx, "y").result, REPL_ERROR);
  destroy(ctx);
});

Deno.test("the traceback of a file shows the line that raised", async () => {
  const ctx = await newRepl("x = 1\nint('z')\n", { level: 5 });
  assertStringIncludes(
    ctx.stderr.join(""),
    `  File "user.py", line 2, in <module>\n    int('z')\n`,
  );
  destroy(ctx);
});

Deno.test("the traceback of an input shows the line that raised", async () => {
  const ctx = await newRepl("", { level: 5 });
  run(ctx, "x = 1");
  assertStringIncludes(
    run(ctx, "int('z')").stderr,
    `  File "<python-input-1>", line 1, in <module>\n    int('z')\n`,
  );
  destroy(ctx);
});

const exitAfterADoctest = (status: number) =>
  `def one() -> int:
    """
    >>> one()
    1
    """
    return 1

exit(${status})
`;

for (const status of [0, 1]) {
  Deno.test(`a file that calls exit(${status}) skips its doctests`, async () => {
    const ctx = await newRepl(exitAfterADoctest(status), { level: 5 });
    assertEquals(ctx.stderr.join(""), "");
    destroy(ctx);
  });
}

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

Deno.test("font_face gives the faces of the frames", async () => {
  const { exports } = await loadWasm();
  const families = new Set<string>();
  let i = 0;
  for (let ptr; (ptr = exports.font_face(i)) !== 0; i++) {
    const [bytes, bytesLen, family, familyLen, weight] = new Uint32Array(
      exports.memory.buffer,
      ptr,
      6,
    );
    // A TrueType file starts with version 1.0.
    assertEquals(
      [...new Uint8Array(exports.memory.buffer, bytes, 4)],
      [0, 1, 0, 0],
    );
    assertEquals(bytesLen > 0, true);
    assertEquals(weight === 400 || weight === 700, true);
    families.add(
      decoder.decode(new Uint8Array(exports.memory.buffer, family, familyLen)),
    );
  }
  // Four faces a family.
  assertEquals(i, families.size * 4);
  assertEquals(i > 0, true);
});

// --- Worlds ---

const WORLD = `from spython import Image, World, circle, fill, red


def draw(n: int) -> Image:
    return circle(10 + n, fill(red))


def tick(n: int) -> int:
    return n + 1


def done(n: int) -> bool:
    return n == 3


def key(n: int, k: str) -> int:
    print(k)
    return n + 1
`;

Deno.test("a World ticks until stop_when, in a session", async () => {
  const ctx = await newRepl(WORLD, { level: 5 });
  const r = run(ctx, "World(0, draw).on_tick(tick).stop_when(done).run()");
  assertEquals(r.stderr, "");
  assertEquals(r.result, REPL_OK);
  // The first state and one per tick, the last one drawn by the close.
  assertEquals(r.frames.length, 4);
  assertEquals(r.svgs.length, 0, "a world draws no svg");
  destroy(ctx);
});

Deno.test("a World takes the keys of the room", async () => {
  const ctx = await newRepl(WORLD, {
    level: 5,
    steps: [
      { type: "press", key: "a" },
      { type: "down", key: "ArrowLeft" },
      { type: "up", key: "Enter" },
    ],
  });
  const r = run(
    ctx,
    "World(0, draw).on_key_down(key).on_key_up(key).stop_when(done).run()",
  );
  assertEquals(r.stderr, "");
  // A press alone stands for a down too.
  assertEquals(r.stdout, "a\nArrowLeft\nEnter\n");
  destroy(ctx);
});

Deno.test("a close ends a World", async () => {
  const ctx = await newRepl(WORLD, {
    level: 5,
    steps: [{ type: "down", key: "a" }, "close"],
  });
  const r = run(ctx, "World(0, draw).on_key_down(key).run()");
  assertEquals(r.stderr, "");
  assertEquals(r.result, REPL_OK);
  assertEquals(r.stdout, "a\n");
  destroy(ctx);
});

Deno.test("a close ends animate", async () => {
  const ctx = await newRepl(WORLD, { level: 5, steps: ["close"] });
  const r = run(ctx, "from spython import animate\nanimate(draw)");
  assertEquals(r.stderr, "");
  assertEquals(r.frames.length >= 1, true);
  destroy(ctx);
});

Deno.test("a stop ends animate", async () => {
  const ctx = await newRepl(WORLD, { level: 5 });
  // The stop comes after a few frames of the room.
  const advance = ctx.room.advance.bind(ctx.room);
  ctx.room.advance = (until) => {
    advance(until);
    if (ctx.room.frames.length === 3) ctx.interruptCtrl.set(true);
  };
  const r = run(ctx, "from spython import animate\nanimate(draw)");
  assertEquals(r.result, REPL_ERROR);
  assertEquals(r.stderr.includes("KeyboardInterrupt"), true, r.stderr);
  destroy(ctx);
});

// A Stop on the page stays on until the next run, so the `finally` of the
// World never closes its display, and the run has to.
Deno.test("a World after a stop opens a session of its own", async () => {
  const ctx = await newRepl(WORLD, { level: 5 });
  const advance = ctx.room.advance.bind(ctx.room);
  let stopped = false;
  ctx.room.advance = (until) => {
    advance(until);
    if (!stopped && ctx.room.frames.length === 2) {
      stopped = true;
      ctx.interruptCtrl.set(true);
    }
  };
  const first = run(ctx, "World(0, draw).on_tick(tick).run()");
  assertEquals(first.result, REPL_ERROR);
  ctx.interruptCtrl.set(false);
  const second = run(ctx, "World(0, draw).on_tick(tick).stop_when(done).run()");
  assertEquals(second.stderr, "");
  assertEquals(second.frames.length, 4);
  destroy(ctx);
});
