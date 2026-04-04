<script>
    import { onMount, onDestroy } from 'svelte';
 
    export let active = false;
 
    let visionSrc = "";
    let visionLoop;
    let isLooping = false;

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
            if (data.status === 'ok' && data.response) {
                visionSrc = "data:image/jpeg;base64," + data.response;
            }
        } catch(e) {
            console.log("Vision feed glitch");
        }
        
        if (active) {
            visionLoop = setTimeout(startVisionLoop, 220);
        }
    }

    onDestroy(stopVisionLoop);
</script>

{#if active && visionSrc}
<div class="absolute top-16 right-8 w-80 border border-red-600/60 bg-black/95 z-50 shadow-2xl backdrop-blur-sm overflow-hidden">
    <div class="bg-red-950/80 text-red-400 text-[9px] px-3 py-1.5 flex justify-between border-b border-red-900/50">
        <span>OPTIC_NERVE // LIVE_FEED</span>
        <span class="animate-pulse">REC</span>
    </div>
    <img src={visionSrc} class="w-full block" alt="Neural Vision" />
    <div class="px-3 py-1 text-[9px] text-gray-500 font-mono">TARGET: DISPLAY_0 • 5 FPS</div>
</div>
{/if}
