export type ModuleAction = { label: string; directive: string; style?: string; };
export type ModuleSpec   = {
    id:          string;
    label:       string;
    version:     string;
    type:        'panel' | 'tool';
    render:      { kind: 'terminal' | 'chart' | 'table' | 'form' | 'custom'; source?: string; actions?: ModuleAction[]; };
    generatedBy?: string;
};

export function validateModule(m: any): m is ModuleSpec {
    if (!m || typeof m !== 'object') return false;
    if (typeof m.id !== 'string') return false;
    const VALID_KINDS = ['terminal', 'chart', 'table', 'form', 'custom'];
    return VALID_KINDS.includes(m.render?.kind);
}
