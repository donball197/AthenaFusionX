// auth.svelte.ts — v1.2 [HANDSHAKE_LOGGING]
export const auth = $state({
    token:           null as string | null,
    isAuthenticated: false,
});

const SESSION_KEY = 'nexus_session_token';

export function restoreSession() {
    const stored = sessionStorage.getItem(SESSION_KEY);
    if (stored) {
        auth.token = stored;
        auth.isAuthenticated = true;
        console.log(">> [AUTH] Session restored from storage.");
    }
}

export async function login(key: string): Promise<boolean> {
    try {
        const res = await fetch('/api/auth/login', {
            method:  'POST',
            headers: { 'Content-Type': 'application/json' },
            body:    JSON.stringify({ key }),
        });
        if (!res.ok) {
            console.error(`!! [AUTH] Login rejected: HTTP ${res.status}`);
            return false;
        }

        const data = await res.json();
        if (data.token) {
            auth.token = data.token;
            auth.isAuthenticated = true;
            sessionStorage.setItem(SESSION_KEY, data.token);
            return true;
        }
        return false;
    } catch (e) {
        console.error("!! [AUTH] Login connection failure:", e);
        return false;
    }
}

export async function bootAuth(): Promise<void> {
    if (auth.isAuthenticated) return;
    const stored = sessionStorage.getItem(SESSION_KEY);
    if (stored) {
        auth.token = stored;
        auth.isAuthenticated = true;
        return;
    }
    
    console.log(">> [AUTH] Attempting silent handshake...");
    const ok = await login('SOVEREIGN');
    if (ok) {
        console.log(">> [AUTH] Substrate Handshake Complete.");
    } else {
        console.warn(">> [AUTH] Silent handshake failed. Manual verify required.");
    }
}
