import { a as attr_class, s as stringify } from "../../../chunks/vendor.js";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    $$renderer2.push(`<div class="p-8 h-full bg-[#050000] grid grid-cols-2 gap-8 font-mono"><div class="border border-red-900/40 p-6 bg-red-950/5 flex flex-col items-center justify-center gap-6"><h2 class="text-red-500 text-xl font-black italic tracking-tighter uppercase">Bio-Dome Shield</h2> <div${attr_class(`w-48 h-48 rounded-full border-2 border-red-900 border-dashed flex items-center justify-center ${stringify("")}`)}><span class="text-red-600 font-bold text-4xl">SEC</span></div> <button class="w-full py-4 border border-red-600 text-red-500 hover:bg-red-600 hover:text-black font-black transition-all">[ TRIGGER_ISOLATION_CHECK ]</button></div> <div class="border border-red-900/10 p-6 text-[10px] text-gray-600 space-y-4"><h3 class="text-red-900 font-bold uppercase underline">Threat_Intelligence</h3> <p>> PODMAN_SHIELD: ACTIVE</p> <p>> ZIRCON_IPC: NOMINAL</p> <p>> HOST_LEAK_PROTECTION: 100%</p></div></div>`);
  });
}
export {
  _page as default
};
