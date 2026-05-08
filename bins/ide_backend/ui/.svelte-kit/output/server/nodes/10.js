

export const index = 10;
let component_cache;
export const component = async () => component_cache ??= (await import('../entries/pages/todos/dark/_page.svelte.js')).default;
export const imports = ["_app/immutable/nodes/10.CQWo7u_W.js","_app/immutable/chunks/DDE5K5xA.js"];
export const stylesheets = [];
export const fonts = [];
