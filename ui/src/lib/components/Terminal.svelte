<script>
    import { onMount } from 'svelte';
    import '@xterm/xterm/css/xterm.css';

    let termContainer;
    let socket;
    let term;
    let fitAddon;

    export function getLogs() {
        if (!term) return "";
        const totalLines = term.buffer.active.length;
        let logs = "";
        for (let i = Math.max(0, totalLines - 30); i < totalLines; i++) {
            const line = term.buffer.active.getLine(i);
            if (line) logs += line.translateToString() + "\n";
        }
        return logs;
    }

    export function fit() {
        if (fitAddon && term) {
            setTimeout(() => {
                fitAddon.fit();
                term.focus();
            }, 50);
        }
    }

    onMount(async () => {
        const { Terminal } = await import('@xterm/xterm');
        const { FitAddon } = await import('@xterm/addon-fit');

        term = new Terminal({
            cursorBlink: true,
            fontSize: 14, 
            fontFamily: '"JetBrains Mono", monospace',
            theme: { background: '#000000', foreground: '#00ff00', cursor: '#00ff00' },
            rendererType: 'canvas'
        });

        fitAddon = new FitAddon();
        term.loadAddon(fitAddon);
        term.open(termContainer);
        term.element.style.padding = '0';
        fitAddon.fit();

        termContainer.addEventListener('click', () => term.focus());
        
        // --- NATIVE MODE ---
        // We removed ALL custom interceptors. 
        // Right-Click now opens the browser's native Copy/Paste menu.
        // This is 100% reliable.

        const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        socket = new WebSocket(`${proto}//${window.location.host}/ws`);
        socket.onmessage = (event) => term.write(event.data);

        term.onData((data) => {
            if (socket.readyState === WebSocket.OPEN) socket.send(data);
        });

        window.addEventListener('resize', () => fitAddon.fit());
        setTimeout(() => { term.focus(); fitAddon.fit(); }, 500);
    });
</script>

<div class="h-full w-full bg-black overflow-hidden cursor-text" bind:this={termContainer}></div>

<style>
    :global(.xterm) { height: 100%; width: 100%; }
</style>
