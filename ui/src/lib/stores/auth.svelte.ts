export const auth = $state({ token: null as string | null, isAuthenticated: false, isInitialized: false });
export async function bootAuth() {
    try {
        const res = await fetch('/api/auth/bootstrap');
        if (res.ok) {
            const data = await res.json();
            auth.token = data.token;
            auth.isAuthenticated = true;
        }
    } finally { auth.isInitialized = true; }
}
