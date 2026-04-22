<!--
  +layout.svelte — v2.0 SINGLE WS BOOT POINT
  FIX: Imports nexusSocket from nexus_socket.svelte.ts (the real WS).
       connect() is called once in onMount.
       destroy() is called in onDestroy to clean up on SPA unmount.
-->
<script>
    import "../app.css";
    import { nexusSocket } from '$lib/nexus_socket.svelte';
    import { onMount, onDestroy } from 'svelte';

    let { children } = $props();

    onMount(() => {
        nexusSocket.connect();
    });

    onDestroy(() => {
        nexusSocket.destroy();
    });
</script>

{@render children()}
