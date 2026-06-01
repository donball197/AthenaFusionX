export const uplink = {
    async post(path, body) {
        return await fetch(path, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(body)
        });
    },
    async json(path) {
        const res = await fetch(path);
        return await res.json();
    }
};
