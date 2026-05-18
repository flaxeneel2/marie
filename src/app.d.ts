import type { invoke, type InvokeArgs } from "@tauri-apps/api/core";
declare global {
    interface Window {
        core: {
            invoke: (fn: string, args?: InvokeArgs) => Promise<unknown>;
        };
    }
}

export {};