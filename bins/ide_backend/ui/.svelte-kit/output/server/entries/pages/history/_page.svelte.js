import "clsx";
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    $$renderer2.push(`<!---->/**
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
 */ <div class="h-full bg-black p-6 flex flex-col gap-4 font-mono text-xs select-text"><header class="flex justify-between items-center border-b border-purple-900/30 pb-4 select-none"><div><h1 class="text-purple-400 font-black tracking-[4px] uppercase italic">Lazarus_Vault</h1> <p class="text-purple-900 text-[9px] mt-1">Temporal Ledger // File Snapshots</p></div> <button class="text-purple-700 hover:text-purple-400 text-lg transition-colors" aria-label="Refresh snapshots">⟳</button></header> `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--> `);
    {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="text-purple-800 animate-pulse text-center py-12">Scanning vault...</div>`);
    }
    $$renderer2.push(`<!--]--></div>`);
  });
}
export {
  _page as default
};
