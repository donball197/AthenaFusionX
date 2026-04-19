function getWsUrl(path: string): string {
    if (typeof window === 'undefined') return `ws://localhost:3000${path}`;
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}${path}`;
}

export function createNexusSocket(path = '/ws/telemetry') {
    let socket: WebSocket | null = null;
    let reconnectTimer: ReturnType<typeof setTimeout>;

    const telemetry = $state({
        connected:   false,
        status:      'DISCONNECTED',
        active_vmos: 0,
        next_handle: '0x00',
        resonance:   0.0,
        live_logs:   [] as string[],
    });

    function connect() {
        const url = getWsUrl(path);
        socket = new WebSocket(url);

        socket.onopen = () => {
            telemetry.connected = true;
            telemetry.status    = 'SOVEREIGN_MODE_ACTIVE';
        };

        socket.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                if (data.type_id === 'HEARTBEAT' || data.status === 'SOVEREIGN_MODE_ACTIVE') {
                    telemetry.status      = data.status      ?? telemetry.status;
                    telemetry.active_vmos = data.active_vmos ?? 0;
                    telemetry.next_handle = data.next_handle ?? '0x00';
                    telemetry.resonance   = data.resonance   ?? 0;
                } else if (data.type === 'NEURAL_STREAM') {
                    const text = typeof data.payload === 'string'
                        ? data.payload
                        : (data.payload?.text ?? '');
                    if (text) telemetry.live_logs = [...telemetry.live_logs.slice(-59), text];
                } else if (data.type === 'NEURAL_THOUGHT') {
                    const text = typeof data.payload === 'string' ? data.payload : '';
                    if (text.trim()) {
                        telemetry.live_logs = [
                            ...telemetry.live_logs.slice(-59),
                            `[THOUGHT] ${text.trim()}`
                        ];
                    }
                } else if (data.type === 'EXECUTOR') {
                    const text = typeof data.payload === 'string' ? data.payload : '';
                    if (text.trim()) {
                        telemetry.live_logs = [
                            ...telemetry.live_logs.slice(-59),
                            `[RESULT] ${text.trim()}`
                        ];
                    }
                }
            } catch {
                telemetry.live_logs = [...telemetry.live_logs.slice(-59), event.data];
            }
        };

        socket.onclose = () => {
            telemetry.connected = false;
            telemetry.status    = 'DISCONNECTED';
            reconnectTimer = setTimeout(connect, 1500);
        };
        socket.onerror = () => { telemetry.connected = false; };
    }

    connect();

    return {
        telemetry,
        // ⚡ FIX: was sendCommand — correct method is send()
        send: (cmd: string) => {
            if (socket?.readyState === WebSocket.OPEN) socket.send(cmd);
        },
        destroy: () => { clearTimeout(reconnectTimer); socket?.close(); }
    };
}
