export class TelemetryStore {
    logs = $state<string[]>([]);
    vitals = $state({ cpu: 0, ram: 0, temp: 0 });
    connected = $state(false);
    graphNodes = $state<Array<{id: string, kind: string, content: string, belief: number}>>([]);
    private ws: WebSocket | null = null;

    constructor() {
        this.connect();
    }

    private connect() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.host}/ws/telemetry`;
        
        this.ws = new WebSocket(wsUrl);
        
        this.ws.onopen = () => { 
            this.connected = true; 
            console.log("[UPLINK] WebSocket connected.");
        };
        
        this.ws.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                if (data.type === "VITALS") {
                    this.vitals = { cpu: data.cpu_load, ram: data.ram_gb, temp: data.temperature };
                } else if (data.type === "GRAPH_UPDATE") {
                    this.graphNodes = data.nodes;
                } else {
                    this.logs = [...this.logs, data.payload];
                    if (this.logs.length > 100) this.logs.shift();
                }
            } catch (e) {
                console.error("Failed to parse telemetry:", e);
            }
        };
        
        this.ws.onclose = () => { 
            this.connected = false; 
            console.warn("[UPLINK] Connection lost. Reconnecting in 3s...");
            setTimeout(() => this.connect(), 3000);
        };
    }
}

export const telemetry = new TelemetryStore();
