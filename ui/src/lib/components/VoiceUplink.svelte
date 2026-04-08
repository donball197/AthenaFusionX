<script>
    import Voice from './Voice.svelte';
    let { prompt = $bindable("") } = $props();
</script>

<div class="relative">
    <Voice bind:userPrompt={prompt} />
    <div class="absolute -bottom-5 left-1/2 -translate-x-1/2 text-[9px] text-green-600/70 whitespace-nowrap">
        VOICE UPLINK ACTIVE
    </div>
</div>
