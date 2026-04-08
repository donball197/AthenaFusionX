// ui/src/lib/api.js
// Standardized API client for Nexus backend (Svelte 5 synchronized)

async function request(url, options = {}) {
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
    try {
        const res = await fetch('/api/fs/write', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ path, content })
        });
        return res.ok;
    } catch (e) {
        console.error("writeFile failed:", e);
        return false;
    }
}

export async function deleteFile(path) {
    try {
        const res = await fetch(`/api/fs/delete?path=${encodeURIComponent(path)}`, { 
            method: 'DELETE' 
        });
        return res.ok;
    } catch (e) {
        console.error("deleteFile failed:", e);
        return false;
    }
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

// === MEMORY & MISSIONS (Newly Synchronized) ===

export async function getMemories() {
    return await request('/api/memories') || [];
}

export async function searchMemories(q) {
    return await request(`/api/memories/search?q=${encodeURIComponent(q)}`) || [];
}

export async function purgeMemories() {
    try {
        const res = await fetch('/api/memories/purge', { method: 'DELETE' });
        return res.ok;
    } catch (e) { return false; }
}

export async function getMissions() {
    return await request('/api/missions') || [];
}

export async function getSnapshots() {
    return await request('/api/lazarus/snapshots') || [];
}
