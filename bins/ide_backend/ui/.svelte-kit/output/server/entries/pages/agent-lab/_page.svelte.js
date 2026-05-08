import "clsx";
import { e as ensure_array_like, d as escape_html, b as attr_style, s as stringify } from "../../../chunks/vendor.js";
function AgentMap($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let activeNodes = [];
    function getTierStyle(tier) {
      switch (tier) {
        case "STD":
          return "background: rgba(100,116,139,.15); color: #94a3b8; border-color: rgba(100,116,139,.3);";
        case "PRO":
          return "background: rgba(34,211,238,.1); color: #00ffff; border-color: rgba(34,211,238,.3);";
        case "ELITE":
          return "background: rgba(192,132,252,.1); color: #c084fc; border-color: rgba(192,132,252,.3);";
        case "APEX":
          return "background: rgba(251,191,36,.1); color: #fbbf24; border-color: rgba(251,191,36,.3);";
        default:
          return "background: rgba(255,255,255,.1); color: #fff; border-color: rgba(255,255,255,.2);";
      }
    }
    $$renderer2.push(`<div class="factory-grid svelte-xisutb"><!--[-->`);
    const each_array = ensure_array_like(activeNodes);
    for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
      let node = each_array[$$index];
      $$renderer2.push(`<div class="agent-card glass-panel svelte-xisutb"><div class="card-hdr svelte-xisutb"><span class="status-pulse svelte-xisutb"></span> <span class="handle-lbl svelte-xisutb">${escape_html(node.handle)}</span> <span class="tier-badge svelte-xisutb"${attr_style(getTierStyle(node.model?.tier))}>${escape_html(node.model?.tier ?? "STD")}</span></div> <div class="card-body svelte-xisutb"><div class="spec svelte-xisutb"><span class="label svelte-xisutb">CORE:</span> <span class="val text-cyan-400 svelte-xisutb">${escape_html(node.model?.name ?? "UNKNOWN")}</span></div> <div class="spec svelte-xisutb"><span class="label svelte-xisutb">CONTEXT:</span> <span class="val text-purple-400 svelte-xisutb">${escape_html(((node.model?.context ?? 0) / 1024).toFixed(0))}K</span></div> <div class="load-bar svelte-xisutb"><div class="fill svelte-xisutb"${attr_style(`width:${stringify(node.progress)}%`)}></div></div></div> <div class="card-footer svelte-xisutb"><span class="uptime svelte-xisutb">SUBSTRATE_LINK_STABLE</span> <span class="percent svelte-xisutb">${escape_html(node.progress)}%</span></div></div>`);
    }
    $$renderer2.push(`<!--]--></div>`);
  });
}
function _page($$renderer) {
  Array.from({ length: 12 }, (_, i) => ({ id: i, target: i < 3 ? "ACTIVE" : "IDLE" }));
  $$renderer.push(`<!---->/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */ <div class="p-8 h-full bg-black overflow-y-auto"><header class="flex justify-between items-baseline mb-8"><h1 class="text-purple-400 font-bold tracking-widest uppercase italic">Swarm_Reflex_Lab</h1> <span class="text-[10px] text-purple-900">NODE_COUNT: 68</span></header> `);
  AgentMap($$renderer);
  $$renderer.push(`<!----></div>`);
}
export {
  _page as default
};
