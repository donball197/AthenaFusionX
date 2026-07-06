export const authState = $state({
    isAuthenticated: false,
    token: null as string | null,
});

export function bootAuth() {
    console.log(">> [AUTH] Booting Sovereign identity...");
    // Future expansion: check localStorage for the JWT token here
    authState.isAuthenticated = true;
}
