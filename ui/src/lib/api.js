// Standardized API client with Sovereign Auth Suture

async function request(url, options = {}) {
    // ⚡ SUTURE: Global Authorization Header
    options.headers = {
        ...options.headers,
        'Authorization': 'SOVEREIGN'
    };

    try {
        const res = await fetch(url, options);
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return await res.json();
    } catch (e) {
        console.error(`[API_FAULT] ${url}:`, e);
        return null;
    }
}

export async function readFile(path) {
    const data = await request(`/api/fs/read?path=${encodeURIComponent(path)}`);
    return data || { content: `// Failed to read ${path}\n` };
}

export async function writeFile(path, content) {
    return await fetch('/api/fs/write', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'Authorization': 'SOVEREIGN' },
        body: JSON.stringify({ path, content })
    }).then(res => res.ok);
}

export async function listFiles(path = ".") {
    return await request(`/api/fs/list?path=${encodeURIComponent(path)}`) || [];
}

export async function runTask(directive) {
    const data = await request('/api/task', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ directive })
    });
    return data || { status: "ERROR", output: "Failed to queue task." };
}

export async function getMemories() {
    return await request('/api/memories') || [];
}

export async function getSnapshots() {
    return await request('/api/lazarus/snapshots') || [];
}
