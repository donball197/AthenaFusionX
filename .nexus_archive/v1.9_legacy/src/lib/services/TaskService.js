import { nexusSocket } from '../nexus_socket.svelte.ts';

export const TaskService = {
    async runDirective(directive, source = "WORKSPACE") {
        nexusSocket.pushLog(`>> [${source}] Queuing: ${directive}`, 'text-cyan-400');
        try {
            const res = await fetch('/api/task', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ directive })
            });
            if (res.ok) {
                nexusSocket.pushLog(`>> [${source}] Task queued — watch terminal.`, 'text-emerald-400');
            } else {
                nexusSocket.pushLog(`!! [${source}] Server error: ${res.status}`, 'text-rose-400');
            }
        } catch (e) {
            nexusSocket.pushLog(`!! [${source}] Fault: ${e.message}`, 'text-rose-400');
        }
    },
    
    log(msg, cls) {
        nexusSocket.pushLog(msg, cls);
    }
};
