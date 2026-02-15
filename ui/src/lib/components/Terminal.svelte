<script>
    import { onMount } from 'svelte';
    // CORRECT SCOPED PATH
    import '@xterm/xterm/css/xterm.css';

    let termContainer;
    let socket;
    let term;

    // NEW: Allow Nexus to "read" the terminal screen
    export function getLogs() {
        if (!term) return "";
        // Grab the last 30 lines of the terminal buffer for context
        const totalLines = term.buffer.active.length;
        let logs = "";
        for (let i = Math.max(0, totalLines - 30); i < totalLines; i++) {
            const line = term.buffer.active.getLine(i);
            if (line) logs += line.translateToString() + "\n";
        }
        return logs;
    }

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
