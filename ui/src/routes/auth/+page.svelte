<script>
    import { fly } from 'svelte/transition';
    import SoundFx from '$lib/SoundFx.js';
    let key = $state('');
    let error = $state(false);

    const authenticate = () => {
        if (key === 'NEXUS_ADMIN') {
            SoundFx.success();
            window.location.href = '/';
        } else {
            SoundFx.error();
            error = true;
            setTimeout(() => error = false, 1000);
        }
    };
</script>

<div class="h-full flex items-center justify-center bg-[#020205]">
    <div class="w-80 glass-panel p-8 rounded-lg shadow-2xl" in:fly={{ y: 20 }}>
        <h1 class="text-cyan-400 text-xs font-black mb-8 tracking-[5px] uppercase text-center">Identity_Verification</h1>
        <div class="space-y-6">
            <div class="relative">
                <input 
                    type="password" 
                    bind:value={key}
                    class="w-full bg-black/50 border {error ? 'border-red-500' : 'border-cyan-900'} p-3 text-xs text-cyan-400 outline-none focus:border-cyan-400 text-center font-mono" 
                    placeholder="DNA_KEY_SIGNATURE" 
                />
            </div>
            <button 
                onclick={authenticate}
                class="w-full py-3 bg-cyan-900/20 border border-cyan-500/50 text-[10px] text-cyan-400 font-bold uppercase hover:bg-cyan-400 hover:text-black transition-all"
            >
                Authorize_Uplink
            </button>
        </div>
    </div>
</div>
