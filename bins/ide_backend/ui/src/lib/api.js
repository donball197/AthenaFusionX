// api.js — v3.0 [UPLINK_UNIFIED]
import { uplink } from '$lib/services/uplink.js';

/** 
 * Unified Backend Bridge 
 * Always use uplink to ensure Bearer Token propagation.
 */
export const readFile    = (path)          => uplink.json(`/api/fs/read?path=${encodeURIComponent(path)}`);
export const writeFile   = (path, content) => uplink.post('/api/fs/write', { path, content });
export const listFiles   = (path = '.')    => uplink.json(`/api/fs/list?path=${encodeURIComponent(path)}`);
export const runTask     = (directive)     => uplink.post('/api/task', { directive });
export const getMemories = ()             => uplink.json('/api/memories');
export const getSnapshots = ()            => uplink.json('/api/lazarus/snapshots');
