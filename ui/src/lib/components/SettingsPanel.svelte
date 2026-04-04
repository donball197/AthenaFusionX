<script>
    let { isOpen = false, onClose } = $props();
</script>

{#if isOpen}
    <div class="fixed inset-0 bg-black/90 z-[300] flex items-center justify-center" 
         role="presentation" 
         onclick={onClose} 
         onkeydown={(e) => e.key === 'Escape' && onClose()}>
        
        <div class="w-[500px] bg-[#0a0a0a] border border-[#30363d] rounded shadow-2xl overflow-hidden" 
             role="dialog" 
             aria-modal="true" 
             tabindex="-1"
             onclick={(e) => e.stopPropagation()}
             onkeydown={(e) => e.stopPropagation()}>
            
            <div class="border-b border-[#30363d] p-4 flex justify-between items-center">
                <span class="text-xs text-gray-500 font-bold tracking-widest uppercase">KERNEL SETTINGS</span>
                <button onclick={onClose} class="text-gray-500 hover:text-white transition-colors">×</button>
            </div>
            
            <div class="p-6 space-y-4 font-mono text-sm text-gray-300">
                <div class="flex justify-between items-center pb-2 border-b border-[#1f1f1f]">
                    <span>Neural Resonance Threshold</span>
                    <span class="text-neon-green">0.85</span>
                </div>
                <div class="flex justify-between items-center pb-2 border-b border-[#1f1f1f]">
                    <span>Memory Allocation (Mimalloc)</span>
                    <span class="text-blue-400">LEAN</span>
                </div>
                <div class="flex justify-between items-center pb-2 border-b border-[#1f1f1f]">
                    <span>Gemma 3 Uplink</span>
                    <span class="text-neon-green">STABLE</span>
                </div>
            </div>
        </div>
    </div>
{/if}
