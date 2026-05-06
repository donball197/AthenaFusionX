import os

# 1. Fix bootAuth in auth.svelte.ts
auth_file = "ui/src/lib/stores/auth.svelte.ts"
with open(auth_file, "r") as f:
    code = f.read()

old_logic = """    console.log(">> [AUTH] Attempting silent handshake...");
    const ok = await login('SOVEREIGN');
    if (ok) {
        console.log(">> [AUTH] Substrate Handshake Complete.");
    } else {
        console.warn(">> [AUTH] Silent handshake failed. Manual verify required.");
    }"""

new_logic = """    console.log(">> [AUTH] Bootstrapping Sovereign Token...");
    try {
        const res = await fetch('/api/auth/bootstrap');
        if (res.ok) {
            const data = await res.json();
            auth.token = data.token;
            auth.isAuthenticated = true;
            sessionStorage.setItem(SESSION_KEY, data.token);
            console.log(">> [SECURITY] Sovereign token bootstrapped. UI authenticated.");
        } else {
            console.warn(">> [AUTH] Bootstrap denied. Manual login required.");
        }
    } catch (e) {
        console.error("!! [AUTH] Bootstrap connection failure:", e);
    }"""

if "login('SOVEREIGN')" in code:
    code = code.replace(old_logic, new_logic)
    with open(auth_file, "w") as f:
        f.write(code)
    print(">> [SUTURE] auth.svelte.ts patched to use /api/auth/bootstrap")

# 2. Inject global fetch interceptor in +layout.svelte
layout_file = "ui/src/routes/+layout.svelte"
with open(layout_file, "r") as f:
    code = f.read()

interceptor = """import { auth } from "$lib/stores/auth.svelte";

    // ⚡ NEXUS SOVEREIGN AUTH INTERCEPTOR
    if (typeof window !== 'undefined') {
        const originalFetch = window.fetch;
        window.fetch = async (...args) => {
            let [resource, config] = args;
            if (typeof resource === 'string' && resource.startsWith('/api') && !resource.includes('/api/auth')) {
                config = config || {};
                config.headers = config.headers || {};
                if (auth.token) {
                    config.headers['Authorization'] = `Bearer ${auth.token}`;
                }
                args[1] = config;
            }
            return originalFetch(...args);
        };
    }
"""

if "NEXUS SOVEREIGN AUTH INTERCEPTOR" not in code:
    code = code.replace('import { restoreSession } from "$lib/stores/auth.svelte";', interceptor + '\n    import { restoreSession } from "$lib/stores/auth.svelte";')
    with open(layout_file, "w") as f:
        f.write(code)
    print(">> [SUTURE] Global fetch interceptor injected into +layout.svelte")

