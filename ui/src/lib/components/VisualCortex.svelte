/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


<script>
    import { onDestroy } from 'svelte';

    let { active = false } = $props();

    let visionSrc = $state('');
    let loopHandle = null;

    async function fetchFrame() {
        try {
            const res = await fetch('/api/vision/frame');
            const data = await res.json();
            if (data.status === 'ok' && data.response) {
                visionSrc = 'data:image/jpeg;base64,' + data.response;
            }
        } catch { }
    }

    function stopLoop() {
        if (loopHandle) { clearTimeout(loopHandle); loopHandle = null; }
        visionSrc = '';
    }

    async function runLoop() {
        if (!active) return;
        await fetchFrame();
        if (active) loopHandle = setTimeout(runLoop, 220);
    }

    $effect(() => {
        if (active) {
            runLoop();
        } else {
            stopLoop();
        }
    });

    onDestroy(stopLoop);
</script>

{#if active && visionSrc}
<div class="absolute top-16 right-8 w-80 border border-red-600/60 bg-black/95 z-50 shadow-2xl overflow-hidden">
    <div class="bg-red-950/80 text-red-400 text-[9px] px-3 py-1.5 flex justify-between border-b border-red-900/50">
        <span>OPTIC_NERVE // LIVE_FEED</span>
        <span class="animate-pulse">REC</span>
    </div>
    <img src={visionSrc} class="w-full block" alt="Neural Vision" />
    <div class="px-3 py-1 text-[9px] text-gray-500 font-mono">TARGET: DISPLAY_0 • 5 FPS</div>
</div>
{/if}
