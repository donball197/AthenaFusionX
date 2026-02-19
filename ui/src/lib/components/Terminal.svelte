<script>
    import { onMount } from 'svelte';
    import '@xterm/xterm/css/xterm.css';

    let termContainer;
    let socket;
    let term;
    let fitAddon;

    function syncSize() {
        if (fitAddon && term && socket && socket.readyState === 1) {
            fitAddon.fit();
            if (term.rows > 0 && term.cols > 0) {
                socket.send(`RESIZE:${term.rows}:${term.cols}`);
            }
        }
    }

    export function getLogs() {
        if (!term) return "";
        const lines = term.buffer.active.length;
        let logs = "";
        for (let i = Math.max(0, lines - 30); i < lines; i++) {
            const line = term.buffer.active.getLine(i);
            if (line) logs += line.translateToString() + "\n";
        }
        return logs;
    }

    onMount(async () => {
        const { Terminal } = await import('@xterm/xterm');
        const { FitAddon } = await import('@xterm/addon-fit');

        term = new Terminal({
            cursorBlink: true, 
            fontSize: 14,
            fontFamily: '"JetBrains Mono", monospace',
            theme: { background: '#000000', foreground: '#00ff00', cursor: '#00ff00' },
            // VITAL: DOM renderer is rock-solid on Chromebook ARM64
            rendererType: 'dom' 
        });

        fitAddon = new FitAddon();
        term.loadAddon(fitAddon);
        term.open(termContainer);
        fitAddon.fit();

        const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        socket = new WebSocket(`${proto}//${window.location.host}/ws`);
        
        socket.onopen = () => syncSize();
        socket.onmessage = (e) => term.write(e.data);

        term.onData((d) => { if (socket.readyState === 1) socket.send(d); });

        window.addEventListener('resize', () => {
            fitAddon.fit();
            syncSize();
        });
        
        setTimeout(() => { term.focus(); fitAddon.fit(); syncSize(); }, 500);
    });
</script>

<div class="h-full w-full bg-black overflow-hidden cursor-text" bind:this={termContainer}></div>

<style>
    :global(.xterm) { height: 100%; width: 100%; }
</style>
