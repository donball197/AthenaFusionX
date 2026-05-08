import { c as attr, a as attr_class, d as escape_html, s as stringify } from "../../../chunks/vendor.js";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let key = "";
    let loading = false;
    $$renderer2.push(`<div class="h-full flex items-center justify-center bg-[#020205]"><div class="w-80 glass-panel p-8 rounded-lg shadow-2xl"><h1 class="text-cyan-400 text-xs font-black mb-8 tracking-[5px] uppercase text-center">Identity_Verification</h1> <div class="space-y-4"><input type="password"${attr("value", key)}${attr_class(`w-full bg-black/50 border ${stringify("border-cyan-900")} p-3 text-xs text-cyan-400 outline-none focus:border-cyan-400 text-center font-mono transition-colors`)} placeholder="DNA_KEY_SIGNATURE" autocomplete="current-password"${attr("disabled", loading, true)}/> `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--> <button${attr("disabled", loading, true)} class="w-full py-3 bg-cyan-900/20 border border-cyan-500/50 text-[10px] text-cyan-400 font-bold uppercase hover:bg-cyan-400 hover:text-black transition-all disabled:opacity-40 disabled:cursor-not-allowed">${escape_html("Authorize_Uplink")}</button></div></div></div>`);
  });
}
export {
  _page as default
};
