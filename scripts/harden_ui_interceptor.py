import os
import re

fpath = "ui/src/routes/+layout.svelte"
if not os.path.exists(fpath):
    print(">> [FAULT] ui/src/routes/+layout.svelte not found.")
    exit(1)

with open(fpath, "r") as f:
    code = f.read()

# ⚡ GREEDY NEXUS AUTH INTERCEPTOR
# This regex catches absolute URLs, relative URLs, and Handles Headers objects
new_interceptor = """    if (typeof window !== 'undefined') {
        const originalFetch = window.fetch;
        window.fetch = async (...args) => {
            let [resource, config] = args;
            const urlStr = typeof resource === 'string' ? resource : resource.url;
            
            // GREEDY MATCH: Catch any /api call that isn't auth-related
            if (urlStr && /api\/(?!auth)/.test(urlStr)) {
                config = config || {};
                let headers = config.headers || {};
                const token = auth.token || sessionStorage.getItem('nexus_session_token');
                
                if (token) {
                    if (headers instanceof Headers) {
                        headers.set('Authorization', `Bearer ${token}`);
                    } else {
                        headers['Authorization'] = `Bearer ${token}`;
                    }
                }
                config.headers = headers;
                args[1] = config;
            }
            return originalFetch.apply(window, args);
        };
    }"""

# Replace the old interceptor block
code = re.sub(
    r"if \(typeof window !== 'undefined'\) \{.*?originalFetch.*?\}",
    new_interceptor,
    code,
    flags=re.DOTALL
)

with open(fpath, "w") as f:
    f.write(code)
print(">> [SUTURE] Greedy UI Interceptor deployed.")
