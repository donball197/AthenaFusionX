<script>
  import '../app.css';
  import { onMount } from 'svelte';

  onMount(() => {
    console.log(">> [NEXUS] UI BOOT SEQUENCE INITIATED.");
  });
</script>

<main class="relative w-full h-screen bg-black overflow-hidden">
    <slot />
</main>

<style>
  :global(body) { margin: 0; background: black; color: #00ffcc; overflow: hidden; }
</style>
