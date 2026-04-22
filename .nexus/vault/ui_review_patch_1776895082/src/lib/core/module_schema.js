// module_schema.js — v1.1  (plain JS, TypeScript types in .ts companion)
// FIX: exported as .js so agent_module_registry.svelte.js can import
//      without TS transform issues.

/** @param {any} m */
export function validateModule(m) {
    if (!m || typeof m !== 'object')       return false;
    if (typeof m.id     !== 'string')      return false;
    if (typeof m.label  !== 'string')      return false;
    if (!m.render || typeof m.render !== 'object') return false;
    const VALID_KINDS = ['terminal', 'chart', 'table', 'form', 'custom'];
    if (!VALID_KINDS.includes(m.render.kind)) {
        console.warn('[SCHEMA] Unknown render kind:', m.render.kind,
            '— allowed:', VALID_KINDS);
        return false;
    }
    return true;
}
