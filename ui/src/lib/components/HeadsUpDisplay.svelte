<script>
    import { onMount } from 'svelte';

    export let ramUsage = "0.0";
    $: ramNum = parseFloat(ramUsage) || 0;
    $: isCritical = ramNum > 2.8;           // warn earlier — safer on 4GB devices

    // Only show background when not critical (saves rendering cost)
    $: showBackground = !isCritical;
</script>

{#if showBackground}
<div class="fixed inset-0 pointer-events-none z-0 flex items-center justify-center font-mono opacity-5">
    <div class="relative w-400 h-400 animate-[spin_90s_linear_infinite]">
        <div class="absolute inset-0 border border-cyan-500/15 rounded-full"></div>
    </div>
</div>
{/if}

{#if isCritical}
<div class="fixed top-0 left-0 right-0 h-5 bg-red-700/85 text-white text-[9px] font-bold flex items-center justify-center z-[9999] animate-pulse">
    ⚠ CRITICAL RAM ({ramNum.toFixed(1)} GB) – CLOSE TABS / RESTART NOW ⚠
</div>
{/if}

<style>
    @keyframes spin { to { transform: rotate(360deg); } }
</style>
