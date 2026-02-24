<script>
    import { onMount, tick } from 'svelte';
    import '@xterm/xterm/css/xterm.css';

    let termContainer;
    let socket;
    let term;
    let logs = [];
    let inputCmd = "";

    export function triggerCommand(cmd) {
        if (socket && socket.readyState === WebSocket.OPEN) {
            socket.send(cmd + "\n");
        }
    }

    onMount(async () => {
        const { Terminal } = await import('@xterm/xterm');
        const { FitAddon } = await import('@xterm/addon-fit');

        term = new Terminal({
            cursorBlink: true,
            fontSize: 12,
            fontFamily: '"JetBrains Mono", monospace',
            theme: {
                background: '#000000',
                foreground: '#00ff00',
                cursor: '#00ff00'
            }
        });

        const fitAddon = new FitAddon();
        term.loadAddon(fitAddon);
        term.open(termContainer);
        fitAddon.fit();

        const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        socket = new WebSocket(`${proto}//${window.location.host}/ws`);

        socket.onmessage = (event) => {
            term.write(event.data);
            const clean = event.data.replace(/\u001b\[[0-9;]*m/g, "");
            logs = [...logs, clean].slice(-100);
        };

        term.onData((data) => {
            if (socket.readyState === WebSocket.OPEN) {
                socket.send(data);
            }
        });

        window.addEventListener('resize', () => fitAddon.fit());
    });

    function handleKey(e) {
        if (e.key === 'Enter') {
            triggerCommand(inputCmd);
            inputCmd = "";
        }
    }
</script>

<div class="h-full w-full bg-black p-2 overflow-hidden flex flex-col">
    <div class="flex-1" bind:this={termContainer}></div>
    <div class="flex mt-2 border-t border-gray-800 pt-1">
        <span class="mr-2 text-green-500">$</span>
        <input 
            class="flex-1 bg-transparent outline-none text-white placeholder-gray-700 font-mono text-xs" 
            bind:value={inputCmd} 
            on:keydown={handleKey} 
            placeholder="Manual Override..."
        />
    </div>
</div>
