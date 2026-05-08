

export const index = 0;
let component_cache;
export const component = async () => component_cache ??= (await import('../entries/pages/_layout.svelte.js')).default;
export const universal = {
  "ssr": false
};
export const universal_id = "src/routes/+layout.js";
export const imports = ["_app/immutable/nodes/0.Ci8yq9GT.js","_app/immutable/chunks/DDE5K5xA.js","_app/immutable/chunks/CXcrBUwK.js"];
export const stylesheets = [];
export const fonts = [];
