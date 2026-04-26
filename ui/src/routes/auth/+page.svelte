<script lang="ts">
    // auth/+page.svelte — v1.1
    // FIX: Password no longer hardcoded in client bundle.
    //      Validation now happens server-side via /api/auth/login.
    //      Client receives a session token on success.
    import { fly } from 'svelte/transition';
    import { login } from '$lib/stores/auth.svelte';
    import SoundFx from '$lib/SoundFx';

    let key     = $state('');
    let error   = $state('');
    let loading = $state(false);

    async function authenticate() {
        if (!key.trim() || loading) return;
        loading = true;
        error   = '';
        const ok = await login(key);
        if (ok) {
            SoundFx.success();
            window.location.href = '/';
        } else {
            SoundFx.error();
            error = 'INVALID_KEY — Authorization Denied';
            key   = '';
        }
        loading = false;
    }

    function onKey(e: KeyboardEvent) {
        if (e.key === 'Enter') authenticate();
    }
</script>

<div class="h-full flex items-center justify-center bg-[#020205]">
    <div class="w-80 glass-panel p-8 rounded-lg shadow-2xl" in:fly={{ y: 20 }}>
        <h1 class="text-cyan-400 text-xs font-black mb-8 tracking-[5px] uppercase text-center">
            Identity_Verification
        </h1>
        <div class="space-y-4">
            <input
                type="password"
                bind:value={key}
                onkeydown={onKey}
                class="w-full bg-black/50 border {error ? 'border-red-500' : 'border-cyan-900'}
                       p-3 text-xs text-cyan-400 outline-none focus:border-cyan-400
                       text-center font-mono transition-colors"
                placeholder="DNA_KEY_SIGNATURE"
                autocomplete="current-password"
                disabled={loading}
            />
            {#if error}
                <div class="text-rose-400 text-[9px] text-center tracking-widest">{error}</div>
            {/if}
            <button
                onclick={authenticate}
                disabled={loading}
                class="w-full py-3 bg-cyan-900/20 border border-cyan-500/50 text-[10px]
                       text-cyan-400 font-bold uppercase hover:bg-cyan-400 hover:text-black
                       transition-all disabled:opacity-40 disabled:cursor-not-allowed"
            >
                {loading ? 'VERIFYING...' : 'Authorize_Uplink'}
            </button>
        </div>
    </div>
</div>
