// TaskService.js — v1.2 [SOVEREIGN SECURED]
import { storeAddLog } from '$lib/core/store.svelte.js';
import { uplink } from '$lib/services/uplink.js';

export const TaskService = {
    log(msg, cls = 'text-gray-400') {
        storeAddLog(msg, cls);
    },

    async runDirective(directive, source = 'TERMINAL') {
        this.log(`>> [${source}] Queuing: ${directive}`, 'text-cyan-400');
        try {
            // ⚡ SUTURE: Using the signed uplink instead of raw fetch
            const res = await uplink.post('/api/task', { directive });
            if (res.ok) {
                this.log(`>> [${source}] Task queued — responses appear below.`, 'text-emerald-400');
            } else {
                this.log(`!! [${source}] Shield Error: ${res.status}`, 'text-rose-400');
            }
        } catch (e) {
            this.log(`!! [${source}] Network fault: ${e.message}`, 'text-rose-400');
        }
    }
};
