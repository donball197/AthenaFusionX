<script lang="ts">
    // +layout.svelte — v1.1 [SESSION_RESTORE]
    // FIX: restoreSession() was never called — sessionStorage tokens were
    //      lost on every page reload, forcing re-auth every time.
    import "../app.css";
    import { onMount, onDestroy } from "svelte";
    import { bootRuntime } from "$lib/core/runtime";
    import { nexusSocket } from "$lib/core/socket";
    import { restoreSession } from "$lib/stores/auth.svelte";

    let { children } = $props();

    onMount(() => {
        // Restore session token before booting runtime so uplink has auth
        restoreSession();
        bootRuntime();
    });

    onDestroy(() => {
        nexusSocket.destroy();
    });
</script>

{@render children()}
