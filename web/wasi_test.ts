import { assertEquals } from "jsr:@std/assert";
import { makeWasi } from "./wasi.ts";

function makeBuffer(): ArrayBuffer {
    return new ArrayBuffer(4096);
}

function noop() {}
function noopStr(): string {
    return "";
}

Deno.test("environ_sizes_get counts UTF-8 bytes", () => {
    const buffer = makeBuffer();
    // "café" is 5 UTF-8 bytes (c=1, a=1, f=1, é=2)
    const wasi = makeWasi({
        getBuffer: () => buffer,
        write: noop,
        svg: noop,
        readStdin: noopStr,
        env: ["LANG=café"],
    });
    const countPtr = 0;
    const sizePtr = 4;
    wasi.environ_sizes_get(countPtr, sizePtr);
    const view = new DataView(buffer);
    assertEquals(view.getInt32(countPtr, true), 1); // 1 env var
    // "LANG=café" = 10 UTF-8 bytes (é is 2 bytes) + 1 null = 11
    assertEquals(view.getInt32(sizePtr, true), 11);
});

Deno.test("environ_get writes valid UTF-8", () => {
    const buffer = makeBuffer();
    const wasi = makeWasi({
        getBuffer: () => buffer,
        write: noop,
        svg: noop,
        readStdin: noopStr,
        env: ["K=café"],
    });
    // environ_get(environPtr, environBufPtr)
    const environPtr = 0;
    const environBufPtr = 64;
    wasi.environ_get(environPtr, environBufPtr);
    // Read the bytes written at environBufPtr
    const bytes = new Uint8Array(buffer, environBufPtr, 16);
    const decoder = new TextDecoder();
    // Find null terminator
    let end = 0;
    while (bytes[end] !== 0) end++;
    const str = decoder.decode(bytes.slice(0, end));
    assertEquals(str, "K=café");
});

Deno.test("args_sizes_get counts UTF-8 bytes", () => {
    const buffer = makeBuffer();
    const wasi = makeWasi({
        getBuffer: () => buffer,
        write: noop,
        svg: noop,
        readStdin: noopStr,
        args: ["héllo"],
    });
    const countPtr = 0;
    const sizePtr = 4;
    wasi.args_sizes_get(countPtr, sizePtr);
    const view = new DataView(buffer);
    assertEquals(view.getInt32(countPtr, true), 1);
    // "héllo" = 6 UTF-8 bytes + 1 null = 7
    assertEquals(view.getInt32(sizePtr, true), 7);
});
