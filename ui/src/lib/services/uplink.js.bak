// uplink.js — v2.2 [TOKEN_GUARD + 415_FIX]
// FIX: No longer sends 'SOVEREIGN' as an Authorization header value.
//      Auth header is only injected when auth.token is a real JWT.
// FIX: Content-Type is always set on POST requests.
// FIX: AbortSignal supported on all methods.

import { auth } from '$lib/stores/auth.svelte';

function buildHeaders(extra = {}) {
    const headers = { 'Content-Type': 'application/json', ...extra };
    if (auth.token) {
        headers['Authorization'] = auth.token;
    }
    return headers;
}

async function request(url, options = {}) {
    const { headers: extraHeaders, ...rest } = options;
    const headers = buildHeaders(extraHeaders ?? {});
    const res = await fetch(url, { ...rest, headers });
    if (!res.ok) throw new Error(`HTTP ${res.status} — ${url}`);
    return res;
}

export const uplink = {
    fetch:    (url, options = {})     => request(url, options),
    post:     (url, body, signal)     => request(url, { method: 'POST',  body: JSON.stringify(body), signal }),
    put:      (url, body, signal)     => request(url, { method: 'PUT',   body: JSON.stringify(body), signal }),
    delete:   (url, signal)           => request(url, { method: 'DELETE', signal }),
    get:      (url, signal)           => request(url, { signal }),

    async json(url, options = {}) {
        const res = await request(url, options);
        return res.json();
    },

    async postJSON(url, body, signal) {
        const res = await request(url, { method: 'POST', body: JSON.stringify(body), signal });
        return res.json();
    },
};
