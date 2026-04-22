/* NexusWS.js — compatibility layer for store.svelte.js */
import { terminalState } from './store.svelte.js';

let logHandlers = [];

export const nexusWS = {
    registerLogHandler(handler) {
        logHandlers.push(handler);
    },

    triggerLog(message, cls = 'text-gray-400') {
        terminalState.logs = [
            ...terminalState.logs.slice(-179),
            { timestamp: new Date().toLocaleTimeString(), message, cls }
        ];
        logHandlers.forEach(h => h(message, cls));
    },

    onMessage(handler) {
        console.log('[NexusWS] onMessage registered (wired via layout)');
    }
};
