// nexus_socket.svelte.ts — v3.0 AUTHORITATIVE SINGLETON
// THE FIX: This is now the single WebSocket instance for the entire app.
// store.svelte.js imports this and wires reactive state from it.
// NexusWS.js is a thin compatibility shim — it does NOT open a socket.
//
// Message routing:
//   HEARTBEAT      → telemetry.$state + onMessageHandlers
//   NEURAL_STREAM  → terminalState.buffers.GHOST_CORTEX (via handlers)
//   NEURAL_THOUGHT → terminalState.logs (via handlers)
//   TASK_STATUS    → terminalState.logs (via handlers)
//   KERNEL_GENESIS → window.dispatchEvent('nexus-genesis')
//   EXECUTOR       → GENESIS check + logs

function getWsUrl(path = '/ws/telemetry'): string {
    if (typeof window === 'undefined') return `ws://localhost:3000${path}`;
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}${path}`;
}

// ── Message handler registry ─────────────────────────────────────────
type MsgHandler = (data: Record<string, any>) => void;
const _msgHandlers: MsgHandler[] = [];

export function onSocketMessage(fn: MsgHandler) {
    _msgHandlers.push(fn);
}

// ── Genesis dispatch ─────────────────────────────────────────────────
function dispatchGenesis(text: string): boolean {
    if (!text.includes('GENESIS_PAYLOAD:')) return false;
    const b64 = text.split('GENESIS_PAYLOAD:')[1].trim();
    if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent('nexus-genesis', { detail: b64 }));
    }
    return true;
}

// ── Socket state ─────────────────────────────────────────────────────
let _socket: WebSocket | null = null;
let _reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let _retries = 0;

export const telemetry = $state({
    connected:   false,
    status:      'DISCONNECTED',
    active_vmos: 0,
    next_handle: '0x00',
    resonance:   0.0,
    cpu_load:    0.0,
    ram_gb:      0.0,
    total_ram_gb: 4.0,
    temperature: 0.0,
    live_logs:   [] as string[],
});

function pushLiveLog(line: string) {
    telemetry.live_logs = [...telemetry.live_logs.slice(-59), line];
}

export function connect(path = '/ws/telemetry') {
    if (_socket && (_socket.readyState === WebSocket.OPEN ||
                    _socket.readyState === WebSocket.CONNECTING)) return;

    const url = getWsUrl(path);
    _socket = new WebSocket(url);

    _socket.onopen = () => {
        telemetry.connected = true;
        telemetry.status    = 'CONNECTED';
        _retries = 0;
        console.log('[NexusSocket] Connected');
    };

    _socket.onmessage = (event: MessageEvent) => {
        let data: Record<string, any>;
        try {
            data = JSON.parse(event.data);
        } catch {
            pushLiveLog(String(event.data));
            return;
        }

        // 1. Dispatch to store.svelte.js and all registered handlers
        _msgHandlers.forEach(h => {
            try { h(data); } catch(e) { console.warn('[NexusSocket] handler error', e); }
        });

        // 2. Update telemetry $state directly (HEARTBEAT)
        if (data.type_id === 'HEARTBEAT') {
            telemetry.active_vmos = data.active_vmos ?? telemetry.active_vmos;
            telemetry.next_handle = data.next_handle ?? telemetry.next_handle;
            telemetry.resonance   = data.resonance   ?? telemetry.resonance;
            telemetry.cpu_load    = data.cpu_load    ?? telemetry.cpu_load;
            telemetry.ram_gb      = data.ram_gb      ?? telemetry.ram_gb;
            telemetry.total_ram_gb= data.total_ram_gb?? telemetry.total_ram_gb;
            telemetry.temperature = data.temperature ?? telemetry.temperature;
            return;
        }

        // 3. GENESIS payloads
        const text = typeof data.payload === 'string' ? data.payload : '';
        if (data.type === 'NEURAL_THOUGHT' || data.type === 'EXECUTOR') {
            if (dispatchGenesis(text)) return;
        }

        // 4. Live log for terminal panel
        if (text.trim()) {
            pushLiveLog(text);
        }
    };

    _socket.onclose = () => {
        telemetry.connected = false;
        telemetry.status    = 'DISCONNECTED';
        const delay = Math.min(1000 * Math.pow(2, _retries), 16000);
        _retries++;
        _reconnectTimer = setTimeout(() => connect(path), delay);
    };

    _socket.onerror = () => {
        telemetry.connected = false;
    };
}

export function send(cmd: string): boolean {
    if (_socket?.readyState === WebSocket.OPEN) {
        _socket.send(cmd);
        return true;
    }
    return false;
}

export function destroy() {
    if (_reconnectTimer) clearTimeout(_reconnectTimer);
    _socket?.close();
    _socket = null;
}

// ── Singleton export for +layout.svelte ─────────────────────────────
export const nexusSocket = { telemetry, connect, send, destroy };
