// Host environment imports for the REPL ABI (env namespace).
// Provides the env import namespace for WASM modules.

import { type KeyEvent } from "./ui_channel.ts";

const IS_DENO = "Deno" in globalThis;

export interface EnvOptions {
    getBuffer(): ArrayBuffer;
    checkInterrupt(): boolean;
    sleep(ms: bigint): void;
    svg(data: string): void;
    dequeueKeyEvent(): KeyEvent | null;
}

export function makeEnv(options: EnvOptions) {
    const buf = () => new Uint8Array(options.getBuffer());
    return {
        check_interrupt: (): number => options.checkInterrupt() ? 1 : 0,
        sleep: (ms: bigint): void => options.sleep(ms),
        draw_svg: (ptr: number, len: number): void => {
            const bytes = buf().slice(ptr, ptr + len);
            options.svg(new TextDecoder().decode(bytes));
        },
        get_key_event: (
            keyPtr: number,
            keyLen: number,
            modsPtr: number,
        ): number => {
            const event = options.dequeueKeyEvent();
            if (event === null) return 3; // no event
            // Write key string into WASM memory
            const mem = buf();
            const keyBytes = new TextEncoder().encode(event.key);
            const n = Math.min(keyBytes.length, keyLen);
            mem.set(keyBytes.subarray(0, n), keyPtr);
            if (n < keyLen) mem[keyPtr + n] = 0;
            // Write modifier bools (1 byte each)
            mem[modsPtr + 0] = event.alt ? 1 : 0;
            mem[modsPtr + 1] = event.ctrl ? 1 : 0;
            mem[modsPtr + 2] = event.shift ? 1 : 0;
            mem[modsPtr + 3] = event.meta ? 1 : 0;
            mem[modsPtr + 4] = event.repeat ? 1 : 0;
            return event.type; // KEYPRESS=0, KEYDOWN=1, KEYUP=2
        },
        text_height: (
            text: number,
            textLen: number,
            font: number,
            fontLen: number,
            size: number,
        ): number => {
            if (IS_DENO) return fontLen;
            const b = buf();
            const jtext = new TextDecoder().decode(
                b.slice(text, text + textLen),
            );
            const jfont = new TextDecoder().decode(
                b.slice(font, font + fontLen),
            );
            // deno-lint-ignore no-undef
            const offscreen = new OffscreenCanvas(1, 1);
            const ctx = offscreen.getContext("2d")!;
            ctx.font = `${size}px ${jfont}`;
            const metrics = ctx.measureText(jtext);
            return metrics.fontBoundingBoxAscent +
                metrics.fontBoundingBoxDescent;
        },
        text_width: (
            text: number,
            textLen: number,
            font: number,
            fontLen: number,
            size: number,
        ): number => {
            if (IS_DENO) return 0.6 * fontLen * textLen;
            const b = buf();
            const jtext = new TextDecoder().decode(
                b.slice(text, text + textLen),
            );
            const jfont = new TextDecoder().decode(
                b.slice(font, font + fontLen),
            );
            // deno-lint-ignore no-undef
            const offscreen = new OffscreenCanvas(1, 1);
            const ctx = offscreen.getContext("2d")!;
            ctx.font = `${size}px ${jfont}`;
            return ctx.measureText(jtext).width;
        },
    };
}
