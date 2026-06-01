export class TelemetryStore {
    logs = $state<string[]>([]);
    vitals = $state({ cpu: 0, ram: 0, temp: 0 });
    connected = $state(false);

    constructor() {
        this.connect();
    }

    private connect() {
        const host = window.location.hostname;
        const ws = new WebSocket("ws://" + host + ":3000/ws/telemetry");
        
        ws.onopen = () => { this.connected = true; };
        ws.onmessage = (event) => {
            const data = JSON.parse(event.data);
            this.logs = [...this.logs, data.payload];
            if (this.logs.length > 50) this.logs.shift();
        };
        ws.onclose = () => { this.connected = false; };
    }
}

export const telemetry = new TelemetryStore();
