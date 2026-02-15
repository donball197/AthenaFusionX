<script>
    import { onMount } from 'svelte';
    let termElement;
    let socket;
    let output = [">> SECURE CHANNEL OPENED", ">> WAITING FOR KERNEL..."];

    onMount(() => {
        const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        socket = new WebSocket(`${proto}//${window.location.host}/ws`);

        socket.onmessage = (event) => {
            output = [...output, event.data];
            if (output.length > 100) output = output.slice(-100);
            scrollToBottom();
        };

        return () => socket.close();
    });

    function scrollToBottom() {
        if (termElement) termElement.scrollTop = termElement.scrollHeight;
    }

    function handleInput(e) {
        if (e.key === 'Enter') {
            const cmd = e.target.value;
            socket.send(cmd + '\n');
            e.target.value = '';
        }
    }
</script>

<div class="h-full flex flex-col bg-black text-green-500 font-mono p-2 overflow-hidden">
    <div class="flex-1 overflow-y-auto whitespace-pre-wrap" bind:this={termElement}>
        {#each output as line}
            <div>{line}</div>
        {/each}
    </div>
    <div class="flex gap-2 mt-2 border-t border-gray-800 pt-2">
        <span class="text-neon-green">root@nexus:~#</span>
        <!-- svelte-ignore a11y-autofocus -->
        <input 
            class="flex-1 bg-transparent border-none outline-none text-white focus:ring-0" 
            on:keydown={handleInput}
            autofocus
        />
    </div>
</div>
