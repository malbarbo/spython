import { assertEquals, assertMatch } from "jsr:@std/assert";
import { KEYDOWN, UIChannel, WorkerMessage } from "./ui_channel.ts";
import { STDERR, STDOUT } from "./wasi.ts";

const DIST = new URL("../dist/", import.meta.url).href;

function makeWorker(): [Worker, UIChannel] {
    const worker = new Worker(
        `${DIST}worker.js?wasm=spython.wasm`,
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

Deno.test("passing doctests report count", async () => {
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
                    channel.load(code, 4);
                } else {
                    assertEquals(stdout, "");
                    assertEquals(stderr, "1 example passed.\n");
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
                    channel.load(code, 4);
                } else {
                    assertMatch(stderr, /Failed example/);
                    assertMatch(stderr, /Expected/);
                    assertMatch(stderr, /Got/);
                    assertMatch(stderr, /Test Failed.*1 of 1/);
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
                    channel.load(code, 4);
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

Deno.test("show_svg sends svg message", async () => {
    const code = [
        "from spython import circle, fill, to_svg, show_svg, red",
        "show_svg(to_svg(circle(30, fill(red))))",
    ].join("\n");

    return new Promise<void>((resolve, reject) => {
        const [worker, channel] = makeWorker();
        let readyCount = 0;

        worker.onmessage = (event: MessageEvent<WorkerMessage>) => {
            const data = event.data;
            if (data.cmd === "ready") {
                readyCount++;
                if (readyCount === 1) {
                    channel.setBuffer(data.buffer);
                    channel.load(code, 5);
                }
            } else if (data.cmd === "svg") {
                assertMatch(data.data, /^<svg /);
                assertMatch(data.data, /ellipse/);
                assertMatch(data.data, /rgba\(255, 0, 0/);
                worker.terminate();
                resolve();
            } else if (data.cmd === "error") {
                reject(new Error(`Worker error: ${data.data}`));
            }
        };
    });
});

Deno.test("get_key_event returns enqueued key", async () => {
    return new Promise<void>((resolve, reject) => {
        const [worker, channel] = makeWorker();
        let readyCount = 0;

        worker.onmessage = (event: MessageEvent<WorkerMessage>) => {
            const data = event.data;
            if (data.cmd === "ready") {
                readyCount++;
                if (readyCount === 1) {
                    channel.setBuffer(data.buffer);
                    // Enqueue a key event before running code that reads it
                    channel.enqueueKeyEvent({
                        type: KEYDOWN,
                        key: "a",
                        alt: false,
                        ctrl: false,
                        shift: false,
                        meta: false,
                        repeat: false,
                    });
                    channel.run(
                        "from spython.system import get_key_event; print(get_key_event())",
                    );
                }
            } else if (data.cmd === "write") {
                if (data.fd === STDERR) return;
                // Should be a tuple: (1, 'a', False, False, False, False, False)
                assertMatch(
                    data.data,
                    /\(1, 'a', False, False, False, False, False\)/,
                );
                worker.terminate();
                resolve();
            } else if (data.cmd === "error") {
                reject(new Error(`Worker error: ${data.data}`));
            }
        };
    });
});

Deno.test("dataclass and enum work", async () => {
    const code = [
        "from dataclasses import dataclass",
        "from enum import Enum",
        "@dataclass",
        "class P:",
        "    x: int",
        "    y: int",
        "class Color(Enum):",
        "    RED = 1",
        "    BLUE = 2",
        "print(P(1, 2), Color.RED)",
    ].join("\n");

    return new Promise<void>((resolve, reject) => {
        const [worker, channel] = makeWorker();
        let readyCount = 0;

        worker.onmessage = (event: MessageEvent<WorkerMessage>) => {
            const data = event.data;
            if (data.cmd === "ready") {
                readyCount++;
                if (readyCount === 1) {
                    channel.setBuffer(data.buffer);
                    channel.load(code, 5);
                }
            } else if (data.cmd === "write") {
                if (data.fd === STDOUT) {
                    assertMatch(data.data, /P\(x=1, y=2\)/);
                    assertMatch(data.data, /Color\.RED/);
                    worker.terminate();
                    resolve();
                }
            } else if (data.cmd === "error") {
                reject(new Error(`Worker error: ${data.data}`));
            }
        };
    });
});
