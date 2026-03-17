import { WorkerChannel } from "./worker_channel.ts";
import { makeEnv } from "./env.ts";
import { makeWasi } from "./wasi.ts";

// --- Types ---

interface WasmExports {
    memory: WebAssembly.Memory;
    repl_new(ptr: number, len: number, level: number): number;
    repl_run(repl: number, ptr: number, len: number): boolean;
    repl_destroy(repl: number): void;
    string_allocate(size: number): number;
    string_deallocate(ptr: number): void;
    format(ptr: number, len: number): number;
    cstr_deallocate(ptr: number): void;
}

// --- Constants ---

const STDOUT = 1;
const STDERR = 2;

// --- Repl session ---

class ReplSession {
    private readonly exports: WasmExports;
    private readonly ptr: number;

    constructor(exports: WasmExports, input: string, level: number) {
        this.exports = exports;
        const [ptr, len] = encodeString(exports, input);
        this.ptr = exports.repl_new(ptr, len, level);
        exports.string_deallocate(ptr);
    }

    // Returns true if the user called exit() / quit().
    run(input: string): boolean {
        const [ptr, len] = encodeString(this.exports, input);
        try {
            return this.exports.repl_run(this.ptr, ptr, len);
        } finally {
            this.exports.string_deallocate(ptr);
        }
    }

    destroy(): void {
        this.exports.repl_destroy(this.ptr);
    }
}

// --- Worker ---

class Worker {
    private level = 4;
    private wasmModule!: WebAssembly.Module;
    private exports!: WasmExports;
    private session: ReplSession | null = null;
    private channel = new WorkerChannel();

    constructor() {
        this.loadWasm();
    }

    private getBuffer(): ArrayBuffer {
        return this.exports.memory.buffer as ArrayBuffer;
    }

    private async loadWasm(): Promise<void> {
        const wasmUrl = new URL(import.meta.url).searchParams.get("wasm") ??
            "spython.wasm";
        try {
            const response = await fetch(wasmUrl);
            if (!response.ok) {
                this.channel.error(
                    `Error loading ${wasmUrl}: ${response.status}`,
                );
                return;
            }
            const total = parseInt(
                response.headers.get("Content-Length") ?? "0",
            );
            const reader = response.body!.getReader();
            const chunks: Uint8Array[] = [];
            let loaded = 0;
            while (true) {
                const { done, value } = await reader.read();
                if (done) break;
                chunks.push(value);
                loaded += value.length;
                if (total) {
                    this.channel.progress((loaded / total) * 100);
                }
            }
            const wasmBytes = new Uint8Array(loaded);
            let offset = 0;
            for (const chunk of chunks) {
                wasmBytes.set(chunk, offset);
                offset += chunk.length;
            }
            await this.instantiateWasm(
                await WebAssembly.compile(wasmBytes.buffer),
            );
        } catch (error) {
            this.channel.error(`${error}`);
        }
    }

    private async instantiateWasm(
        wasmModule: WebAssembly.Module,
    ): Promise<void> {
        this.wasmModule = wasmModule;
        const instance = await WebAssembly.instantiate(wasmModule, {
            env: makeEnv({
                getBuffer: () => this.getBuffer(),
                checkInterrupt: () => this.channel.checkInterrupt(),
                sleep: (ms) => this.channel.sleep(ms),
                svg: (data) => this.channel.svg(data),
                dequeueKeyEvent: () => this.channel.dequeueKeyEvent(),
            }),
            wasi_snapshot_preview1: makeWasi({
                getBuffer: () => this.getBuffer(),
                write: (fd, text) => this.channel.write(fd, text),
                svg: (data) => this.channel.svg(data),
                env: ["RUST_BACKTRACE=1"],
            }),
        });
        this.exports = instance.exports as unknown as WasmExports;
        self.onmessage = (e) => this.processMsg(e);
        this.level = 4;
        this.initRepl("");
    }

    processMsg(event: MessageEvent): void {
        const data = event.data;
        switch (data.cmd) {
            case "run":
                this.runRepl(data.data);
                break;
            case "format":
                this.formatRepl(data.data);
                break;
            case "load":
                this.level = data.level ?? 4;
                this.initRepl(data.data);
                break;
            default:
                console.log(`${event}`);
        }
    }

    initRepl(input: string): void {
        this.session?.destroy();
        this.session = new ReplSession(this.exports, input, this.level);
        this.channel.ready();
    }

    async runRepl(input: string): Promise<void> {
        try {
            if (this.session!.run(input)) {
                // exit() / quit()
                this.channel.write(STDOUT, "Reloading the repl.");
                this.initRepl("");
            } else {
                this.channel.ready();
            }
        } catch (err) {
            console.log(err);
            this.channel.write(
                STDERR,
                "Execution error (probably a stackoverflow). Reloading the repl.",
            );
            this.session = null;
            await this.instantiateWasm(this.wasmModule);
        }
    }

    formatRepl(input: string): void {
        const [ptr, len] = encodeString(this.exports, input);
        const r = this.exports.format(ptr, len);
        this.exports.string_deallocate(ptr);
        if (r !== 0) {
            this.channel.formatted(readCstr(this.exports, r));
            this.exports.cstr_deallocate(r);
        } else {
            // Format failed (e.g. syntax error) — send original code unchanged
            this.channel.formatted(input);
        }
    }
}

// --- Memory helpers ---

function encodeString(exports: WasmExports, str: string): [number, number] {
    const encoded = new TextEncoder().encode(str);
    const ptr = exports.string_allocate(encoded.length);
    new Uint8Array(exports.memory.buffer, ptr, encoded.length).set(encoded);
    return [ptr, encoded.length];
}

function readCstr(exports: WasmExports, ptr: number): string {
    const buffer = new Uint8Array(exports.memory.buffer);
    let end = ptr;
    while (buffer[end] !== 0) end++;
    return new TextDecoder().decode(buffer.slice(ptr, end));
}

// --- Init ---

new Worker();
