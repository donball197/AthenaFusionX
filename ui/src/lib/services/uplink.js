import { authState } from '../stores/auth.svelte.ts';

function handle401(res) {
    if (res.status === 401) {
        console.warn("[SECURITY] 401 Unauthorized detected. Purging token.");
        localStorage.removeItem('nexus_token');
        window.location.reload();
    }
}

export const uplink = {
    async post(path, body) {
        const headers = { 'Content-Type': 'application/json' };
        if (authState.token) {
            headers['Authorization'] = `Bearer ${authState.token}`;
        }
        const res = await fetch(path, { method: 'POST', headers, body: JSON.stringify(body) });
        handle401(res);
        if (!res.ok) throw new Error(`HTTP Error: ${res.status}`);
        return res;
    },
    async json(path) {
        const headers = {};
        if (authState.token) {
            headers['Authorization'] = `Bearer ${authState.token}`;
        }
        const res = await fetch(path, { headers });
        handle401(res);
        if (!res.ok) throw new Error(`HTTP Error: ${res.status}`);
        return await res.json();
    }
};
