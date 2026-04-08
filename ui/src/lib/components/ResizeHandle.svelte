<script>
    import { onMount, onDestroy } from 'svelte';

    // Svelte 5 props
    let { horizontal = false, vertical = false, value = $bindable(0.5) } = $props();

    let dragging = false;

    function start(e) {
        dragging = true;
        e.preventDefault();
    }

    function move(e) {
        if (!dragging) return;
        let next = value;
        if (horizontal) next = e.clientX / window.innerWidth;
        if (vertical)   next = 1 - (e.clientY / window.innerHeight);
        value = Math.min(0.9, Math.max(0.1, next));
    }

    function stop() { dragging = false; }

    // FIX: Listeners registered in onMount and cleaned up in onDestroy.
    // The previous pattern registered them at module evaluation time and
    // never removed them, leaking one pair of handlers per component instance.
    onMount(() => {
        window.addEventListener('mousemove', move);
        window.addEventListener('mouseup',   stop);
    });

    onDestroy(() => {
        window.removeEventListener('mousemove', move);
        window.removeEventListener('mouseup',   stop);
    });
</script>

<button
    class="handle {horizontal ? 'h' : ''} {vertical ? 'v' : ''}"
    onmousedown={start}
    aria-label="Resize Splitter"
    type="button"
></button>

<style>
    .handle            { background: #222; border: none; padding: 0; transition: background 0.2s; }
    .handle:hover      { background: #444; }
    .handle.h          { width: 4px; cursor: col-resize; height: 100%; }
    .handle.v          { height: 4px; cursor: row-resize; width: 100%; }
</style>
