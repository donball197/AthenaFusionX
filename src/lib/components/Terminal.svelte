<script>
    import { onMount } from 'svelte';
    // Direct scoped import to satisfy Vite/Rollup
    import '@xterm/xterm/css/xterm.css';

    let termContainer;
    let socket;
    let term;

    export function triggerCommand(cmd) {
        if (socket && socket.readyState === WebSocket.OPEN) {
            socket.send(cmd + "\r");
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
        };

        term.onData((data) => {
            if (socket.readyState === WebSocket.OPEN) {
                socket.send(data);
            }
        });

        window.addEventListener('resize', () => fitAddon.fit());
    });
</script>

<div class="h-full w-full bg-black p-2 overflow-hidden" bind:this={termContainer}></div>
