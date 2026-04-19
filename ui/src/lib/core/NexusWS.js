import { terminalState, wsState, systemVitals, vmoTelemetry } from './store.svelte.js';

export let WS = null;
let ghostTypingTimer = null;

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
    WS = new WebSocket(`${protocol}//${window.location.host}/ws/telemetry`);

    WS.onopen = () => {
        wsState.ready = true;
        pushTermLine('>> [UPLINK] Unified Telemetry Stream Active.', 'text-emerald-500');
    };

    WS.onclose = () => {
        wsState.ready = false;
        WS = null;
        setTimeout(connectWS, 3000);
    };

    WS.onmessage = (event) => {
        let data;
        try { data = JSON.parse(event.data); } catch { return; }

        // ── HEARTBEAT ──────────────────────────────────────────────
        if (data.type_id === 'HEARTBEAT' || data.status === 'SOVEREIGN_MODE_ACTIVE') {
            vmoTelemetry.connected   = true;
            vmoTelemetry.status      = data.status;
            vmoTelemetry.active_vmos = data.active_vmos;
            vmoTelemetry.next_handle = data.next_handle;
            vmoTelemetry.resonance   = data.resonance;

            systemVitals.cpu_load     = data.cpu_load     ?? systemVitals.cpu_load;
            systemVitals.ram_gb       = data.ram_gb       ?? systemVitals.ram_gb;
            systemVitals.total_ram_gb = data.total_ram_gb ?? systemVitals.total_ram_gb;
            systemVitals.temperature  = data.temperature  ?? systemVitals.temperature;
            systemVitals.resonance    = data.resonance    ?? systemVitals.resonance;
            return;
        }

        // ── NEURAL_STREAM — raw token chunks to Ghost Cortex ───────
        if (data.type === 'NEURAL_STREAM') {
            terminalState.isGhostTyping = true;
            clearTimeout(ghostTypingTimer);
            ghostTypingTimer = setTimeout(() => {
                terminalState.isGhostTyping = false;
            }, 1500);
            const text = typeof data.payload === 'string'
                ? data.payload
                : (data.payload?.text ?? '');
            terminalState.buffers.GHOST_CORTEX += text;
            return;
        }

        // ── NEURAL_THOUGHT — clean summary line in terminal ─────────
        if (data.type === 'NEURAL_THOUGHT') {
            const text = typeof data.payload === 'string' ? data.payload : '';
            if (text.trim()) {
                pushTermLine(`>> [THOUGHT] ${text.trim()}`, 'text-purple-400');
            }
            return;
        }

        // ── EXECUTOR — result lines in terminal ─────────────────────
        if (data.type === 'EXECUTOR') {
            const text = typeof data.payload === 'string' ? data.payload : '';
            if (text.trim()) {
                pushTermLine(`>> [RESULT] ${text.trim()}`, 'text-emerald-400');
            }
            return;
        }

        // ── LOG_EVENT — scheduler/supervisor messages ───────────────
        if (data.type === 'LOG_EVENT') {
            const msg = data.payload?.message ?? JSON.stringify(data.payload);
            pushTermLine(`>> [LOG] ${msg}`, 'text-sky-400');
            return;
        }
    };

    return WS;
}
