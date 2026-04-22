import { terminalState } from '$lib/core/store.svelte.js';

export const TaskService = {
    log(msg, cls = 'text-gray-400') {
        terminalState.logs = [
            ...terminalState.logs.slice(-179),
            { timestamp: new Date().toLocaleTimeString(), message: msg, cls }
        ];
    },

    async runDirective(directive, source = "WORKSPACE") {
        this.log(`>> [${source}] Queuing: ${directive}`, 'text-cyan-400');
        try {
            const res = await fetch('/api/task', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ directive })
            });
            if (res.ok) {
                this.log(`>> [${source}] Task queued — watch terminal.`, 'text-emerald-400');
            } else {
                this.log(`!! [${source}] Server error: ${res.status}`, 'text-rose-400');
            }
        } catch (e) {
            this.log(`!! [${source}] Fault: ${e.message}`, 'text-rose-400');
        }
    }
};
