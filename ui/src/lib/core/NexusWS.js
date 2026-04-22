// NexusWS.js — v3.1 compatibility shim
// The real WebSocket lives in nexus_socket.svelte.ts.
// This module exists only so legacy components that call nexusWS.* work.
import { storeAddLog } from './store.svelte.js';

export function pushTermLine(message, cls = 'text-gray-400') {
    storeAddLog(message, cls);
}

export const nexusWS = {
    logHandlers:         [],
    registerLogHandler() {},
    onMessage()          {},
    triggerLog(msg, cls) { storeAddLog(msg, cls); },
    send()               {},
};
