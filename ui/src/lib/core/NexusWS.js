// NexusWS.js — v3.0 COMPATIBILITY SHIM (no WebSocket here)
// The real WebSocket is in nexus_socket.svelte.ts.
// This module exists only so legacy imports of pushTermLine still work.
import { storeAddLog } from './store.svelte.js';

// pushTermLine: Used by TaskService.js and some components directly.
export function pushTermLine(message, cls = 'text-gray-400') {
    storeAddLog(message, cls);
}

// Stub nexusWS object so any legacy import of `nexusWS` doesn't crash.
export const nexusWS = {
    registerLogHandler() {},
    onMessage() {},
    send() {},
    logHandlers: [],
};

export function connectWS() {
    // No-op: connection is managed by nexus_socket.svelte.ts
}
