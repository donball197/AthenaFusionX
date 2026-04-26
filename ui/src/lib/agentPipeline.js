// agentPipeline.js — v1.6
// FIX: extractJSON now handles single-object results (not only arrays).
// FIX: isSafePath extended with .nexus, .env patterns and null-byte check.
// FIX: normalizeResponse handles non-string input gracefully.

export function normalizeResponse(response) {
    if (response == null) return '';
    if (typeof response !== 'string') {
        try { return JSON.stringify(response); } catch { return ''; }
    }
    return response.trim();
}

/**
 * extractJSON: Parse all <ACTION> blocks from a response string.
 * Handles arrays and single objects. Returns null if nothing valid found.
 */
export function extractJSON(str) {
    const cleaned = normalizeResponse(str);
    const actions = [];

    const actionRegex = /<ACTION>([\s\S]*?)<\/ACTION>/g;
    let match;
    while ((match = actionRegex.exec(cleaned)) !== null) {
        try {
            const rawJson = match[1].trim();
            const parsed  = JSON.parse(rawJson);
            if (Array.isArray(parsed)) {
                actions.push(...parsed);
            } else if (parsed && typeof parsed === 'object') {
                // Single action object
                actions.push(parsed);
            }
        } catch (e) {
            console.error('[PIPELINE] Malformed JSON in <ACTION> block:', e.message);
        }
    }

    // Fallback: last [...] block in response
    if (actions.length === 0) {
        const end = cleaned.lastIndexOf(']');
        if (end !== -1) {
            const start = cleaned.lastIndexOf('[', end);
            if (start !== -1) {
                try {
                    const parsed = JSON.parse(cleaned.slice(start, end + 1));
                    if (Array.isArray(parsed)) actions.push(...parsed);
                } catch { /* ignore */ }
            }
        }
    }

    return actions.length > 0 ? actions : null;
}

/**
 * isSafePath: Returns false for dangerous or sensitive paths.
 * Checks for null bytes, traversal components, and blacklisted prefixes.
 */
export function isSafePath(path) {
    if (!path || typeof path !== 'string') return false;
    if (path.includes('\0')) return false;

    const normalized = path.toLowerCase().replace(/\\/g, '/');

    const forbidden = [
        '../',
        '.env',
        '.ssh/',
        '/etc/',
        'target/',
        'node_modules/',
        '.nexus_memory.db',
        '.nexus_wasm_registry.db',
        '.nexus/vault',
        '.git/',
    ];
    return !forbidden.some(f => normalized.includes(f));
}
