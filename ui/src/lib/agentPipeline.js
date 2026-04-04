/**
 * Project Nexus Singularity - Secure Agent Pipeline v1.4.2
 * Standardized JSON ingestion with Error-Gating.
 */

export function normalizeResponse(response) {
    if (typeof response !== 'string') return "";
    // Aggressive cleaning of Markdown envelopes
    return response.replace(/```json/g, "").replace(/```/g, "").trim();
}

export function extractJSON(str) {
    const cleaned = normalizeResponse(str);
    
    // Find the first valid JSON structure boundary
    const startIdx = cleaned.indexOf('{') !== -1 ? cleaned.indexOf('{') : cleaned.indexOf('[');
    const endIdx = cleaned.lastIndexOf('}') !== -1 ? cleaned.lastIndexOf('}') : cleaned.lastIndexOf(']');

    if (startIdx === -1 || endIdx === -1) return null;

    const rawJson = cleaned.substring(startIdx, endIdx + 1);

    try {
        return JSON.parse(rawJson);
    } catch (e) {
        console.error(">> PIPELINE_FAULT: Invalid JSON structure detected.", e);
        return null;
    }
}

export function validateActions(actions) {
    const list = Array.isArray(actions) ? actions : [actions];
    return list.filter(a => a && typeof a.action === 'string');
}

export function isSafePatch(path) {
    // Whitelist check could go here, but keeping expanded Blacklist for stability
    const forbidden = [".env", "target/", "node_modules/", "ledger.json", "/etc/", "../", "~", ".ssh"];
    const normalized = path.toLowerCase();
    return !forbidden.some(f => normalized.includes(f));
}
