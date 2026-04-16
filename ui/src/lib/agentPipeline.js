/**
 * Project Nexus Singularity - Robust Agent Pipeline v1.5.0
 */

export function normalizeResponse(response) {
    if (typeof response !== 'string') return "";
    return response.trim();
}

export function extractJSON(str) {
    const cleaned = normalizeResponse(str);
    const actions = [];
    
    // Regex to find all content inside <ACTION> tags
    const actionRegex = /<ACTION>([\s\S]*?)<\/ACTION>/g;
    let match;

    while ((match = actionRegex.exec(cleaned)) !== null) {
        try {
            const rawJson = match[1].trim();
            const parsed = JSON.parse(rawJson);
            
            // If it's an array, flatten it; if object, push it
            if (Array.isArray(parsed)) {
                actions.push(...parsed);
            } else {
                actions.push(parsed);
            }
        } catch (e) {
            console.error(">> PIPELINE_FAULT: Malformed JSON in block", e);
        }
    }

    return actions.length > 0 ? actions : null;
}

export function isSafePath(path) {
    const forbidden = [".env", "target/", "node_modules/", "/etc/", "../", ".ssh"];
    const normalized = path.toLowerCase();
    return !forbidden.some(f => normalized.includes(f));
}
