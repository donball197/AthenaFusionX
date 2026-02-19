<script>
  import '../app.css';
  import HeadsUpDisplay from '$lib/components/HeadsUpDisplay.svelte';

  // Layout acts as the "Cockpit Frame". 
  // Specific tools (Blade, Vitals, Editor) are delegated to the +page.svelte
  // to maintain tighter state coupling with the file system.
</script>

<HeadsUpDisplay />

<main class="relative z-10 w-full min-h-screen bg-black">
    <slot />
</main>
