const API_URL = "/api";

export async function runTask(directive) {
    try {
        const res = await fetch(`${API_URL}/task`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ directive })
        });
        if (!res.ok) throw new Error("Zenith Task failed to initiate");
        const data = await res.json();
        return data.output; // Returns the full string session log
    } catch (e) {
        console.error(">> ZENITH_ERROR:", e);
        return `!! [SYSTEM_FAULT]: ${e.message}`;
    }
}

export async function readFile(path) {
    try {
        const res = await fetch(`${API_URL}/fs/read?path=${path}`);
        if (!res.ok) throw new Error("File not found");
        return await res.json();
    } catch (e) {
        console.error(e);
        return { content: "// Error loading file" };
    }
}

export async function saveFile(path, content) {
    const res = await fetch(`${API_URL}/fs/write`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ path, content })
    });
    return res.ok;
}

export async function deleteFile(path) {
    const res = await fetch(`${API_URL}/fs/delete?path=${path}`, {
        method: "DELETE"
    });
    return res.ok;
}

export async function spawnDrone(directive) {
    try {
        const res = await fetch(`${API_URL}/drone`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ directive })
        });
        return res.ok;
    } catch (e) {
        console.error(">> DRONE_FAULT:", e);
        return false;
    }
}
