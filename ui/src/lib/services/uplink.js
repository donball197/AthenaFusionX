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
