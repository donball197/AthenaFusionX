// NexusWS.js — v2.0 Hardened Message Bus
// FIX: Silent catch replaced with console.warn for parse errors.
// FIX: GENESIS_PAYLOAD dispatch added — shared store now fires WASM events.
// FIX: Exponential backoff on reconnect (was fixed 2000ms).
import { agentRegistry } from './agent_module_registry.svelte.js';

export class NexusWS {
    constructor() {
        this.url        = `ws://${window.location.hostname}:3000/ws/telemetry`;
        this.socket     = null;
        this.listeners  = [];
        this.logHandlers= [];
        this._retries   = 0;
        this._retryTimer= null;
        this.connect();
    }

    connect() {
        if (
            this.socket &&
            (this.socket.readyState === WebSocket.OPEN ||
             this.socket.readyState === WebSocket.CONNECTING)
        ) return;

        this.socket = new WebSocket(this.url);

        this.socket.onopen = () => {
            this._retries = 0;
        };

        this.socket.onmessage = (event) => {
            let data;
            try {
                data = JSON.parse(event.data);
            } catch (e) {
                // FIX: was silent — now warns with first 120 chars for debugging
                console.warn('[NexusWS] Parse fault:', e.message,
                    '|', event.data.slice(0, 120));
                return;
            }

            // FIX: GENESIS_PAYLOAD dispatch — CognitiveMesh receives WASM payloads
            // regardless of which socket instance is in use.
            if (data.type === 'NEURAL_THOUGHT' || data.type === 'EXECUTOR') {
                const text = typeof data.payload === 'string' ? data.payload : '';
                if (text.includes('GENESIS_PAYLOAD:')) {
                    const b64 = text.split('GENESIS_PAYLOAD:')[1].trim();
                    if (typeof window !== 'undefined') {
                        window.dispatchEvent(
                            new CustomEvent('nexus-genesis', { detail: b64 })
                        );
                    }
                    return;
                }
            }

            // V7 HOOK — dynamic module proposals
            if (data.type === 'AGENT_MODULE_PROPOSE') {
                agentRegistry.propose(data.payload);
            }

            this.listeners.forEach(cb => cb(data));
        };

        this.socket.onclose = () => {
            // Exponential backoff: 1s, 2s, 4s, 8s, max 16s
            const delay = Math.min(1000 * Math.pow(2, this._retries), 16000);
            this._retries++;
            this._retryTimer = setTimeout(() => this.connect(), delay);
        };

        this.socket.onerror = (e) => {
            console.warn('[NexusWS] Socket error:', e);
        };
    }

    disconnect() {
        clearTimeout(this._retryTimer);
        this.socket?.close();
        this.socket = null;
    }

    onMessage(callback)        { this.listeners.push(callback); }
    registerLogHandler(handler){ this.logHandlers.push(handler); }

    send(type, payload) {
        if (this.socket?.readyState === WebSocket.OPEN) {
            this.socket.send(JSON.stringify({ type, payload }));
        } else {
            console.warn('[NexusWS] Cannot send — socket not open');
        }
    }
}

export const nexusWS    = new NexusWS();
export function connectWS()     { nexusWS.connect(); return nexusWS; }
export function pushTermLine(message, cls = '') {
    nexusWS.logHandlers.forEach(h => h(message, cls));
}
