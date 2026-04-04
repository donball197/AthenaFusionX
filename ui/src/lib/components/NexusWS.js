import {
    terminalLogs, isGhostTyping, ghostBuffer,
    systemVitals, fileTreeTrigger, wsReady, pendingPatch,
    activeTab, openTabs
} from '$lib/store';
import { readFile } from '$lib/api';

let socket = null;
let messageHandlers = [];

function esc(str) {
    if (!str) return "";
    return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function ansiToHtml(raw) {
    if (!raw) return "";
    const safe = esc(raw);
    return safe
        .replace(/\n/g,                  '<br/>')
        .replace(/\x1b\[31m/g,           '<span class="text-red-500">')
        .replace(/\x1b\[32m/g,           '<span class="text-green-400">')
        .replace(/\x1b\[33m/g,           '<span class="text-yellow-400">')
        .replace(/\x1b\[35m/g,           '<span class="text-purple-400">')
        .replace(/\x1b\[36m/g,           '<span class="text-cyan-400 font-bold">')
        .replace(/\x1b\[90m/g,           '<span class="text-gray-500">')
        .replace(/\x1b\[0m/g,            '</span>');
}

export function pushTermLine(rawText, cssClass = '') {
    const html = ansiToHtml(rawText);
    terminalLogs.update(logs => {
        const next = [...logs, { html, cls: cssClass }];
        return next.length > 500 ? next.slice(next.length - 500) : next;
    });
}

function handleMessage(event) {
    let data;
    try { data = JSON.parse(event.data); }
    catch { pushTermLine(event.data); return; }

    messageHandlers.forEach(h => h(data));

    // === PERFECT SYNC SUTURE ===
    if (data.type === 'file_opened' || data.type === 'file_written') {
        const path = data.payload;
        readFile(path).then(res => {
            openTabs.update(tabs => {
                if (!tabs.find(t => t.path === path)) {
                    return [...tabs, { path, content: res.content, isDirty: false }];
                }
                return tabs;
            });
            activeTab.set(path);
        });
        
        if (data.type === 'file_written') {
            fileTreeTrigger.update(n => n + 1);
        }
    }
    // ============================

    switch (data.type) {
        case 'telemetry':
            systemVitals.set(data.payload);
            break;
        case 'terminal':
            pushTermLine(data.payload);
            break;
        case 'ghost_start':
            isGhostTyping.set(true);
            ghostBuffer.set('>> [NEURAL_UPLINK_ESTABLISHED]\n');
            break;
        case 'ghost_token':
            ghostBuffer.update(b => b + data.payload);
            break;
        case 'ghost_end':
            isGhostTyping.set(false);
            fileTreeTrigger.update(n => n + 1);
            break;
        case 'patch_ready':
            pendingPatch.set(data.payload);
            break;
    }
}

export const WS = {
    connect() {
        if (socket && socket.readyState <= WebSocket.OPEN) return;
        const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        socket = new WebSocket(`${proto}//${window.location.host}/ws/nexus`);
        
        socket.onopen = () => {
            wsReady.set(true);
            pushTermLine('\x1b[32m[NEXUS] WebSocket link established.\x1b[0m');
        };

        socket.onmessage = handleMessage;

        socket.onclose = () => {
            wsReady.set(false);
            pushTermLine('\x1b[33m[NEXUS] Link lost — reconnecting...\x1b[0m');
            setTimeout(() => this.connect(), 3000);
        };
    },
    send(data) {
        if (socket && socket.readyState === WebSocket.OPEN) {
            socket.send(typeof data === 'string' ? data : JSON.stringify(data));
        }
    }
};

export const connectWS = WS.connect;
export const sendRaw = WS.send;
export function onMessage(fn) {
    messageHandlers.push(fn);
    return () => { messageHandlers = messageHandlers.filter(h => h !== fn); };
}
