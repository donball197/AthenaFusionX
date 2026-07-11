import { authState } from '../stores/auth.svelte.ts';

export const uplink = {
    async post(path, body) {
        const headers = { 'Content-Type': 'application/json' };
        if (authState.token) {
            headers['Authorization'] = `Bearer ${authState.token}`;
        }
        
        const res = await fetch(path, {
            method: 'POST',
            headers,
            body: JSON.stringify(body)
        });
        
        if (!res.ok) throw new Error(`HTTP Error: ${res.status}`);
        return res;
    },
    
    async json(path) {
        const headers = {};
        if (authState.token) {
            headers['Authorization'] = `Bearer ${authState.token}`;
        }
        
        const res = await fetch(path, { headers });
        if (!res.ok) throw new Error(`HTTP Error: ${res.status}`);
        return await res.json();
    }
};
