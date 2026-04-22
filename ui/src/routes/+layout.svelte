<!--
  +layout.svelte — v1.1
  FIX: Added onDestroy to disconnect WS when layout unmounts.
  Single WebSocket boot point — all routes share this layout.
-->
<script>
    import "../app.css";
    import { connectWS, nexusWS } from '$lib/core/NexusWS.js';
    import { onMount, onDestroy } from 'svelte';

    let { children } = $props();

    onMount(() => {
        connectWS();
    });

    // FIX: clean up on layout destroy
    onDestroy(() => {
        nexusWS.disconnect();
    });
</script>

{@render children()}
