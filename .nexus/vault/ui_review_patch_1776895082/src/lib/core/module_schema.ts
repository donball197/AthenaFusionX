export type ModuleAction = { label: string; directive: string; style?: string; };
export type ModuleSpec = { id: string; label: string; version: string; type: "panel"|"tool"; render: { kind: string; source?: string; actions?: ModuleAction[]; }; generatedBy?: string; };
export function validateModule(m: ModuleSpec) { return !!(m.id && m.label && m.render?.kind); }
