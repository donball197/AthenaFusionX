<script>
    import { onMount } from 'svelte';
    import SoundFx from '$lib/SoundFx.js';

    export let userPrompt = "";
    export let isListening = false;
    
    let recognition;

    onMount(() => {
        if ('webkitSpeechRecognition' in window || 'SpeechRecognition' in window) {
            const SpeechRecognition = window.SpeechRecognition || window.webkitSpeechRecognition;
            recognition = new SpeechRecognition();
            recognition.continuous = false;
            recognition.interimResults = true;

            recognition.onresult = (event) => {
                userPrompt = event.results[0][0].transcript;
            };

            recognition.onend = () => {
                isListening = false;
            };
        }
    });

    export function toggle() {
        if (!recognition) return;
        SoundFx.click();
        if (isListening) {
            recognition.stop();
            isListening = false;
        } else {
            userPrompt = "";
            try {
                recognition.start();
                isListening = true;
            } catch(e) { console.error("Speech Start Error:", e); }
        }
    }
</script>

<button on:click={toggle} class="absolute left-2 z-20 transition-all {isListening ? 'text-red-500 animate-pulse' : 'text-gray-500 hover:text-white'}">
    {#if isListening}🎙️{:else}🔇{/if}
</button>
