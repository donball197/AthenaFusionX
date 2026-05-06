// TaskService.ts — v1.6 [DIRECT_UPLINK + AUTH_AWARE]
import { uplink } from '$lib/services/uplink.js';
import { auth } from '$lib/stores/auth.svelte';

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
        }
    },
};
