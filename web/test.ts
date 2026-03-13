import { assertEquals, assertMatch } from "jsr:@std/assert";
import { UIChannel, WorkerMessage } from "./ui_channel.ts";

const STDERR = 2;

function makeWorker(): [Worker, UIChannel] {
    const worker = new Worker(
        new URL("./worker.js?wasm=spython.wasm", import.meta.url).href,
        { type: "module" },
    );
    return [worker, new UIChannel(worker)];
}

Deno.test("repl smoke test", async () => {
    return new Promise<void>((resolve, reject) => {
        const [worker, channel] = makeWorker();
        let initialized = false;

        worker.onmessage = (event: MessageEvent<WorkerMessage>) => {
            const data = event.data;
            if (data.cmd === "ready") {
                if (!initialized) {
                    initialized = true;
                    channel.setBuffer(data.buffer);
                    channel.run("1 + 2");
                }
            } else if (data.cmd === "write") {
                if (data.fd === STDERR) return;
                assertEquals(data.data, "3\n");
                worker.terminate();
                resolve();
            } else if (data.cmd === "error") {
                reject(new Error(`Worker error: ${data.data}`));
            }
        };
    });
});

Deno.test("multiple runs", async () => {
    return new Promise<void>((resolve, reject) => {
        const [worker, channel] = makeWorker();
        let readyCount = 0;

        worker.onmessage = (event: MessageEvent<WorkerMessage>) => {
            const data = event.data;
            if (data.cmd === "ready") {
                readyCount++;
                if (readyCount === 1) {
                    channel.setBuffer(data.buffer);
                    channel.run("1 + 2");
                } else if (readyCount === 2) {
                    channel.run("10 + 20");
                }
            } else if (data.cmd === "write") {
                if (data.fd === STDERR) return;
                if (readyCount === 1) {
                    assertEquals(data.data, "3\n");
                } else if (readyCount === 2) {
                    assertEquals(data.data, "30\n");
                    worker.terminate();
                    resolve();
                }
            } else if (data.cmd === "error") {
                reject(new Error(`Worker error: ${data.data}`));
            }
        };
    });
});

Deno.test("type error output contains ansi codes", async () => {
    // Load code with missing annotations — ty prints colored diagnostics via repl_new.
    const code = "def foo(x):\n    return x\n";
    return new Promise<void>((resolve, reject) => {
        const [worker, channel] = makeWorker();
        let initialized = false;

        worker.onmessage = (event: MessageEvent<WorkerMessage>) => {
            const data = event.data;
            if (data.cmd === "ready") {
                if (!initialized) {
                    initialized = true;
                    channel.setBuffer(data.buffer);
                    channel.load(code);
                }
            } else if (data.cmd === "write") {
                if (data.fd === STDERR && /\x1b\[/.test(data.data)) {
                    worker.terminate();
                    resolve();
                }
            } else if (data.cmd === "error") {
                reject(new Error(`Worker error: ${data.data}`));
            }
        };
    });
});
