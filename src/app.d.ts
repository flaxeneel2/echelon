import type { invoke, type InvokeArgs } from "@tauri-apps/api/core";
import type { listen } from "@tauri-apps/api/event";

declare global {
    interface Window {
        core: {
            invoke: (fn: string, args?: InvokeArgs) => Promise<unknown>;
            invoke_no_timer: typeof invoke;
            listen: typeof listen;
        };
    }
}

export {};
