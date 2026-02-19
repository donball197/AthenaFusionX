<script>
  import '../app.css';
  import { onMount } from 'svelte';
  import HeadsUpDisplay from '$lib/components/HeadsUpDisplay.svelte';
  import TacticalBlade from '$lib/components/TacticalBlade.svelte';
  import Vitals from '$lib/components/Vitals.svelte';

  let visorOpen = false;
  
  // System State
  let stats = {
    status: "OFFLINE",
    cpu: 0,
    ram: "0GB / 0GB"
  };

  async function refreshVitals() {
    try {
      const res = await fetch('/api/vitals');
      if (res.ok) {
        const data = await res.json();
        // The backend returns { status, cpu_usage, ram_usage }
        stats.status = data.status;
        stats.cpu = data.cpu_usage;
        stats.ram = data.ram_usage;
      }
    } catch (e) {
      stats.status = "DISCONNECTED";
    }
  }

  onMount(() => {
    refreshVitals();
    const interval = setInterval(refreshVitals, 2000);
    return () => clearInterval(interval);
  });

  function handleFileSelect(path) {
      console.log("OPENING:", path);
  }
</script>

<HeadsUpDisplay />

<div class="fixed bottom-4 right-4 z-50 w-64 shadow-2xl">
    <Vitals 
      status={stats.status} 
      cpu={stats.cpu} 
      ram={stats.ram} 
    />
</div>

<TacticalBlade bind:open={visorOpen} onFileSelect={handleFileSelect} />

<main class="relative z-10 w-full min-h-screen">
    <slot />
</main>
