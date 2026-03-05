<script lang="ts">
    import { invoke, type InvokeArgs } from "@tauri-apps/api/core";
    import { listen } from "@tauri-apps/api/event";
    import "$lib/styles/chat.css";

    // Tauri Core Shim
    window.core = window.core || {} as Window["core"];
    window.core.invoke_no_timer = invoke;
    window.core.invoke = async (fn_to_invoke: string, args: InvokeArgs | undefined) => {
        const start = performance.now();
        try {
            const res = await invoke(fn_to_invoke, args);
            console.log(`Fetch [${fn_to_invoke}] took ${performance.now() - start}ms.`, res);
            return res;
        } catch (error) {
            console.error(`Command [${fn_to_invoke}] failed:`, error);
            throw error;
        }
    };
    window.core.listen = listen;


</script>
