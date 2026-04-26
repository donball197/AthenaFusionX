// api.js — v2.0 shim
// Backward-compat wrapper. New code should import from uplink.js directly.
import { uplink } from '$lib/services/uplink.js';

export const readFile    = (path)            => uplink.json(`/api/fs/read?path=${encodeURIComponent(path)}`).catch(() => ({ content: `// Failed to read ${path}\n` }));
export const writeFile   = (path, content)   => uplink.post('/api/fs/write', { path, content }).then(r => r.ok).catch(() => false);
export const listFiles   = (path = '.')      => uplink.json(`/api/fs/list?path=${encodeURIComponent(path)}`).catch(() => []);
export const runTask     = (directive)       => uplink.postJSON('/api/task', { directive }).catch(() => ({ status: 'ERROR', output: 'Failed to queue task.' }));
export const getMemories = ()               => uplink.json('/api/memories').catch(() => []);
export const getSnapshots = ()              => uplink.json('/api/lazarus/snapshots').catch(() => []);
