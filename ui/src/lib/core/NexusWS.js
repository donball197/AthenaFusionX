// ui/src/lib/core/NexusWS.js — v3.4 Stateless Relay
export class NexusWS {
    constructor() {
        this.url = `ws://${window.location.hostname}:3000/ws/telemetry`;
        this.socket = null;
        this.listeners = [];
        this.logHandlers = [];
        this.connect();
    }
    connect() {
        if (this.socket && this.socket.readyState <= 1) return;
        this.socket = new WebSocket(this.url);
        this.socket.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                this.listeners.forEach(cb => cb(data));
            } catch (e) {
                console.error(">> [WS_FRACTURE]: Malformed packet", e);
            }
        };
        this.socket.onclose = () => {
            console.warn(">> [WS_DROPOUT]: Reconnecting substrate...");
            setTimeout(() => this.connect(), 1500);
        };
    }
    onMessage(callback) {
        this.listeners.push(callback);
    }
    registerLogHandler(handler) {
        this.logHandlers.push(handler);
    }
    send(type, payload) {
        if (this.socket?.readyState === WebSocket.OPEN) {
            this.socket.send(JSON.stringify({ type, payload }));
        }
    }
}

export const nexusWS = new NexusWS();
export function connectWS() {
    nexusWS.connect();
    return nexusWS;
}

// ⚡ SUTURE: Relay function that components use
export function pushTermLine(message, cls = '') {
    nexusWS.logHandlers.forEach(handler => handler(message, cls));
}
