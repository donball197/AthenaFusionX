// terminal.svelte.ts — v2.1
// FIX: clearGhost now also resets isGhostTyping (was leaving spinner stuck)
// FIX: finishGhost() added — call this when NEURAL_STREAM ends (no more chunks)
// FIX: log cap raised to 500 with a hard splice to avoid unbounded growth
// FIX: addLog is now the single write path; direct mutation discouraged

const MAX_LOGS = 500;

export const terminal = $state({
    logs:          [] as { timestamp: string; message: string; cls: string }[],
    ghostBuffer:   '',
    isGhostTyping: false,
});

export function addLog(message: string, cls = 'text-gray-400') {
    const entry = { timestamp: new Date().toLocaleTimeString(), message, cls };
    if (terminal.logs.length >= MAX_LOGS) {
        terminal.logs = [...terminal.logs.slice(-MAX_LOGS + 1), entry];
    } else {
        terminal.logs = [...terminal.logs, entry];
    }
}

export function appendGhostBuffer(chunk: string) {
    const next = terminal.ghostBuffer + chunk;
    terminal.ghostBuffer = next.length > 5000 ? next.slice(-5000) : next;
    terminal.isGhostTyping = true;
}

/** Call when the stream is done (no more NEURAL_STREAM events expected). */
export function finishGhost() {
    terminal.isGhostTyping = false;
}

export function clearGhost() {
    terminal.ghostBuffer   = '';
    terminal.isGhostTyping = false;
}
