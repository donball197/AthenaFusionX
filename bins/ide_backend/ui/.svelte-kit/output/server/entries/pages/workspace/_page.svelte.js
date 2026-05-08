import "clsx";
import { e as ensure_array_like, a as attr_class, s as stringify, d as escape_html, h as clsx, f as bind_props, c as attr } from "../../../chunks/vendor.js";
import { t as terminal } from "../../../chunks/terminal.svelte.js";
function TerminalPanel($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    $$renderer2.push(`<div class="flex flex-col h-full bg-[#0d1117] border border-gray-800 rounded shadow-lg font-mono text-sm"><div class="flex items-center px-4 py-2 bg-[#161b22] border-b border-gray-800 shadow-sm"><div class="flex gap-1.5 mr-4"><div class="w-2.5 h-2.5 rounded-full bg-[#ff5f56]"></div> <div class="w-2.5 h-2.5 rounded-full bg-[#ffbd2e]"></div> <div class="w-2.5 h-2.5 rounded-full bg-[#27c93f]"></div></div> <span class="text-gray-400 text-[10px] font-bold uppercase tracking-[0.2em]">Ghost_Cortex // Substrate_Output</span></div> <div class="flex-1 p-4 overflow-y-auto whitespace-pre-wrap break-words leading-relaxed scrollbar-thin svelte-slj7fa">`);
    const each_array = ensure_array_like(
      // Auto-scroll logic using Svelte 5 Runes
      // Track logs length for reactivity
      terminal.logs
    );
    if (each_array.length !== 0) {
      $$renderer2.push("<!--[-->");
      for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
        let log = each_array[$$index];
        $$renderer2.push(`<div${attr_class(`${stringify(log.cls)} mb-1.5 flex gap-3`, "svelte-slj7fa")}><span class="opacity-20 shrink-0">[${escape_html(log.timestamp)}]</span> <span class="flex-1">${escape_html(log.message)}</span></div>`);
      }
    } else {
      $$renderer2.push("<!--[!-->");
      $$renderer2.push(`<div class="text-gray-700 italic">Awaiting neural uplink...</div>`);
    }
    $$renderer2.push(`<!--]--> `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div></div>`);
  });
}
function FileExplorerPanel($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { activeFile = void 0 } = $$props;
    let currentPath = ".";
    let files = [];
    $$renderer2.push(`<div class="flex flex-col h-full bg-[#0d1117] border border-gray-800 rounded shadow-lg font-mono text-sm text-gray-300"><div class="flex items-center justify-between px-3 py-2 bg-[#161b22] border-b border-gray-800 shadow-sm"><span class="text-cyan-500 text-xs font-semibold uppercase tracking-wider">DNA_INDEX // ${escape_html(currentPath)}</span> `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div> <div class="flex-1 overflow-y-auto p-2">`);
    if (files.length === 0) {
      $$renderer2.push("<!--[1-->");
      $$renderer2.push(`<div class="text-gray-600 italic px-2">Directory empty.</div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<!--[-->`);
      const each_array = ensure_array_like(files);
      for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
        let file = each_array[$$index];
        $$renderer2.push(`<button class="w-full text-left px-2 py-1 flex items-center gap-2 hover:bg-gray-800/50 hover:text-cyan-300 rounded transition-colors group"><span${attr_class(clsx(file.is_dir ? "text-amber-500/80" : "text-gray-500"))}>${escape_html(file.is_dir ? "📁" : "📄")}</span> <span class="truncate">${escape_html(file.name)}</span></button>`);
      }
      $$renderer2.push(`<!--]-->`);
    }
    $$renderer2.push(`<!--]--></div></div>`);
    bind_props($$props, { activeFile });
  });
}
function EditorPanel($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { activeFile } = $$props;
    let fileContent = "";
    let saving = false;
    $$renderer2.push(`<div class="flex flex-col h-full bg-[#0d1117] border border-gray-800 rounded overflow-hidden"><div class="flex items-center justify-between px-4 py-2 bg-[#161b22] border-b border-gray-800"><span class="text-purple-400 text-[10px] font-bold tracking-widest uppercase">AST_Buffer // ${escape_html(activeFile || "IDLE")}</span> <button${attr("disabled", !activeFile || saving, true)} class="text-[9px] px-2 py-1 border border-gray-700 text-gray-400 hover:text-green-400 hover:border-green-900 transition-all disabled:opacity-30">${escape_html("[SAVE_STATE]")}</button></div> <div class="flex-1 relative">`);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--> <textarea${attr("disabled", !activeFile, true)} spellcheck="false" class="w-full h-full p-6 bg-black text-gray-300 resize-none outline-none focus:ring-0 leading-relaxed font-mono text-sm whitespace-pre">`);
    const $$body = escape_html(fileContent);
    if ($$body) {
      $$renderer2.push(`${$$body}`);
    }
    $$renderer2.push(`</textarea></div></div>`);
  });
}
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let activeFile = null;
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      $$renderer3.push(`<div class="flex flex-col h-screen bg-black text-cyan-500 font-mono p-4 gap-4 overflow-hidden"><div class="shrink-0"><h1 class="text-lg font-bold mb-4 border-b border-cyan-900/40 pb-2 italic uppercase tracking-widest pl-2">WORKSPACE_DNA_MAP // COMMAND UPLINK</h1> <div class="grid grid-cols-1 md:grid-cols-3 gap-6"><div class="p-4 border border-cyan-900/30 bg-cyan-950/10 rounded"><h2 class="text-[10px] text-cyan-400 font-black tracking-widest mb-3 uppercase">Build_Substrate</h2> <div class="space-y-1"><button class="ws-btn text-cyan-200 svelte-1edgkge">▶ CARGO CHECK</button> <button class="ws-btn svelte-1edgkge">▶ UI RE-LINK</button></div></div> <div class="p-4 border border-purple-900/30 bg-purple-950/10 rounded"><h2 class="text-[10px] text-purple-400 font-black tracking-widest mb-3 uppercase">Neural_Context</h2> <div class="space-y-1"><button class="ws-btn svelte-1edgkge">▶ SECURITY AUDIT</button> <button class="ws-btn svelte-1edgkge">▶ SCAN SUBSTRATE</button></div></div> <div class="p-4 border border-amber-900/30 bg-amber-950/10 rounded"><h2 class="text-[10px] text-amber-400 font-black tracking-widest mb-3 uppercase">Lazarus_Vault</h2> <div class="space-y-1"><button class="ws-btn svelte-1edgkge">▶ VIEW_LEDGER</button> <button class="ws-btn svelte-1edgkge">▶ SAVE_STATE</button></div></div></div></div> <div class="flex-1 flex gap-4 min-h-0"><div class="w-1/5 flex flex-col min-h-0">`);
      FileExplorerPanel($$renderer3, {
        get activeFile() {
          return activeFile;
        },
        set activeFile($$value) {
          activeFile = $$value;
          $$settled = false;
        }
      });
      $$renderer3.push(`<!----></div> <div class="w-1/2 flex flex-col min-h-0">`);
      EditorPanel($$renderer3, { activeFile });
      $$renderer3.push(`<!----></div> <div class="w-[30%] flex flex-col min-h-0">`);
      TerminalPanel($$renderer3);
      $$renderer3.push(`<!----></div></div></div>`);
    }
    do {
      $$settled = true;
      $$inner_renderer = $$renderer2.copy();
      $$render_inner($$inner_renderer);
    } while (!$$settled);
    $$renderer2.subsume($$inner_renderer);
  });
}
export {
  _page as default
};
