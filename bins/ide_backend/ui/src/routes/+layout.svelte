<script>
    import { auth, bootAuth } from '$lib/stores/auth.svelte';
    import { onMount } from 'svelte';
    let { children } = $props();
    onMount(() => bootAuth());
</script>

{#if auth.isInitialized}
    {@render children()}
{:else}
    <div style="height:100vh;width:100vw;background:black;color:#00ffff;display:flex;align-items:center;justify-content:center;font-family:monospace;">
        NEURAL HANDSHAKE IN PROGRESS...
    </div>
{/if}
