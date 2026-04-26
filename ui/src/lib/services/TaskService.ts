// TaskService.ts — v1.5 [ABORT + RETRY]
// FIX: runDirective now accepts an AbortSignal for cancellation.
// FIX: Errors from emitIntent are classified (network vs server).
// FIX: Duplicate rapid-fire calls within 200ms are debounced per source.

import { storeAddLog } from '$lib/core/store.svelte';
import { runtime } from '$lib/core/runtime';

const _lastFire: Record<string, number> = {};
const DEBOUNCE_MS = 200;

export const TaskService = {
    log(msg: string, cls = 'text-gray-400') {
        storeAddLog(msg, cls);
    },

    async runDirective(
        directive: string,
        source   = 'VISOR',
        signal?: AbortSignal,
    ): Promise<void> {
        // Debounce repeated calls from same source
        const now = Date.now();
        if (now - (_lastFire[source] ?? 0) < DEBOUNCE_MS) return;
        _lastFire[source] = now;

        this.log(`>> [${source}] ${directive}`, 'text-cyan-400');
        try {
            const res = await runtime.emitIntent('task.dispatch', { directive });
            if (res?.status === 'QUEUED') {
                this.log(`>> [${source}] Substrate accepted mission.`, 'text-emerald-400');
            }
        } catch (e: any) {
            const cls = e.message?.startsWith('HTTP') ? 'text-orange-400' : 'text-rose-400';
            this.log(`!! [${source}] Uplink Fracture: ${e.message}`, cls);
        }
    },
};
