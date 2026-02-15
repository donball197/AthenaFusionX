const API_URL = "http://localhost:3000/api";

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
