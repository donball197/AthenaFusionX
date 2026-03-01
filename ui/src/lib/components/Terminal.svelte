<script>
    import { onMount, tick, onDestroy } from 'svelte';
    
    let logs = [];
    let socket;
    let inputCmd = "";
    let terminalDiv;

    // API
    export function getLogs() { return logs.join("\n"); }
    export function triggerCommand(cmd) { 
        if (socket && socket.readyState === WebSocket.OPEN) { 
            socket.send(cmd); // Sent as text
        } else {
            logs = [...logs, ">> ERROR: TERMINAL OFFLINE"];
        }
    }

    onMount(() => {
        const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        socket = new WebSocket(`${proto}//${window.location.host}/ws`);
        
        socket.onopen = () => { logs = [...logs, ">> NEXUS SHELL UPLINK ESTABLISHED..."]; };
        
        socket.onmessage = async (event) => {
            const raw = event.data;
            const clean = raw.replace(/\x1B\[[0-9;]*[mK]/g, ""); 
            logs = [...logs, clean].slice(-500);
            
            await tick();
            if (terminalDiv) terminalDiv.scrollTop = terminalDiv.scrollHeight;
        };

        socket.onclose = () => { logs = [...logs, ">> DISCONNECTED."]; };
    });
    
    onDestroy(() => { if (socket) socket.close(); });

    function handleKey(e) { 
        if (e.key === 'Enter') { 
            triggerCommand(inputCmd); 
            inputCmd = ""; 
        } 
    }
</script>

<div class="h-full w-full bg-[#0b0c10] text-[#8ab4f8] font-mono text-[11px] flex flex-col">
    <div class="flex-1 overflow-y-auto whitespace-pre-wrap leading-tight p-3 custom-scrollbar" bind:this={terminalDiv}>
        {#each logs as log}
            <div class="break-words">{log}</div>
        {/each}
    </div>
    <div class="shrink-0 flex pt-2 pb-2 px-3 border-t border-[#30363d] items-center bg-[#161b22]">
        <span class="mr-2 text-green-500 font-bold">$</span>
        <input 
            class="flex-1 bg-transparent outline-none text-white placeholder-gray-600" 
            bind:value={inputCmd} 
            on:keydown={handleKey} 
            placeholder="Execute command..."
            autocomplete="off"
            spellcheck="false"
        />
    </div>
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 6px; }
    .custom-scrollbar::-webkit-scrollbar-track { background: #0b0c10; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #30363d; border-radius: 3px; }
    .custom-scrollbar::-webkit-scrollbar-thumb:hover { background: #58a6ff; }
</style>
