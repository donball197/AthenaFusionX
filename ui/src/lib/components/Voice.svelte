<script>
    import { onMount } from 'svelte';

    // Svelte 5: $bindable allows parent to two-way bind userPrompt.
    let {
        userPrompt  = $bindable(''),
        isListening = $bindable(false),
        disabled    = false,
    } = $props();

    let recognition;

    onMount(() => {
        const SR = window.SpeechRecognition || window.webkitSpeechRecognition;
        if (!SR) return;

        recognition = new SR();
        recognition.continuous     = false;
        recognition.interimResults = false;

        recognition.onstart  = () => { isListening = true;  };
        recognition.onend    = () => { isListening = false; };
        recognition.onerror  = () => { isListening = false; };

        recognition.onresult = (event) => {
            const transcript = event.results[0][0].transcript;
            userPrompt = userPrompt ? `${userPrompt} ${transcript}` : transcript;
        };
    });

    function toggle() {
        if (disabled || !recognition) return;
        isListening ? recognition.stop() : recognition.start();
    }
</script>

<!-- FIX: on:click → onclick (Svelte 5 event syntax) -->
<button
    onclick={toggle}
    {disabled}
    class="w-6 h-6 border flex items-center justify-center rounded transition-all
           {disabled    ? 'border-gray-800 cursor-not-allowed' :
            isListening ? 'border-red-500 bg-red-900/30 shadow-[0_0_10px_#ef4444]' :
                          'border-[#30363d] hover:border-green-500 bg-[#0a0a0a]'}"
    title="Voice Uplink"
    type="button"
>
    <div class="w-2 h-2 rounded-full transition-all
        {disabled    ? 'bg-gray-700' :
         isListening ? 'bg-red-500 animate-pulse' :
                       'bg-green-600'}">
    </div>
</button>
