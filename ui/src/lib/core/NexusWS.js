import { terminalState, wsState, systemVitals } from './store.svelte.js';

export let WS = null;

export function pushTermLine(message, cls = "text-gray-400") {
    terminalState.logs.push({ 
        message, 
        cls, 
        timestamp: new Date().toLocaleTimeString() 
    });
    if (terminalState.logs.length > 100) terminalState.logs.shift();
}

export async function connectWS() {
    if (typeof window === 'undefined') return null;
    if (WS && (WS.readyState === WebSocket.OPEN || WS.readyState === WebSocket.CONNECTING)) return WS;

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    WS = new WebSocket(`${protocol}//${window.location.host}/ws/nexus`);

    WS.onopen = () => {
        wsState.ready = true;
        pushTermLine(">> [UPLINK] Neural connection established.", "text-emerald-500");
    };

    WS.onclose = () => {
        wsState.ready = false;
        WS = null;
        pushTermLine("!! [OFFLINE] Neural link severed. Retrying in 5s...", "text-rose-500");
        setTimeout(connectWS, 5000);
    };

    WS.onmessage = async (event) => {
        let data;
        try {
            data = JSON.parse(event.data);
        } catch (e) { return; }

        switch (data.type) {
            case 'NEURAL_STREAM':
                terminalState.isGhostTyping = true;
                terminalState.ghostBuffer += data.payload.text;
                break;
            
            case 'LOG_EVENT':
                if (data.payload.message === "Mission Initiated.") {
                    terminalState.ghostBuffer = "";
                }
                pushTermLine(`[${data.payload.level}] ${data.payload.message}`, "text-sky-400");
                break;

            case 'telemetry':
                systemVitals.cpu_load = data.payload.cpu_load;
                systemVitals.ram_gb = data.payload.ram_gb;
                systemVitals.temperature = data.payload.temperature;
                systemVitals.resonance = data.payload.resonance;
                break;

            case 'error':
                pushTermLine(`!! [CORTEX_ERROR] ${data.payload}`, "text-red-500");
                terminalState.isGhostTyping = false;
                break;
        }
    };
    return WS;
}
