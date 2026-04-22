import { validateModule } from "./module_schema";
export const agentRegistry = $state({
    modules: [], log: [],
    propose: function(module) {
        if (!validateModule(module)) return false;
        const idx = this.modules.findIndex(m => m.id === module.id);
        if (idx >= 0) this.modules[idx] = module;
        else this.modules.push(module);
        return true;
    }
});
