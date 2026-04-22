<!--
  +layout.svelte — Single WebSocket boot point.
  All routes share this layout. WS is connected exactly once here.
-->
<script>
    import "../app.css";
    import { connectWS } from '$lib/core/NexusWS.js';
    import { onMount } from 'svelte';

    let { children } = $props();

    onMount(() => {
        connectWS();
    });
</script>

{@render children()}
