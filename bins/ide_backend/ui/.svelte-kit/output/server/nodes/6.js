

export const index = 6;
let component_cache;
export const component = async () => component_cache ??= (await import('../entries/pages/auth/_page.svelte.js')).default;
export const imports = ["_app/immutable/nodes/6.BvpGl5QD.js","_app/immutable/chunks/DDE5K5xA.js","_app/immutable/chunks/CXcrBUwK.js"];
export const stylesheets = [];
export const fonts = [];
