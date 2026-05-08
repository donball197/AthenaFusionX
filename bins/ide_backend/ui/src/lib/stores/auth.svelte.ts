import { uplink } from '$lib/services/uplink.js';

// Added isInitialized to prevent Handshake hang
export const auth = $state({ 
    token: '', 
    isInitialized: false 
});

export async function bootAuth(): Promise<void> {
    if (auth.token) {
        auth.isInitialized = true;
        return;
    }
    try {
        const res = await fetch('/api/auth/bootstrap');
        if (res.ok) {
            const data = await res.json();
            auth.token = data.token;
        }
    } catch (e) {
        console.warn('[AUTH] Bootstrap failed:', e);
    } finally {
        // Handshake clears regardless of success (allows login page to show)
        auth.isInitialized = true;
    }
}

export async function login(key: string): Promise<boolean> {
    try {
        const res = await uplink.post('/api/auth/login', { key });
        const data = await res.json();
        if (data.token) {
            auth.token = data.token;
            return true;
        }
        return false;
    } catch {
        return false;
    }
}
