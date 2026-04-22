export const uplink = {
    async fetch(url, options = {}) {
        const headers = {
            ...options.headers,
            'Authorization': 'SOVEREIGN',
            'Content-Type': 'application/json'
        };
        return fetch(url, { ...options, headers });
    },
    async post(url, body) {
        return this.fetch(url, {
            method: 'POST',
            body: JSON.stringify(body)
        });
    }
};
