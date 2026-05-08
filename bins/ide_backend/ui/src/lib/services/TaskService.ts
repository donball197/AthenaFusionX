// TaskService.ts — v1.7 [LOG_PROXY_FIX]
import { uplink } from '$lib/services/uplink.js';
import { auth } from '$lib/stores/auth.svelte';
import { storeAddLog } from '$lib/core/store.svelte';

const _lastFire: Record<string, number> = {};
const DEBOUNCE_MS = 200;

async function ensureAuth(): Promise<void> {
    if (auth.token) return;
    try {
        const res = await fetch('/api/auth/bootstrap');
        if (res.ok) {
            const data = await res.json();
            auth.token = data.token;
        }
    } catch (e) {
        console.warn('[TaskService] Bootstrap auth failed:', e);
    }
}

export const TaskService = {
    /**
     * log: Centralized proxy for UI components to write to the 
     * terminal log list.
     */
    log(message: string, cls = 'text-gray-400') {
        storeAddLog(message, cls);
    },

    async runDirective(
        directive: string,
        source = 'VISOR',
        signal?: AbortSignal,
    ): Promise<void> {
        const now = Date.now();
        if (now - (_lastFire[source] ?? 0) < DEBOUNCE_MS) return;
        _lastFire[source] = now;

        await ensureAuth();

        try {
            await uplink.post('/api/task', { directive }, signal);
        } catch (e: any) {
            console.error(`!! [${source}] Uplink Fracture: ${e.message}`);
            this.log(`!! [${source}] Uplink Fracture: ${e.message}`, 'text-rose-500');
        }
    },
};
