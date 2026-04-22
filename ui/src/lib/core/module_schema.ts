// module_schema.ts — TYPE DEFINITIONS ONLY
// Logic lives in module_schema.js — do not duplicate validateModule here.
// This file exists only for TypeScript consumers that need the types.
export type ModuleAction = { label: string; directive: string; style?: string; };
export type ModuleSpec   = {
    id:          string;
    label:       string;
    version:     string;
    type:        'panel' | 'tool';
    render:      { kind: string; source?: string; actions?: ModuleAction[]; };
    generatedBy?: string;
};
// Re-export validateModule from JS so TS callers can use it
export { validateModule } from './module_schema.js';
