// ════════════════════════════════════════════════════════════════
//  NEXUS SINGULARITY — WebSocket Manager v6.0.0
// ════════════════════════════════════════════════════════════════
import { terminalState, wsState, systemVitals } from './store.svelte.js';

export let WS = null;

// Push a timestamped line into the terminal log
export function pushTermLine(message, cls = 'text-gray-400') {
    if (!terminalState.logs) terminalState.logs = [];
    terminalState.logs.push({
        message,
        cls,
        timestamp: new Date().toLocaleTimeString('en-US', { hour12: false }),
    });
    if (terminalState.logs.length > 200) terminalState.logs.shift();
}

export async function connectWS() {
    if (typeof window === 'undefined') return null;
    if (WS && (WS.readyState === WebSocket.OPEN || WS.readyState === WebSocket.CONNECTING)) {
        return WS;
    }

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    WS = new WebSocket(`${protocol}//${window.location.host}/ws/nexus`);

    WS.onopen = () => {
        wsState.ready = true;
        pushTermLine('>> [UPLINK] Neural connection established.', 'text-emerald-500');
    };

    WS.onclose = () => {
        wsState.ready = false;
        WS = null;
        pushTermLine('!! [OFFLINE] Neural link severed — reconnecting...', 'text-rose-500');
        // Auto-reconnect after 3 seconds
        setTimeout(connectWS, 3000);
    };

    WS.onerror = () => {
        wsState.ready = false;
    };

    WS.onmessage = (event) => {
        let data;
        try { data = JSON.parse(event.data); } catch { return; }

        switch (data.type) {
            case 'NEURAL_STREAM':
                terminalState.isGhostTyping = true;
                const sector = data.payload?.sector ?? 'GHOST_CORTEX';
                if (!terminalState.buffers[sector]) terminalState.buffers[sector] = '';
                terminalState.buffers[sector] += data.payload?.text ?? '';
                break;

            case 'telemetry':
                systemVitals.cpu_load    = data.payload.cpu_load    ?? 0;
                systemVitals.ram_gb      = data.payload.ram_gb      ?? 0;
                systemVitals.temperature = data.payload.temperature ?? 0;
                systemVitals.resonance   = data.payload.resonance   ?? 0;
                terminalState.isGhostTyping = false;
                break;

            case 'LOG_EVENT':
                if (data.payload?.message === 'Mission Initiated.') {
                    terminalState.buffers.GHOST_CORTEX = '';
                }
                pushTermLine(
                    `[${data.payload?.level ?? 'INFO'}] ${data.payload?.message ?? ''}`,
                    'text-sky-400'
                );
                break;
        }
    };

    return WS;
}
