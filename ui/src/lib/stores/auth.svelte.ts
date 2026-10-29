export const authState = $state({
    isAuthenticated: false,
    token: null as string | null,
});

export async function bootAuth() {
    console.log(">> [AUTH] Booting Sovereign identity...");
    
    const cachedToken = localStorage.getItem('nexus_token');
    if (cachedToken) {
        authState.token = cachedToken;
        authState.isAuthenticated = true;
        return;
    }

    const masterKey = prompt("Enter NEXUS_AUTH_KEY to establish Sovereign Uplink:");
    if (!masterKey) return;

    try {
        const res = await fetch('/api/auth/login', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({password: masterKey })
        });
        
        if (res.ok) {
            const data = await res.json();
            authState.token = data.token;
            authState.isAuthenticated = true;
            localStorage.setItem('nexus_token', data.token);
            console.log(">> [AUTH] Uplink Secured.");
        } else {
            alert("Sovereign Guard Rejected the Key.");
        }
    } catch (e) {
        console.error("Auth Exception:", e);
    }
}
