<script>
    import { onMount, onDestroy } from 'svelte';
    import '@xterm/xterm/css/xterm.css';

    let termContainer;
    let socket;
    let term;
    let fitAddon;

    onMount(async () => {
        const { Terminal } = await import('@xterm/xterm');
        const { FitAddon } = await import('@xterm/addon-fit');

        term = new Terminal({
            convertEol: true, // This maps \n to \r\n automatically
            cursorBlink: true,
            fontSize: 12,
            fontFamily: '"JetBrains Mono", monospace',
            theme: { background: '#000000', foreground: '#00ff00' }
        });

        fitAddon = new FitAddon();
        term.loadAddon(fitAddon);
        term.open(termContainer);
        setTimeout(() => fitAddon.fit(), 100);

        const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        socket = new WebSocket(`${proto}//${window.location.host}/ws`);

        socket.onmessage = (e) => term.write(e.data);
        term.onData((data) => socket.readyState === 1 && socket.send(data));
        window.addEventListener('resize', () => fitAddon.fit());
    });

    onDestroy(() => {
        if (socket) socket.close();
        if (term) term.dispose();
    });
</script>

<div class="h-full w-full bg-black p-2" bind:this={termContainer}></div>
