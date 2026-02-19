<script>
    import Response from './Response.svelte';
    export let response = "";
    export let isThinking = false;
    export let patchAvailable = false;
    export let onApplyPatch;
</script>

<div class="flex flex-col min-h-[320px] max-h-[400px] bg-[#050505] border border-blue-900/30 p-3 rounded shadow-2xl">
    <Response {response} {isThinking} {patchAvailable} {onApplyPatch} />
</div>
