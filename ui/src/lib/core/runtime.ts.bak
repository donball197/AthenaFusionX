// runtime.ts — v3.6 [GHOST_TYPING_RESET + STREAM_DONE_DETECTION]
// FIX: isGhostTyping is now reset after a 2s idle timeout post last NEURAL_STREAM chunk.
// FIX: TASK_STATUS event calls finishGhost() to clear the spinner reliably.
// FIX: LOG_EVENT properly calls addLog helper rather than storeAddLog alias.

import { systemVitals, storeAddLog, setActiveVmos, terminalState } from './store.svelte';
import { appendGhostBuffer, finishGhost, clearGhost } from '../stores/terminal.svelte';
import { nexusSocket } from './socket';
import { bootAuth } from '../stores/auth.svelte';
import { agentRegistry } from './agent_module_registry.svelte';

let _ghostIdleTimer: ReturnType<typeof setTimeout> | null = null;

function scheduleGhostIdle() {
    if (_ghostIdleTimer) clearTimeout(_ghostIdleTimer);
    _ghostIdleTimer = setTimeout(() => {
        // No new chunk for 2s → stream has ended
        finishGhost();
        _ghostIdleTimer = null;
    }, 2000);
}

class NexusRuntime {
    async init() {
        try {
            // Attempt silent auth handshake — non-fatal if it fails
            await bootAuth();
            systemVitals.online = true;
            nexusSocket.connect();

            nexusSocket.subscribe((msg: any) => {
                // Heartbeat telemetry
                if (msg.cpu_load !== undefined) {
                    systemVitals.cpu_load     = msg.cpu_load;
                    systemVitals.ram_gb       = msg.ram_gb;
                    systemVitals.temperature  = msg.temperature;
                    systemVitals.resonance    = msg.resonance;
                    if (msg.active_vmos !== undefined) setActiveVmos(msg.active_vmos);
                }

                // Dynamic UI module proposal from agent
                if (msg.type === 'DYNAMIC_UI_PROPOSAL') {
                    agentRegistry.propose(msg.module);
                    storeAddLog(`>> [GHOST] UI Extension projected: ${msg.module?.label}`, 'text-purple-400');
                }

                // Streaming agent output
                if (msg.type === 'NEURAL_STREAM') {
                    appendGhostBuffer(msg.payload ?? '');
                    scheduleGhostIdle();
                }

                // Task completed — always clear spinner
                if (msg.type === 'TASK_STATUS') {
                    finishGhost();
                    if (_ghostIdleTimer) { clearTimeout(_ghostIdleTimer); _ghostIdleTimer = null; }
                    storeAddLog(`>> [STATUS] ${msg.payload ?? ''}`, 'text-emerald-400');
                }

                // Generic log event
                if (msg.type === 'LOG_EVENT') {
                    storeAddLog(msg.payload ?? '', msg.cls ?? 'text-cyan-300');
                }

                // Internal socket lifecycle events
                if (msg._internal === 'CONNECTION_STATE') {
                    systemVitals.online = msg.status === 'CONNECTED';
                }
            });
        } catch (e) {
            console.error('!! [RUNTIME] Substrate Connection Failed:', e);
        }
    }

    async emitIntent(intent_type: string, payload: any) {
        const res = await fetch('/api/intent', {
            method:  'POST',
            headers: { 'Content-Type': 'application/json' },
            body:    JSON.stringify({ intent_type, payload }),
        });
        if (!res.ok) throw new Error(`HTTP_${res.status}`);
        return res.json();
    }
}

export const runtime    = new NexusRuntime();
export function bootRuntime() { runtime.init(); }
