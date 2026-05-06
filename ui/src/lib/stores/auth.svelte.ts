export const auth = $state({
    token:           null as string | null,
    isAuthenticated: false,
    isInitialized:   false,
});

const SESSION_KEY = 'nexus_session_token';

export function restoreSession() {
    if (typeof window === 'undefined') return;
    const stored = sessionStorage.getItem(SESSION_KEY);
    if (stored) {
        auth.token = stored;
        auth.isAuthenticated = true;
        console.log(">> [AUTH] Session restored from storage.");
    }
}

export async function login(key: string): Promise<boolean> {
    console.log(">> [AUTH] Attempting manual login...");
    try {
        const res = await fetch('/api/auth/login', {
            method:  'POST',
            headers: { 'Content-Type': 'application/json' },
            body:    JSON.stringify({ key }),
        });
        if (!res.ok) return false;
        const data = await res.json();
        if (data.token) {
            auth.token = data.token;
            auth.isAuthenticated = true;
            sessionStorage.setItem(SESSION_KEY, data.token);
            return true;
        }
        return false;
    } catch (e) {
        console.error("!! [AUTH] Login failed:", e);
        return false;
    }
}

export async function bootAuth(): Promise<void> {
    if (typeof window === 'undefined') return;
    
    // 1. Try session storage first
    restoreSession();
    if (auth.isAuthenticated) {
        auth.isInitialized = true;
        return;
    }
    
    // 2. Fallback to local bootstrap
    console.log(">> [AUTH] No session. Bootstrapping Sovereign Token...");
    try {
        const res = await fetch('/api/auth/bootstrap');
        if (res.ok) {
            const data = await res.json();
            auth.token = data.token;
            auth.isAuthenticated = true;
            sessionStorage.setItem(SESSION_KEY, data.token);
            console.log(">> [SECURITY] Sovereign token bootstrapped.");
        } else {
            console.warn(">> [AUTH] Bootstrap denied. Manual login required.");
        }
    } catch (e) {
        console.error("!! [AUTH] Bootstrap connection failure:", e);
    } finally {
        auth.isInitialized = true;
    }
}
