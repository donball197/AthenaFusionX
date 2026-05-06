// runtime.ts — v3.8 [TERMINAL_STREAM_FIX]
//
// FIX: NEURAL_STREAM events now also call storeAddLog() so output appears
//      in Terminal.svelte's log list, not only in ghostBuffer/GhostBlade.
//      Previously the agent's streamed response was only visible in the CLI
//      because the browser terminal renders terminal.logs, not ghostBuffer.
//
// FIX: Streaming chunks are buffered into a flush line — we only add a log
//      entry when a newline or sentence boundary arrives, not on every token.
//      This prevents the log from exploding with hundreds of single-word rows.
//
// FIX: TASK_STATUS also writes a log entry directly (already was happening
//      via storeAddLog, kept as-is).

import { systemVitals, storeAddLog, setActiveVmos, terminalState } from './store.svelte';
import { appendGhostBuffer, finishGhost, clearGhost } from '../stores/terminal.svelte';
import { nexusSocket } from './socket';
import { bootAuth } from '../stores/auth.svelte';
import { agentRegistry } from './agent_module_registry.svelte';
import { uplink } from '../services/uplink.js';

let _ghostIdleTimer: ReturnType<typeof setTimeout> | null = null;

// Buffer for assembling streaming tokens into complete log lines
let _streamLineBuffer = '';

function flushStreamLine(force = false) {
    const line = _streamLineBuffer.trim();
    if (!line) return;
    const hasBreak = force || line.endsWith('.') || line.endsWith('\n') || line.length > 120;
    if (hasBreak) {
        storeAddLog(line, 'text-purple-300');
        _streamLineBuffer = '';
    }
}

function scheduleGhostIdle() {
    if (_ghostIdleTimer) clearTimeout(_ghostIdleTimer);
    _ghostIdleTimer = setTimeout(() => {
        // Flush any remaining buffered stream text when the stream ends
        flushStreamLine(true);
        finishGhost();
        _ghostIdleTimer = null;
    }, 2000);
}

class NexusRuntime {
    async init() {
        try {
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

                // Dynamic UI module proposal
                if (msg.type === 'DYNAMIC_UI_PROPOSAL') {
                    agentRegistry.propose(msg.module);
                    storeAddLog(`>> [GHOST] UI Extension projected: ${msg.module?.label}`, 'text-purple-400');
                }

                // ─── FIX: Stream goes to BOTH ghostBuffer AND terminal.logs ──
                if (msg.type === 'NEURAL_STREAM') {
                    const chunk = msg.payload ?? '';
                    appendGhostBuffer(chunk);
                    _streamLineBuffer += chunk;
                    flushStreamLine(false);
                    scheduleGhostIdle();
                }
                // ─────────────────────────────────────────────────────────────

                // Task completed
                if (msg.type === 'TASK_STATUS') {
                    flushStreamLine(true);          // flush any partial stream line
                    finishGhost();
                    if (_ghostIdleTimer) { clearTimeout(_ghostIdleTimer); _ghostIdleTimer = null; }
                    storeAddLog(`>> [STATUS] ${msg.payload ?? ''}`, 'text-emerald-400');
                }

                // Thought monologue from agent
                if (msg.type === 'NEURAL_THOUGHT') {
                    storeAddLog(`   [THOUGHT] ${msg.payload ?? ''}`, 'text-cyan-700');
                }

                // Generic log event
                if (msg.type === 'LOG_EVENT') {
                    storeAddLog(msg.payload ?? '', msg.cls ?? 'text-cyan-300');
                }

                // Internal socket lifecycle
                if (msg._internal === 'CONNECTION_STATE') {
                    systemVitals.online = msg.status === 'CONNECTED';
                    storeAddLog(
                        `>> [WS] ${msg.status}`,
                        msg.status === 'CONNECTED' ? 'text-emerald-400' : 'text-rose-400'
                    );
                }
            });
        } catch (e) {
            console.error('!! [RUNTIME] Substrate Connection Failed:', e);
        }
    }

    async emitIntent(intent_type: string, payload: any) {
        const res = await uplink.post('/api/intent', { intent_type, payload });
        return res.json();
    }
}

export const runtime    = new NexusRuntime();
export function bootRuntime() { runtime.init(); }
