// socket.ts — v2.1
// FIX: Added .telemetry property so +page.svelte references don't silently explode
// FIX: Capped retries at 15 (was already there, kept)

type Listener = (msg: any) => void;

export interface Telemetry {
    status:      string;
    active_vmos: number;
    resonance:   number;
}

class NexusSocket {
    private socket:       WebSocket | null = null;
    private url:          string;
    private listeners:    Set<Listener> = new Set();
    private retries     = 0;
    private retryTimer?: ReturnType<typeof setTimeout>;
    private watchdogTimer?: ReturnType<typeof setInterval>;
    private lastHeartbeat = Date.now();

    public status:    'DISCONNECTED' | 'CONNECTED' = 'DISCONNECTED';
    public telemetry: Telemetry = { status: 'IDLE', active_vmos: 0, resonance: 0 };

    constructor(path: string) {
        if (typeof window === 'undefined') {
            this.url = `ws://localhost:3000${path}`;
        } else {
            const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            this.url = `${proto}//${window.location.host}${path}`;
        }
    }

    connect() {
        if (this.socket?.readyState === WebSocket.OPEN || this.socket?.readyState === WebSocket.CONNECTING) return;
        this.socket = new WebSocket(this.url);

        this.socket.onopen = () => {
            this.status = 'CONNECTED';
            this.telemetry.status = 'CONNECTED';
            this.retries = 0;
            this.lastHeartbeat = Date.now();
            this.emit({ _internal: 'CONNECTION_STATE', status: 'CONNECTED' });
            this.startWatchdog();
        };

        this.socket.onmessage = (e) => {
            this.lastHeartbeat = Date.now();
            try {
                const msg = JSON.parse(e.data);
                if (!msg.type && !msg.type_id && msg._internal === undefined) return;
                // Mirror telemetry fields
                if (msg.active_vmos !== undefined) this.telemetry.active_vmos = msg.active_vmos;
                if (msg.resonance   !== undefined) this.telemetry.resonance   = msg.resonance;
                this.emit(msg);
            } catch {
                console.warn('[SOCKET] Dropped invalid JSON frame');
            }
        };

        this.socket.onclose = () => {
            this.status = 'DISCONNECTED';
            this.telemetry.status = 'DISCONNECTED';
            this.emit({ _internal: 'CONNECTION_STATE', status: 'DISCONNECTED' });
            clearInterval(this.watchdogTimer);
            if (this.retries < 15) this.retries++;
            const delay = Math.min(1000 * 2 ** this.retries, 16000);
            this.retryTimer = setTimeout(() => this.connect(), delay);
        };
    }

    send(payload: any) {
        if (this.socket?.readyState === WebSocket.OPEN) {
            const data = typeof payload === 'string' ? { directive: payload } : payload;
            this.socket.send(JSON.stringify(data));
        }
    }

    subscribe(fn: Listener) {
        this.listeners.add(fn);
        return () => this.listeners.delete(fn);
    }

    private emit(msg: any) { this.listeners.forEach(l => l(msg)); }

    private startWatchdog() {
        clearInterval(this.watchdogTimer);
        this.watchdogTimer = setInterval(() => {
            if (this.status === 'CONNECTED' && Date.now() - this.lastHeartbeat > 15000) {
                console.warn('[WATCHDOG] Socket frozen — forcing reconnect.');
                this.socket?.close();
            }
        }, 5000);
    }

    destroy() {
        clearTimeout(this.retryTimer);
        clearInterval(this.watchdogTimer);
        this.socket?.close();
        this.listeners.clear();
    }
}

export const nexusSocket = new NexusSocket('/ws/telemetry');
