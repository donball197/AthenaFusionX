

export const index = 9;
let component_cache;
export const component = async () => component_cache ??= (await import('../entries/pages/settings/_page.svelte.js')).default;
export const imports = ["_app/immutable/nodes/9.PKaE71ER.js","_app/immutable/chunks/DDE5K5xA.js"];
export const stylesheets = [];
export const fonts = [];
