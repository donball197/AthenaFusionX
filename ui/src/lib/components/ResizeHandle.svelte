<script>
    export let horizontal = false;
    export let vertical = false;
    export let value;
    let dragging = false;

    function start(e) { dragging = true; e.preventDefault(); }
    
    function move(e) {
        if (!dragging) return;
        if (horizontal) value = e.clientX / window.innerWidth;
        if (vertical) value = 1 - (e.clientY / window.innerHeight);
        if (value < 0.1) value = 0.1; 
        if (value > 0.9) value = 0.9;
    }

    function stop() { dragging = false; }

    if (typeof window !== 'undefined') {
        window.addEventListener("mousemove", move);
        window.addEventListener("mouseup", stop);
    }
</script>

<button 
    class="handle {horizontal ? 'h' : ''} {vertical ? 'v' : ''}" 
    on:mousedown={start}
    aria-label="Resize Splitter"
    type="button"
></button>

<style>
    .handle { 
        background: #222; 
        border: none; 
        padding: 0;
        transition: background 0.2s; 
    }
    .handle:hover { background: #444; }
    .handle.h { width: 4px; cursor: col-resize; height: 100%; }
    .handle.v { height: 4px; cursor: row-resize; width: 100%; }
</style>
