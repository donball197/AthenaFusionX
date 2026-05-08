import { b as attr_style, s as stringify, d as escape_html, g as derived, a as attr_class } from "../../../chunks/vendor.js";
import "clsx";
import { s as systemVitals } from "../../../chunks/store.svelte.js";
function Heartbeat($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { resonance = 0.5 } = $$props;
    let pulseSpeed = derived(() => 0.5 + systemVitals.cpu_load * 2.5);
    $$renderer2.push(`<div class="heartbeat-container svelte-1v0e5yo"><svg viewBox="0 0 100 40" class="waveform svelte-1v0e5yo"><path d="M0 20 L30 20 L35 10 L45 30 L50 20 L100 20" fill="none" stroke="var(--pulse-color, #00ffff)" stroke-width="1.5"${attr_style(`--speed: ${stringify(pulseSpeed())}s`)} class="pulse-path svelte-1v0e5yo"></path></svg> <div class="resonance-label svelte-1v0e5yo">NEURAL_RESONANCE: ${escape_html((resonance * 1e3).toFixed(0))}Hz</div></div>`);
  });
}
function VitalsModule($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let cpuPercent = derived(() => (systemVitals.cpu_load * 100).toFixed(1));
    let ramVal = derived(() => 0);
    $$renderer2.push(`<div class="vitals-grid svelte-n0i5ux"><div class="stat-card svelte-n0i5ux"><div class="label svelte-n0i5ux">CPU_COMPUTE</div> <div class="value svelte-n0i5ux">${escape_html(cpuPercent())}%</div> <div class="bar-bg svelte-n0i5ux"><div class="bar-fill svelte-n0i5ux"${attr_style(`width: ${stringify(cpuPercent())}%`)}></div></div></div> <div class="stat-card svelte-n0i5ux"><div class="label svelte-n0i5ux">RAM_RESIDENT</div> <div class="value svelte-n0i5ux">${escape_html(ramVal().toFixed(2))} GB</div> <div class="bar-bg svelte-n0i5ux"><div class="bar-fill svelte-n0i5ux"${attr_style(`width: ${stringify(ramVal() / 4 * 100)}%`)}></div></div></div> <div${attr_class("stat-card svelte-n0i5ux", void 0, { "warning": systemVitals.temperature > 60 })}><div class="label svelte-n0i5ux">THERMAL_THRESHOLD</div> <div class="value svelte-n0i5ux">${escape_html(systemVitals.temperature.toFixed(1))}°C</div></div></div>`);
  });
}
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
 */ <div class="p-8 h-full bg-black flex flex-col"><header class="mb-8"><h1 class="text-cyan-400 text-2xl font-black tracking-widest">SYSTEM VITALS</h1> <p class="text-xs text-cyan-700">REAL-TIME HARDWARE + NEURAL RESONANCE</p></header> <div class="grid grid-cols-1 md:grid-cols-2 gap-6 flex-1"><div class="space-y-6">`);
    VitalsModule($$renderer2);
    $$renderer2.push(`<!----> <div class="border border-cyan-900/30 bg-[#0a0a0a] p-6 rounded">`);
    Heartbeat($$renderer2, { resonance: systemVitals.resonance });
    $$renderer2.push(`<!----></div></div> <div class="border border-cyan-900/30 bg-[#0a0a0a] p-6 font-mono text-xs overflow-auto custom-scrollbar svelte-3xqqso"><h2 class="text-cyan-400 mb-4 uppercase tracking-widest text-[10px]">RAW TELEMETRY</h2> <pre class="text-gray-400 whitespace-pre-wrap">${escape_html(JSON.stringify(
      {
        cpu: systemVitals.cpu_load,
        ram: systemVitals.ram_gb,
        temp: systemVitals.temperature,
        res: systemVitals.resonance
      },
      null,
      2
    ))}
            </pre></div></div></div>`);
  });
}
export {
  _page as default
};
