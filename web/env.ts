// Host environment imports for the REPL ABI (env namespace).
// Provides the env import namespace for WASM modules.

export interface EnvOptions {
    getBuffer(): ArrayBuffer;
    checkInterrupt(): boolean;
    sleep(ms: bigint): void;
}

export function makeEnv(options: EnvOptions) {
    return {
        check_interrupt: (): number => options.checkInterrupt() ? 1 : 0,
        sleep: (ms: bigint): void => options.sleep(ms),
    };
}
