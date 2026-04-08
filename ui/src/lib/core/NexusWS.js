import { terminalState, wsState, systemVitals } from './store.svelte.js';

export let WS = null;

/**
 * pushTermLine: Inject messages into the visual terminal state.
 * Maintains a lean 100-line buffer for ARM64/4GB stability.
 */
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
    // Prevent redundant handshakes
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
        } catch (e) { 
            // Fallback for raw text tokens if necessary
            return; 
        }

        switch (data.type) {
            case 'ghost_start':
                terminalState.isGhostTyping = true;
                terminalState.ghostBuffer = "";
                break;
            
            case 'ghost_token':
                terminalState.ghostBuffer += data.payload;
                break;

            case 'ghost_end':
                terminalState.isGhostTyping = false;
                pushTermLine(terminalState.ghostBuffer, "text-cyan-100");
                terminalState.ghostBuffer = "";
                break;

            case 'capture_request':
                // --- VISION SUTURE: Handle Pixel-Vision opcode ---
                try {
                    console.log(">> [VISOR] Initiating Pixel-Vision Capture:", data.payload);
                    // Dynamic import prevents SSR/Vite build failures
                    const html2canvas = (await import('html2canvas')).default;
                    const targetElement = document.querySelector(data.payload || 'body');
                    
                    if (targetElement) {
                        const canvas = await html2canvas(targetElement, {
                            backgroundColor: '#0a0a0a',
                            scale: 1, // High-performance low-scale capture
                            logging: false
                        });
                        
                        const snapshot = canvas.toDataURL("image/png");
                        
                        await fetch('/api/fs/write', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({
                                path: `.nexus_vision/${Date.now()}.vision.png`,
                                content: snapshot
                            })
                        });
                        pushTermLine(`>> [VISION] Snapshot secured for: ${data.payload || 'Global Body'}`, "text-sky-400");
                    }
                } catch (err) {
                    console.error("!! [VISION_FAULT]:", err);
                    pushTermLine("!! [VISION_FAULT] Failed to initialize visual capture.", "text-rose-500");
                }
                break;

            case 'telemetry':
                // Update Svelte 5 systemVitals state
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
