// nexus_socket.svelte.ts — v2.1 (fixed export for +layout.svelte)

function getWsUrl(path = '/ws/telemetry') {
    if (typeof window === 'undefined') return `ws://localhost:3000${path}`;
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}${path}`;
}

let socketInstance = null;

export function createNexusSocket(path = '/ws/telemetry') {
    if (socketInstance) return socketInstance;

    let socket = null;
    let reconnectTimer = null;
    let _retries = 0;

    const telemetry = $state({
        connected: false,
        status: 'DISCONNECTED',
        active_vmos: 0,
        next_handle: '0x00',
        resonance: 0.0,
        live_logs: []
    });

    function dispatchGenesis(text) {
        if (!text.includes('GENESIS_PAYLOAD:')) return false;
        const b64 = text.split('GENESIS_PAYLOAD:')[1].trim();
        if (typeof window !== 'undefined') {
            window.dispatchEvent(new CustomEvent('nexus-genesis', { detail: b64 }));
        }
        return true;
    }

    function pushLog(line) {
        telemetry.live_logs = [...telemetry.live_logs.slice(-59), line];
    }

    function connect() {
        const url = getWsUrl(path);
        socket = new WebSocket(url);

        socket.onopen = () => {
            telemetry.connected = true;
            telemetry.status = 'SOVEREIGN_MODE_ACTIVE';
            _retries = 0;
        };

        socket.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);

                if (data.type_id === 'HEARTBEAT' || data.status === 'SOVEREIGN_MODE_ACTIVE') {
                    telemetry.status = data.status ?? telemetry.status;
                    telemetry.active_vmos = data.active_vmos ?? 0;
                    telemetry.next_handle = data.next_handle ?? '0x00';
                    telemetry.resonance = data.resonance ?? 0;
                    return;
                }

                const text = typeof data.payload === 'string' ? data.payload : '';

                switch (data.type) {
                    case 'NEURAL_STREAM':
                        if (text) pushLog(text);
                        break;
                    case 'NEURAL_THOUGHT':
                    case 'EXECUTOR':
                        if (dispatchGenesis(text)) return;
                        if (text.trim()) pushLog(`[${data.type}] ${text.trim()}`);
                        break;
                    default:
                        if (text.trim()) pushLog(text);
                }
            } catch {
                const raw = String(event.data).trim();
                if (raw) pushLog(raw);
            }
        };

        socket.onclose = () => {
            telemetry.connected = false;
            telemetry.status = 'DISCONNECTED';
            const delay = Math.min(1000 * Math.pow(2, _retries), 16000);
            _retries++;
            reconnectTimer = setTimeout(connect, delay);
        };

        socket.onerror = () => {
            telemetry.connected = false;
        };
    }

    connect();

    return (socketInstance = {
        telemetry,
        send: (cmd) => socket?.readyState === WebSocket.OPEN && socket.send(cmd),
        destroy: () => {
            clearTimeout(reconnectTimer);
            socket?.close();
            socket = null;
        },
        connect  // expose connect for +layout.svelte
    });
}

// Legacy export for +layout.svelte (creates singleton on first import)
export const nexusSocket = createNexusSocket();
