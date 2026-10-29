export type BeliefState = { impl: number; qual: number; risk: number; sec: number; scalar: number };
export type DagNode = { id: string; objective: string; status: string };

export class TelemetryStore {
    logs = $state<string[]>([]);
    vitals = $state({ cpu: 0, ram: 0, temp: 0 });
    connected = $state(false);
    
    // ⚡ Phase 6 Glass Terminal States
    belief = $state<BeliefState>({ impl: 0, qual: 0, risk: 0, sec: 0, scalar: 0 });
    dagNodes = $state<Record<string, DagNode>>({});

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
                } else if (data.type === "BELIEF_STATE") {
                    this.belief = { impl: data.impl, qual: data.qual, risk: data.risk, sec: data.sec, scalar: data.scalar };
                } else if (data.type === "DAG_STATE") {
                    if (data.objective) {
                        this.dagNodes[data.id] = { id: data.id, objective: data.objective, status: data.status };
                    } else if (this.dagNodes[data.id]) {
                        this.dagNodes[data.id].status = data.status;
                    }
                } else {
                    this.logs = [...this.logs, data.payload];
                    if (this.logs.length > 200) this.logs.shift(); // Keep logs manageable
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
