import { assertEquals, assertMatch } from "jsr:@std/assert";
import { UIChannel, WorkerMessage } from "./ui_channel.ts";

const STDOUT = 1;
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

Deno.test("stop interrupts long-running code", async () => {
    return new Promise<void>((resolve, reject) => {
        const [worker, channel] = makeWorker();
        let readyCount = 0;

        const failTimeout = setTimeout(() => {
            worker.terminate();
            reject(new Error("Timed out: stop did not interrupt execution"));
        }, 5000);

        worker.onmessage = (event: MessageEvent<WorkerMessage>) => {
            const data = event.data;
            if (data.cmd === "ready") {
                readyCount++;
                if (readyCount === 1) {
                    channel.setBuffer(data.buffer);
                    channel.run("while True: pass");
                    setTimeout(() => channel.stop(), 200);
                } else {
                    // Interrupted — REPL is ready again
                    clearTimeout(failTimeout);
                    worker.terminate();
                    resolve();
                }
            } else if (data.cmd === "error") {
                clearTimeout(failTimeout);
                reject(new Error(`Worker error: ${data.data}`));
            }
        };
    });
});

Deno.test("passing doctests succeed silently", async () => {
    const code = [
        "def add(a: int, b: int) -> int:",
        '    """',
        "    >>> add(1, 2)",
        "    3",
        '    """',
        "    return a + b",
    ].join("\n");

    return new Promise<void>((resolve, reject) => {
        const [worker, channel] = makeWorker();
        let readyCount = 0;
        let stdout = "";
        let stderr = "";

        worker.onmessage = (event: MessageEvent<WorkerMessage>) => {
            const data = event.data;
            if (data.cmd === "ready") {
                readyCount++;
                if (readyCount === 1) {
                    channel.setBuffer(data.buffer);
                    channel.load(code);
                } else {
                    assertEquals(stdout, "");
                    assertEquals(stderr, "");
                    worker.terminate();
                    resolve();
                }
            } else if (data.cmd === "write") {
                if (data.fd === STDOUT) stdout += data.data;
                if (data.fd === STDERR) stderr += data.data;
            } else if (data.cmd === "error") {
                reject(new Error(`Worker error: ${data.data}`));
            }
        };
    });
});

Deno.test("failing doctests are reported on stderr", async () => {
    const code = [
        "def add(a: int, b: int) -> int:",
        '    """',
        "    >>> add(1, 2)",
        "    99",
        '    """',
        "    return a + b",
    ].join("\n");

    return new Promise<void>((resolve, reject) => {
        const [worker, channel] = makeWorker();
        let readyCount = 0;
        let stderr = "";

        worker.onmessage = (event: MessageEvent<WorkerMessage>) => {
            const data = event.data;
            if (data.cmd === "ready") {
                readyCount++;
                if (readyCount === 1) {
                    channel.setBuffer(data.buffer);
                    channel.load(code);
                } else {
                    assertMatch(stderr, /Failed example:/);
                    assertMatch(stderr, /Expected:\n\s+99/);
                    assertMatch(stderr, /Got:\n\s+3/);
                    assertMatch(stderr, /\*\*\*Test Failed\*\*\* 1 of 1/);
                    worker.terminate();
                    resolve();
                }
            } else if (data.cmd === "write" && data.fd === STDERR) {
                stderr += data.data;
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
        let readyCount = 0;
        let stderr = "";

        worker.onmessage = (event: MessageEvent<WorkerMessage>) => {
            const data = event.data;
            if (data.cmd === "ready") {
                readyCount++;
                if (readyCount === 1) {
                    channel.setBuffer(data.buffer);
                    channel.load(code);
                } else {
                    assertMatch(stderr, /\x1b\[/); // contains ANSI codes
                    assertMatch(stderr, /missing-parameter-annotation/);
                    assertMatch(stderr, /missing-return-annotation/);
                    assertMatch(stderr, /Found 2 errors/);
                    worker.terminate();
                    resolve();
                }
            } else if (data.cmd === "write" && data.fd === STDERR) {
                stderr += data.data;
            } else if (data.cmd === "error") {
                reject(new Error(`Worker error: ${data.data}`));
            }
        };
    });
});
