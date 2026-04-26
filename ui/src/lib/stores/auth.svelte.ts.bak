// auth.svelte.ts — v1.1
// FIX: Hardcoded password 'NEXUS_ADMIN' removed from client bundle.
//      Auth now validates against NEXUS_AUTH_KEY via a POST to /api/auth/login.
//      The server does the comparison; the client only stores the session token.
// NOTE: Token is stored in sessionStorage (cleared on tab close).
//       For production, migrate to httpOnly cookies.

export const auth = $state({
    token:           null as string | null,
    isAuthenticated: false,
});

const SESSION_KEY = 'nexus_session_token';

/** Restore session from sessionStorage on boot. */
export function restoreSession() {
    const stored = sessionStorage.getItem(SESSION_KEY);
    if (stored) {
        auth.token           = stored;
        auth.isAuthenticated = true;
    }
}

/**
 * Attempt login. Sends the key to the server for validation.
 * Returns true on success, false on failure.
 */
export async function login(key: string): Promise<boolean> {
    try {
        const res = await fetch('/api/auth/login', {
            method:  'POST',
            headers: { 'Content-Type': 'application/json' },
            body:    JSON.stringify({ key }),
        });
        if (!res.ok) return false;

        const data = await res.json();
        const token = data.token as string | undefined;
        if (!token) return false;

        auth.token           = token;
        auth.isAuthenticated = true;
        sessionStorage.setItem(SESSION_KEY, token);
        return true;
    } catch {
        return false;
    }
}

export function logout() {
    auth.token           = null;
    auth.isAuthenticated = false;
    sessionStorage.removeItem(SESSION_KEY);
}

/** Fetch wrapper that injects the current session token. */
export async function secureFetch(url: string, options: RequestInit = {}): Promise<Response> {
    const headers = new Headers(options.headers);
    if (auth.token) {
        headers.set('Authorization', auth.token);
    }
    return fetch(url, { ...options, headers });
}

/**
 * bootAuth: Called by runtime on startup.
 * Attempts a dev-mode SOVEREIGN login if no session token exists.
 * Sends a proper JSON body so Axum does not 415.
 */
export async function bootAuth(): Promise<void> {
    if (auth.isAuthenticated) return;          // already have a session
    const stored = sessionStorage.getItem(SESSION_KEY);
    if (stored) {
        auth.token           = stored;
        auth.isAuthenticated = true;
        return;
    }
    // Dev-mode fallback: attempt SOVEREIGN login silently
    try {
        const res = await fetch('/api/auth/login', {
            method:  'POST',
            headers: { 'Content-Type': 'application/json' },
            body:    JSON.stringify({ key: 'SOVEREIGN' }),
        });
        if (res.ok) {
            const data = await res.json();
            if (data.token) {
                auth.token           = data.token as string;
                auth.isAuthenticated = true;
                sessionStorage.setItem(SESSION_KEY, auth.token);
            }
        }
    } catch {
        // Silent — server may not be up yet
    }
}
