import { validateModule, type ModuleSpec } from './module_schema';

export const agentRegistry = $state({
    modules: [] as ModuleSpec[],
    log: [] as { ts: number, action: string, id: string }[],

    propose(module: any) {
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

    remove(id: string) {
        this.modules = this.modules.filter(m => m.id !== id);
    },
});
