<script>
  import { onMount, onDestroy } from 'svelte';
  
  export let active = false;
  
  let visionSrc = "";
  let visionLoop;
  let isLooping = false;

  // Reactive: When 'active' changes, start or stop the eye.
  $: if (active) {
    if (!isLooping) startVisionLoop();
  } else {
    stopVisionLoop();
  }

  function stopVisionLoop() {
    isLooping = false;
    clearTimeout(visionLoop);
    visionSrc = "";
  }

  async function startVisionLoop() {
    isLooping = true;
    try {
        const res = await fetch('/api/vision/frame');
        const data = await res.json();
        if (data.status === 'ok') {
            visionSrc = "data:image/jpeg;base64," + data.response;
        }
    } catch(e) { 
        console.log("Vision Glitch", e); 
    }
    
    // Check if we are still active before looping
    if (active) {
        visionLoop = setTimeout(startVisionLoop, 200); // 5 FPS
    }
  }

  onDestroy(() => stopVisionLoop());
</script>

{#if active && visionSrc}
<div class="absolute top-4 right-4 w-72 border border-red-500/50 bg-black/90 z-50 shadow-2xl backdrop-blur-sm pointer-events-none">
    <div class="bg-red-900/20 text-red-500 text-[9px] font-bold px-2 py-1 flex justify-between border-b border-red-900/30">
        <span>OPTIC NERVE // LIVE</span>
        <span class="animate-pulse">REC</span>
    </div>
    <img src={visionSrc} class="w-full opacity-90 block" alt="Neural Feed" />
    <div class="px-2 py-1 text-[8px] text-gray-500 font-mono">TARGET: DISPLAY_0</div>
</div>
{/if}
