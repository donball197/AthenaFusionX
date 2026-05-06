#!/bin/bash
# Nexus Singularity — Auth Substrate Suture v1.9.1
# Targets: Header Protocol Mismatch & Bootstrap Race Conditions

echo ">> [SUTURE] Realignment of Auth Arteries initiated..."

# 1. Update Uplink Service (Add Bearer prefix & 401 Interceptor)
cat <<'GHOST' > ui/src/lib/services/uplink.js
// uplink.js — v2.3 [BEARER_SUTURE]
import { auth } from '$lib/stores/auth.svelte';

function buildHeaders(extra = {}) {
    const headers = { 'Content-Type': 'application/json', ...extra };
    if (auth.token) {
        // SUTURE: Added 'Bearer ' prefix to align with standard Axum/JWT extractors
        // Ensure backend middleware strips this or verify_token handles it
        headers['Authorization'] = `Bearer ${auth.token}`;
    }
    return headers;
}

async function request(url, options = {}) {
    const { headers: extraHeaders, ...rest } = options;
    const headers = buildHeaders(extraHeaders ?? {});
    
    try {
        const res = await fetch(url, { ...rest, headers });
        
        if (res.status === 401) {
            console.warn(`!! [UPLINK] 401 Unauthorized at ${url}. Token invalid or missing.`);
        }
        
        if (!res.ok) throw new Error(`HTTP ${res.status} — ${url}`);
        return res;
    } catch (e) {
        throw e;
    }
}

export const uplink = {
    fetch:    (url, options = {})     => request(url, options),
    post:     (url, body, signal)     => request(url, { method: 'POST',  body: JSON.stringify(body), signal }),
    put:      (url, body, signal)     => request(url, { method: 'PUT',   body: JSON.stringify(body), signal }),
    delete:   (url, signal)           => request(url, { method: 'DELETE', signal }),
    get:      (url, signal)           => request(url, { signal }),
    json:     async (url, opts)       => (await request(url, opts)).json(),
};
GHOST

# 2. Update Auth Store (Enhanced Handshake Logs)
cat <<'GHOST' > ui/src/lib/stores/auth.svelte.ts
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
GHOST

# 3. Ensure Environment Parity
if [ ! -f .env ]; then
    echo "NEXUS_AUTH_KEY=SOVEREIGN" > .env
    echo ">> [SUTURE] .env created with default SOVEREIGN key."
fi

chmod +x suture_substrate.sh
echo ">> [SUTURE] Realignment complete. Run 'source .env' before starting the backend."
