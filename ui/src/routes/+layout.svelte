<script>
    import "../app.css";
    import { nexusSocket } from '$lib/nexus_socket.svelte.ts';
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
