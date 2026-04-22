// agent_module_registry.svelte.js — v1.1
// FIX: Added remove() helper for module cleanup.
import { validateModule } from './module_schema.js';

export const agentRegistry = $state({
    modules: [],
    log:     [],

    propose(module) {
        if (!validateModule(module)) {
            console.warn('[REGISTRY] Invalid module proposal rejected:', module?.id);
            return false;
        }
        const idx = this.modules.findIndex(m => m.id === module.id);
        if (idx >= 0) {
            this.modules[idx] = module;
        } else {
            this.modules.push(module);
        }
        this.log.push({ ts: Date.now(), action: 'propose', id: module.id });
        return true;
    },

    remove(id) {
        this.modules = this.modules.filter(m => m.id !== id);
    },
});
