<script>
    import { onMount } from 'svelte';

    let logs = $state([]);
    let terminalContainer = $state(null);
    let ws = $state(null);
    let idCounter = 0;

    onMount(() => {
        connectWebSocket();
        return () => {
            if (ws) ws.close();
        };
    });

    // ⚡ FIX: Svelte 5 Runes Reactive Auto-Scroll
    $effect(() => {
        // Accessing logs.length tells the compiler to track this array
        if (logs.length >= 0 && terminalContainer) {
            terminalContainer.scrollTop = terminalContainer.scrollHeight;
        }
    });

    function connectWebSocket() {
        const host = window.location.host;
        ws = new WebSocket(`ws://${host}/ws/telemetry`);
        
        ws.onopen = () => {
            addLog('SYSTEM', '>> UPLINK ESTABLISHED: GHOST CORTEX ONLINE', 'text-green-400 font-bold');
        };

        ws.onmessage = (event) => {
            try {
                const msg = JSON.parse(event.data);
                handleMessage(msg);
            } catch (e) {
                addLog('RAW', event.data, 'text-gray-400');
            }
        };

        ws.onclose = () => {
            addLog('SYSTEM', '>> UPLINK SEVERED. ATTEMPTING RECONNECTION...', 'text-red-500 font-bold');
            setTimeout(connectWebSocket, 3000);
        };
    }

    function handleMessage(msg) {
        let color = 'text-gray-300';
        let prefix = '';

        switch (msg.type) {
            case 'NEURAL_STREAM':
                color = 'text-cyan-400';
                break;
            case 'NEURAL_THOUGHT':
                color = 'text-purple-400 italic';
                prefix = '[THOUGHT] ';
                break;
            case 'TASK_STATUS':
                color = 'text-green-400 font-semibold';
                break;
            case 'LOG_EVENT':
                color = 'text-yellow-400';
                break;
            case 'TERMINAL_OUT':
                color = 'text-gray-100';
                break;
            case 'TERMINAL_ERR':
                color = 'text-red-400';
                break;
            default:
                color = 'text-gray-500';
        }

        if (msg.type === 'NEURAL_STREAM' && logs.length > 0 && logs[logs.length - 1].type === 'NEURAL_STREAM') {
            logs[logs.length - 1].payload += msg.payload;
        } else {
            addLog(msg.type, prefix + msg.payload, color);
        }
    }

    function addLog(type, payload, color) {
        logs.push({ id: idCounter++, type, payload, color });
        if (logs.length > 500) logs.shift(); // Bound memory footprint
    }
</script>

<div class="flex flex-col h-full bg-[#0d1117] border border-gray-800 rounded shadow-lg font-mono text-sm">
    <div class="flex items-center px-4 py-2 bg-[#161b22] border-b border-gray-800 shadow-sm">
        <div class="w-3 h-3 rounded-full bg-red-500 mr-2"></div>
        <div class="w-3 h-3 rounded-full bg-yellow-500 mr-2"></div>
        <div class="w-3 h-3 rounded-full bg-green-500 mr-3"></div>
        <span class="text-gray-400 text-xs font-semibold uppercase tracking-wider">Ghost Cortex // Terminal Multiplexer</span>
    </div>
    
    <div bind:this={terminalContainer} class="flex-1 p-4 overflow-y-auto whitespace-pre-wrap break-words leading-relaxed">
        {#each logs as log (log.id)}
            <div class="{log.color} mb-1">{log.payload}</div>
        {/each}
    </div>
</div>
