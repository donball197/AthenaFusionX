<script>
    import "../app.css";
    import { layout } from '$lib/core/store.svelte.js';
    import { connectWS } from '$lib/core/NexusWS.js';
    import { onMount } from 'svelte';

    onMount(() => {
        connectWS();
    });
</script>

<slot />
