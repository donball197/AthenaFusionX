import { o as onDestroy, d as escape_html, e as ensure_array_like, a as attr_class, c as attr } from "./vendor.js";
import { v as vmoTelemetry } from "./store.svelte.js";
function VmoTable($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let vmos = [];
    let loading = true;
    let errorMsg = "";
    let pollTimer;
    async function fetchVmos() {
      if (document.visibilityState === "hidden") return;
      try {
        const r = await fetch("/api/vmos");
        if (r.ok) {
          const data = await r.json();
          vmos = data.vmos ?? [];
          errorMsg = "";
        } else {
          errorMsg = `HTTP ${r.status} — VMO data unavailable`;
        }
      } catch (e) {
        errorMsg = "Uplink to Kernel VMO Table severed.";
      } finally {
        loading = false;
      }
    }
    function schedulePoll() {
      clearTimeout(pollTimer);
      if (document.visibilityState === "visible") {
        pollTimer = setTimeout(
          async () => {
            await fetchVmos();
            schedulePoll();
          },
          2e3
        );
      }
    }
    function onVisChange() {
      if (document.visibilityState === "visible") {
        fetchVmos();
        schedulePoll();
      } else {
        clearTimeout(pollTimer);
      }
    }
    onDestroy(() => {
      clearTimeout(pollTimer);
      document.removeEventListener("visibilitychange", onVisChange);
    });
    $$renderer2.push(`<div class="vmo-container svelte-1bbsp6"><div class="header svelte-1bbsp6"><span class="title svelte-1bbsp6">◈ VIRTUAL MEMORY OBJECTS (VMO)</span> <span class="active-count svelte-1bbsp6">ACTIVE: <span class="highlight svelte-1bbsp6">${escape_html(vmoTelemetry.active_vmos)}</span></span></div> `);
    if (loading && vmos.length === 0) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="status-msg svelte-1bbsp6" style="color:#22d3ee">SCANNING KERNEL MEMORY MAP...</div>`);
    } else if (errorMsg) {
      $$renderer2.push("<!--[1-->");
      $$renderer2.push(`<div class="status-msg svelte-1bbsp6" style="color:#f87171">⚠ ${escape_html(errorMsg)}</div>`);
    } else if (vmos.length === 0) {
      $$renderer2.push("<!--[2-->");
      $$renderer2.push(`<div class="status-msg svelte-1bbsp6" style="color:#64748b">NO ACTIVE VMO ALLOCATIONS DETECTED.</div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<div class="table-wrap svelte-1bbsp6"><table class="svelte-1bbsp6"><thead><tr><th class="svelte-1bbsp6">HANDLE</th><th class="svelte-1bbsp6">SIZE (B)</th><th class="svelte-1bbsp6">COMMITTED</th><th class="svelte-1bbsp6">RAM USED</th><th class="svelte-1bbsp6">COW</th><th class="svelte-1bbsp6">PARENT</th></tr></thead><tbody><!--[-->`);
      const each_array = ensure_array_like(vmos);
      for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
        let vmo = each_array[$$index];
        $$renderer2.push(`<tr${attr_class("svelte-1bbsp6", void 0, { "is-cow": vmo.is_cow })}><td class="handle svelte-1bbsp6">${escape_html(vmo.handle)}</td><td class="numeric svelte-1bbsp6">${escape_html((vmo.size_bytes || 0).toLocaleString())}</td><td class="numeric svelte-1bbsp6">${escape_html(vmo.committed_pages)}</td><td class="ram-highlight svelte-1bbsp6">${escape_html((vmo.memory_used_kb || 0).toLocaleString())} KB</td><td class="boolean svelte-1bbsp6">`);
        if (vmo.is_cow) {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<span class="badge cow-badge svelte-1bbsp6">YES</span>`);
        } else {
          $$renderer2.push("<!--[-1-->");
          $$renderer2.push(`<span class="badge std-badge svelte-1bbsp6">NO</span>`);
        }
        $$renderer2.push(`<!--]--></td><td class="parent svelte-1bbsp6">`);
        if (vmo.parent_handle !== "None") {
          $$renderer2.push("<!--[0-->");
          $$renderer2.push(`<span class="parent-link svelte-1bbsp6">↳ ${escape_html(vmo.parent_handle)}</span>`);
        } else {
          $$renderer2.push("<!--[-1-->");
          $$renderer2.push(`<span style="color:#374151">-</span>`);
        }
        $$renderer2.push(`<!--]--></td></tr>`);
      }
      $$renderer2.push(`<!--]--></tbody></table></div>`);
    }
    $$renderer2.push(`<!--]--></div>`);
  });
}
function MemoryVault($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let memories = [];
    let searchQ = "";
    let purging = false;
    $$renderer2.push(`<div class="mv" style="display:flex;flex-direction:column;height:100%;background:#020008;font-family:'JetBrains Mono',monospace;"><div style="flex:none;display:flex;align-items:center;justify-content:space-between;padding:12px 16px;border-bottom:1px solid rgba(167,139,250,.1);background:#04000f;"><div style="font-size:11px;font-weight:900;letter-spacing:.15em;color:#a78bfa;display:flex;align-items:center;gap:6px;">◈ NEURAL_STRANDS</div> <div style="font-size:9px;color:rgba(167,139,250,.3);">${escape_html(memories.length)} STORED</div></div> <div style="flex:none;padding:10px 12px;border-bottom:1px solid rgba(167,139,250,.06);"><div style="display:flex;gap:4px;margin-bottom:6px;"><input${attr("value", searchQ)} placeholder="search memory..." style="flex:1;background:#000;border:1px solid rgba(167,139,250,.2);color:#c4b5fd;font-size:10px;padding:5px 8px;outline:none;font-family:'JetBrains Mono',monospace;"/> <button style="background:rgba(167,139,250,.1);border:1px solid rgba(167,139,250,.2);color:#a78bfa;font-size:8px;font-weight:900;padding:0 10px;cursor:pointer;">SEARCH</button> <button style="background:rgba(167,139,250,.1);border:1px solid rgba(167,139,250,.2);color:#a78bfa;font-size:8px;font-weight:900;padding:0 10px;cursor:pointer;">ALL</button></div> <div style="display:flex;align-items:center;gap:8px;"><button${attr("disabled", purging, true)} style="background:transparent;border:1px solid rgba(248,113,113,.2);color:rgba(248,113,113,.5);font-size:8px;font-weight:900;padding:3px 10px;cursor:pointer;font-family:'JetBrains Mono',monospace;">⚠ PURGE ALL</button> `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div></div> <div style="flex:1; padding:10px; min-height: 250px;">`);
    VmoTable($$renderer2);
    $$renderer2.push(`<!----></div> <div style="flex:1; overflow-y:auto; padding:8px; border-top: 1px solid rgba(167,139,250,.2);"><div style="font-size:8px;font-weight:900;letter-spacing:.15em;color:rgba(167,139,250,.4);margin-bottom:8px;padding-left:4px;">HISTORICAL_STRANDS</div> `);
    {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div style="display:flex;flex-direction:column;align-items:center;gap:12px;padding:20px 0;color:rgba(167,139,250,.3);font-size:9px;letter-spacing:.1em;"><div style="width:28px;height:28px;border-radius:50%;border:2px solid rgba(167,139,250,.1);border-top-color:#a78bfa;animation:spin 1s linear infinite;"></div> SCANNING NEURAL ARCHIVE...</div>`);
    }
    $$renderer2.push(`<!--]--></div></div>`);
  });
}
export {
  MemoryVault as M
};
