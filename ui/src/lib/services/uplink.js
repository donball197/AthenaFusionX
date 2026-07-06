const AUTH_HEADER = 'Basic YWRtaW46cGFzc3dvcmQ=';

export const uplink = {
    async post(path, body) {
        return await fetch(path, {
            method: 'POST',
            headers: { 
                'Content-Type': 'application/json',
                'Authorization': AUTH_HEADER
            },
            body: JSON.stringify(body)
        });
    },
    async json(path) {
        const res = await fetch(path, {
            headers: {
                'Authorization': AUTH_HEADER
            }
        });
        return await res.json();
    }
};
