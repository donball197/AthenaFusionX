<script>
    import { onMount } from 'svelte';

    export let userPrompt = "";
    export let isListening = false;
    export let disabled = false;

    let recognition;

    onMount(() => {
        if ('SpeechRecognition' in window || 'webkitSpeechRecognition' in window) {
            const SpeechRecognition = window.SpeechRecognition || window.webkitSpeechRecognition;
            recognition = new SpeechRecognition();
            recognition.continuous = false;
            recognition.interimResults = false;

            recognition.onstart = () => { isListening = true; };
            recognition.onend = () => { isListening = false; };
            recognition.onerror = (e) => { console.error("Comms Error:", e); isListening = false; };
            recognition.onresult = (event) => {
                const transcript = event.results[0][0].transcript;
                userPrompt = userPrompt ? `${userPrompt} ${transcript}` : transcript;
            };
        }
    });

    function toggle() {
        if (disabled || !recognition) return;
        isListening ? recognition.stop() : recognition.start();
    }
</script>

<button 
    on:click={toggle}
    {disabled}
    class="absolute left-6 z-10 flex items-center justify-center w-5 h-5 border transition-all duration-200 
           {disabled ? 'border-[#1f2428] bg-transparent cursor-not-allowed' : 
            isListening ? 'border-red-500 bg-red-900/20 shadow-[0_0_8px_#ef4444]' : 
            'border-[#30363d] bg-[#0a0a0a] hover:border-green-500'}"
    title="Tactical Voice Uplink"
>
    <div class="w-1.5 h-1.5 rounded-full transition-all duration-300 
        {disabled ? 'bg-[#30363d]' : 
         isListening ? 'bg-red-500 animate-pulse shadow-[0_0_5px_#ef4444]' : 
         'bg-green-700'}">
    </div>
</button>
