export type Node = { id: string; type: string; capacity: number; load: number; lastHeartbeat: number; };

export const nodeRegistry = {
    nodes: new Map<string, Node>(),
    register(n: Node) { this.nodes.set(n.id, n); },
    getAvailable(): Node[] { 
        return Array.from(this.nodes.values() as IterableIterator<Node>)
            .filter((n) => Date.now() - n.lastHeartbeat < 5000); 
    }
};

export const kernel_dna: Record<string, any> = {
    "logic_audit": { id: "logic_audit", label: "Logic Integrity Sentinel", synapses: ["vmo_manager"] },
    "thermal_monitor": { id: "thermal_monitor", label: "Thermal Governor", synapses: ["sensor_array"] }
};

export type KernelProcess = { cell: any; nodeId: string; state: string; };
export const cognitiveKernel = {
    registry: new Map<string, KernelProcess>(),
    ignite(id: string) {
        const nodes = nodeRegistry.getAvailable();
        if (nodes.length > 0 && kernel_dna[id]) {
            this.registry.set(id, { cell: kernel_dna[id], nodeId: nodes[0].id, state: "active" });
        }
    },
    snapshot(): KernelProcess[] { return Array.from(this.registry.values() as IterableIterator<KernelProcess>); }
};
