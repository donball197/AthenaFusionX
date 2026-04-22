// nexus_socket.svelte.ts — v2.0
// FIX: Do not call createNexusSocket at module level — it opens a WS
//      immediately. Call it inside onMount in the consuming component.
// FIX: GENESIS_PAYLOAD dispatched for both NEURAL_THOUGHT and EXECUTOR
//      message types (previously only NEURAL_THOUGHT was checked).

function getWsUrl(path: string): string {
    if (typeof window === 'undefined') return `ws://localhost:3000${path}`;
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}${path}`;
}

export function createNexusSocket(path = '/ws/telemetry') {
    let socket: WebSocket | null = null;
    let reconnectTimer: ReturnType<typeof setTimeout>;
    let _retries = 0;

    const telemetry = $state({
        connected:   false,
        status:      'DISCONNECTED',
        active_vmos: 0,
        next_handle: '0x00',
        resonance:   0.0,
        live_logs:   [] as string[],
    });

    function dispatchGenesis(text: string) {
        if (!text.includes('GENESIS_PAYLOAD:')) return false;
        const b64 = text.split('GENESIS_PAYLOAD:')[1].trim();
        if (typeof window !== 'undefined') {
            window.dispatchEvent(new CustomEvent('nexus-genesis', { detail: b64 }));
        }
        return true;
    }

    function pushLog(line: string) {
        telemetry.live_logs = [...telemetry.live_logs.slice(-59), line];
    }

    function connect() {
        const url = getWsUrl(path);
        socket = new WebSocket(url);

        socket.onopen = () => {
            telemetry.connected = true;
            telemetry.status    = 'SOVEREIGN_MODE_ACTIVE';
            _retries = 0;
        };

        socket.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);

                // HEARTBEAT — update telemetry state
                if (data.type_id === 'HEARTBEAT' || data.status === 'SOVEREIGN_MODE_ACTIVE') {
                    telemetry.status      = data.status      ?? telemetry.status;
                    telemetry.active_vmos = data.active_vmos ?? 0;
                    telemetry.next_handle = data.next_handle ?? '0x00';
                    telemetry.resonance   = data.resonance   ?? 0;
                    return;
                }

                const text = typeof data.payload === 'string' ? data.payload : '';

                switch (data.type) {
                    case 'NEURAL_STREAM':
                        if (text) pushLog(text);
                        break;

                    case 'NEURAL_THOUGHT':
                        // FIX: check genesis before logging thought
                        if (dispatchGenesis(text)) return;
                        if (text.trim()) pushLog(`[THOUGHT] ${text.trim()}`);
                        break;

                    case 'EXECUTOR':
                        // FIX: EXECUTOR type also carries genesis payloads
                        if (dispatchGenesis(text)) return;
                        if (text.trim()) pushLog(`[RESULT] ${text.trim()}`);
                        break;

                    default:
                        if (text.trim()) pushLog(text);
                }
            } catch {
                // Non-JSON frame — display as raw text
                const raw = String(event.data).trim();
                if (raw) pushLog(raw);
            }
        };

        socket.onclose = () => {
            telemetry.connected = false;
            telemetry.status    = 'DISCONNECTED';
            // Exponential backoff: 1s → 2s → 4s → max 16s
            const delay = Math.min(1000 * Math.pow(2, _retries), 16000);
            _retries++;
            reconnectTimer = setTimeout(connect, delay);
        };

        socket.onerror = () => {
            telemetry.connected = false;
        };
    }

    connect();

    return {
        telemetry,
        send: (cmd: string) => {
            if (socket?.readyState === WebSocket.OPEN) {
                socket.send(cmd);
            } else {
                console.warn('[NexusSocket] Cannot send — socket not open');
            }
        },
        destroy: () => {
            clearTimeout(reconnectTimer);
            socket?.close();
            socket = null;
        },
    };
}
