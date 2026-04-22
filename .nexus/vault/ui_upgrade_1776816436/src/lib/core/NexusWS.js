import { agentRegistry } from './agent_module_registry.svelte.js';

export class NexusWS {
    constructor() {
        this.url = `ws://${window.location.hostname}:3000/ws/telemetry`;
        this.socket = null;
        this.listeners = [];
        this.logHandlers = [];
        this.connect();
    }
    connect() {
        if (this.socket && (this.socket.readyState === WebSocket.OPEN || this.socket.readyState === WebSocket.CONNECTING)) return;
        this.socket = new WebSocket(this.url);
        this.socket.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                // ⚡ V7 HOOK
                if (data.type === "AGENT_MODULE_PROPOSE") agentRegistry.propose(data.payload);
                this.listeners.forEach(cb => cb(data));
            } catch (e) {}
        };
        this.socket.onclose = () => setTimeout(() => this.connect(), 2000);
    }
    onMessage(callback) { this.listeners.push(callback); }
    registerLogHandler(handler) { this.logHandlers.push(handler); }
    send(type, payload) { if (this.socket?.readyState === WebSocket.OPEN) this.socket.send(JSON.stringify({ type, payload })); }
}
export const nexusWS = new NexusWS();
export function connectWS() { nexusWS.connect(); return nexusWS; }
export function pushTermLine(message, cls = '') { nexusWS.logHandlers.forEach(h => h(message, cls)); }
