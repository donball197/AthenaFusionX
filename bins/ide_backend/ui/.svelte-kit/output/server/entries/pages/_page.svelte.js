import { o as onDestroy, a as attr_class, e as ensure_array_like, b as attr_style, s as stringify, c as attr, d as escape_html, f as bind_props, g as derived } from "../../chunks/vendor.js";
import "clsx";
import { s as systemVitals, w as wsState, i as ide, v as vmoTelemetry } from "../../chunks/store.svelte.js";
import { a as addLog, t as terminal } from "../../chunks/terminal.svelte.js";
import { M as MemoryVault } from "../../chunks/MemoryVault.js";
function buildHeaders(extra = {}) {
  const headers = { "Content-Type": "application/json", ...extra };
  if (auth.token) {
    headers["Authorization"] = `Bearer ${auth.token}`;
  }
  return headers;
}
async function request(url, options = {}) {
  const { headers: extraHeaders, ...rest } = options;
  const headers = buildHeaders(extraHeaders ?? {});
  try {
    const res = await fetch(url, { ...rest, headers });
    if (res.status === 401) {
      console.warn(`!! [UPLINK] 401 Unauthorized at ${url}. Token invalid or missing.`);
    }
    if (!res.ok) throw new Error(`HTTP ${res.status} — ${url}`);
    return res;
  } catch (e) {
    throw e;
  }
}
const uplink = {
  fetch: (url, options = {}) => request(url, options),
  post: (url, body, signal) => request(url, { method: "POST", body: JSON.stringify(body), signal }),
  put: (url, body, signal) => request(url, { method: "PUT", body: JSON.stringify(body), signal }),
  delete: (url, signal) => request(url, { method: "DELETE", signal }),
  get: (url, signal) => request(url, { signal }),
  json: async (url, opts) => (await request(url, opts)).json()
};
const auth = { token: "", isInitialized: false };
const ui = {
  activeView: "WORKSPACE"
};
const _lastFire = {};
const DEBOUNCE_MS = 200;
async function ensureAuth() {
  if (auth.token) return;
  try {
    const res = await fetch("/api/auth/bootstrap");
    if (res.ok) {
      const data = await res.json();
      auth.token = data.token;
    }
  } catch (e) {
    console.warn("[TaskService] Bootstrap auth failed:", e);
  }
}
const TaskService = {
  /**
   * log: Centralized proxy for UI components to write to the 
   * terminal log list.
   */
  log(message, cls = "text-gray-400") {
    addLog(message, cls);
  },
  async runDirective(directive, source = "VISOR", signal) {
    const now = Date.now();
    if (now - (_lastFire[source] ?? 0) < DEBOUNCE_MS) return;
    _lastFire[source] = now;
    await ensureAuth();
    try {
      await uplink.post("/api/task", { directive }, signal);
    } catch (e) {
      console.error(`!! [${source}] Uplink Fracture: ${e.message}`);
      this.log(`!! [${source}] Uplink Fracture: ${e.message}`, "text-rose-500");
    }
  }
};
class NexusSocket {
  socket = null;
  url;
  listeners = /* @__PURE__ */ new Set();
  retries = 0;
  retryTimer;
  watchdogTimer;
  lastHeartbeat = Date.now();
  status = "DISCONNECTED";
  telemetry = { status: "IDLE", active_vmos: 0, resonance: 0 };
  constructor(path) {
    if (typeof window === "undefined") {
      this.url = `ws://localhost:3000${path}`;
    } else {
      const proto = window.location.protocol === "https:" ? "wss:" : "ws:";
      this.url = `${proto}//${window.location.host}${path}`;
    }
  }
  connect() {
    if (this.socket?.readyState === WebSocket.OPEN || this.socket?.readyState === WebSocket.CONNECTING) return;
    this.socket = new WebSocket(this.url);
    this.socket.onopen = () => {
      this.status = "CONNECTED";
      this.telemetry.status = "CONNECTED";
      this.retries = 0;
      this.lastHeartbeat = Date.now();
      this.emit({ _internal: "CONNECTION_STATE", status: "CONNECTED" });
      this.startWatchdog();
    };
    this.socket.onmessage = (e) => {
      this.lastHeartbeat = Date.now();
      try {
        const msg = JSON.parse(e.data);
        if (!msg.type && !msg.type_id && msg._internal === void 0) return;
        if (msg.active_vmos !== void 0) this.telemetry.active_vmos = msg.active_vmos;
        if (msg.resonance !== void 0) this.telemetry.resonance = msg.resonance;
        this.emit(msg);
      } catch {
        console.warn("[SOCKET] Dropped invalid JSON frame");
      }
    };
    this.socket.onclose = () => {
      this.status = "DISCONNECTED";
      this.telemetry.status = "DISCONNECTED";
      this.emit({ _internal: "CONNECTION_STATE", status: "DISCONNECTED" });
      clearInterval(this.watchdogTimer);
      if (this.retries < 15) this.retries++;
      const delay = Math.min(1e3 * 2 ** this.retries, 16e3);
      this.retryTimer = setTimeout(() => this.connect(), delay);
    };
  }
  send(payload) {
    if (this.socket?.readyState === WebSocket.OPEN) {
      const data = typeof payload === "string" ? { directive: payload } : payload;
      this.socket.send(JSON.stringify(data));
    }
  }
  subscribe(fn) {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }
  emit(msg) {
    this.listeners.forEach((l) => l(msg));
  }
  startWatchdog() {
    clearInterval(this.watchdogTimer);
    this.watchdogTimer = setInterval(() => {
      if (this.status === "CONNECTED" && Date.now() - this.lastHeartbeat > 15e3) {
        console.warn("[WATCHDOG] Socket frozen — forcing reconnect.");
        this.socket?.close();
      }
    }, 5e3);
  }
  destroy() {
    clearTimeout(this.retryTimer);
    clearInterval(this.watchdogTimer);
    this.socket?.close();
    this.listeners.clear();
  }
}
const nexusSocket = new NexusSocket("/ws/telemetry");
const agentRegistry = {
  modules: []
};
function NavHeader($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { activeNav = "WORKSPACE" } = $$props;
    const TABS = [
      { id: "NEXUS_OS", color: "#00ffff" },
      { id: "IDE", color: "#22d3ee" },
      { id: "WORKSPACE", color: "#34d399" },
      { id: "VITALS", color: "#fb923c" },
      { id: "MEMORY", color: "#a78bfa" },
      { id: "SECOPS", color: "#f87171" },
      { id: "AGENT-LAB", color: "#fbbf24" },
      { id: "HISTORY", color: "#818cf8" },
      { id: "DYNAMIC_UI", color: "#e879f9" },
      { id: "COGNITIVE_MESH", color: "#f472b6" }
    ];
    let clockTime = "00:00:00";
    let showOnline = false;
    let _clockInt;
    let _graceTimer;
    onDestroy(() => {
      clearInterval(_clockInt);
      clearTimeout(_graceTimer);
    });
    let dotLive = derived(() => showOnline);
    let tempColor = derived(() => "#34d399");
    $$renderer2.push(`<header class="nexus-nav svelte-1m1lios"><div class="nav-left svelte-1m1lios"><button class="hamburger svelte-1m1lios" title="Toggle sidebar" aria-label="Toggle sidebar"><span class="svelte-1m1lios"></span><span class="svelte-1m1lios"></span><span class="svelte-1m1lios"></span></button> <div class="brand svelte-1m1lios"><span class="brand-n svelte-1m1lios">NEXUS</span> <span class="brand-s svelte-1m1lios">://</span> <span class="brand-v svelte-1m1lios">SINGULARITY</span> <span${attr_class("brand-dot svelte-1m1lios", void 0, { "dot-live": dotLive() })}></span></div></div> <nav class="tab-strip svelte-1m1lios" aria-label="Main navigation"><!--[-->`);
    const each_array = ensure_array_like(TABS);
    for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
      let t = each_array[$$index];
      $$renderer2.push(`<button${attr_class("nav-tab svelte-1m1lios", void 0, { "is-active": activeNav === t.id })}${attr_style(`--c:${stringify(t.color)}`)}${attr("aria-current", activeNav === t.id ? "page" : void 0)}>${escape_html(t.id)}</button>`);
    }
    $$renderer2.push(`<!--]--></nav> <div class="nav-right svelte-1m1lios"><span class="metric svelte-1m1lios"${attr_style(`color:${stringify(tempColor())}`)} title="Temperature">🌡 ${escape_html(systemVitals.temperature.toFixed(1))}°C</span> <span class="metric-sep svelte-1m1lios">│</span> <span class="metric svelte-1m1lios" style="color:#22d3ee" title="CPU Load">⚡ ${escape_html((0 * 100).toFixed(0))}%</span> <span class="metric-sep svelte-1m1lios">│</span> <span class="metric svelte-1m1lios" style="color:#a78bfa" title="RAM Used">◈ ${escape_html(0 .toFixed(2))}GB</span> <span class="metric-sep svelte-1m1lios">│</span> <span class="clock svelte-1m1lios">${escape_html(clockTime)}</span> <span${attr_class("ws-pill svelte-1m1lios", void 0, { "ws-live": wsState.ready })}>${escape_html("○ INIT")}</span> <button class="ghost-toggle svelte-1m1lios" title="Toggle Ghost Cortex" aria-label="Toggle Ghost Cortex">◈</button></div></header>`);
    bind_props($$props, { activeNav });
  });
}
function GhostBlade($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let directive = "";
    let sending = false;
    const QUICK = [
      {
        label: "CARGO CHECK",
        d: "run cargo check and show all errors"
      },
      {
        label: "NPM BUILD",
        d: "run npm run build in the ui directory"
      },
      {
        label: "AUDIT SRC",
        d: "audit all rust source files for security vulnerabilities"
      },
      {
        label: "SCAN TODOS",
        d: "find all TODO FIXME HACK and SUTURE comments"
      },
      {
        label: "LIST MEM",
        d: "list the 10 most recent memory strands"
      },
      {
        label: "GIT STATUS",
        d: "run git status and show all changed files"
      }
    ];
    {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<aside class="ghost-blade svelte-wl8dw9"><div class="ghost-hdr svelte-wl8dw9"><div class="ghost-brand svelte-wl8dw9"><span class="ghost-icon svelte-wl8dw9">◈</span> <span class="ghost-name svelte-wl8dw9">GHOST_CORTEX</span></div> <button class="ghost-close svelte-wl8dw9" aria-label="Close Ghost Cortex">×</button></div> <div class="quick-strip svelte-wl8dw9"><!--[-->`);
      const each_array = ensure_array_like(QUICK);
      for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
        let q = each_array[$$index];
        $$renderer2.push(`<button class="quick-btn svelte-wl8dw9"${attr("title", q.d)}>${escape_html(q.label)}</button>`);
      }
      $$renderer2.push(`<!--]--></div> <div class="stream-area svelte-wl8dw9">`);
      {
        $$renderer2.push("<!--[-1-->");
        $$renderer2.push(`<div class="idle-state svelte-wl8dw9"><div class="idle-icon svelte-wl8dw9">◈</div> <div class="idle-title svelte-wl8dw9">GHOST CORTEX</div> <div class="idle-hint svelte-wl8dw9">AWAITING DIRECTIVE</div></div>`);
      }
      $$renderer2.push(`<!--]--></div> <div class="section directive-section svelte-wl8dw9"><div class="section-hdr svelte-wl8dw9"><span class="svelte-wl8dw9">DIRECTIVE</span> `);
      {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<span class="ws-warn svelte-wl8dw9">⚡ UPLINK SEVERED</span>`);
      }
      $$renderer2.push(`<!--]--></div> <textarea class="dir-textarea svelte-wl8dw9" placeholder="Enter directive... Enter to send, Shift+Enter for newline" rows="3"${attr("disabled", sending, true)}>`);
      const $$body = escape_html(directive);
      if ($$body) {
        $$renderer2.push(`${$$body}`);
      }
      $$renderer2.push(`</textarea> <button${attr_class("fire-btn svelte-wl8dw9", void 0, { "fire-sending": sending })}${attr("disabled", sending, true)}>${escape_html("▶ FIRE DIRECTIVE")}</button></div></aside>`);
    }
    $$renderer2.push(`<!--]-->`);
  });
}
function Sidebar($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let files = [];
    let breadcrumb = ["."];
    let newFileName = "";
    let creating = false;
    const FILE_ICONS = {
      rs: "🦀",
      js: "🟨",
      ts: "🔷",
      svelte: "🔶",
      toml: "⚙",
      md: "📝",
      json: "📋",
      zig: "⚡",
      css: "🎨",
      html: "🌐",
      sh: "💻",
      py: "🐍",
      lock: "🔒",
      env: "🔑"
    };
    function fileIcon(name) {
      const ext = name.split(".").at(-1) ?? "";
      return FILE_ICONS[ext] ?? "📄";
    }
    $$renderer2.push(`<div class="sidebar-root svelte-129hoe0"><div class="path-bar svelte-129hoe0">`);
    if (breadcrumb.length > 1) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<button class="up-btn svelte-129hoe0" title="Go up">‹ UP</button>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--> <span class="path-label svelte-129hoe0">${escape_html("~/")}</span> `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div> <div class="create-zone svelte-129hoe0"><div class="zone-label svelte-129hoe0">NEW FILE</div> <form class="create-row svelte-129hoe0"><input${attr("value", newFileName)} placeholder="e.g. module.rs" class="create-inp svelte-129hoe0"${attr("disabled", creating, true)} autocomplete="off" spellcheck="false"/> <button type="submit" class="create-btn svelte-129hoe0"${attr("disabled", creating, true)} title="Create">${escape_html("+")}</button></form> `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div> <div class="tree-label svelte-129hoe0">DNA_STRUCTURE</div> <div class="tree-body svelte-129hoe0">`);
    if (files.length === 0 && true) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="tree-empty svelte-129hoe0">EMPTY DIRECTORY</div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--> <!--[-->`);
    const each_array = ensure_array_like(files.filter((f) => f.is_dir));
    for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
      let d = each_array[$$index];
      $$renderer2.push(`<button class="tree-item dir-item svelte-129hoe0"${attr("title", d.path)}><span class="ti-icon svelte-129hoe0">📁</span> <span class="ti-name svelte-129hoe0">${escape_html(d.name)}/</span> <span class="ti-arrow svelte-129hoe0">›</span></button>`);
    }
    $$renderer2.push(`<!--]--> <!--[-->`);
    const each_array_1 = ensure_array_like(files.filter((f) => !f.is_dir));
    for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
      let f = each_array_1[$$index_1];
      $$renderer2.push(`<button class="tree-item file-item svelte-129hoe0"${attr("title", f.path)}><span class="ti-icon svelte-129hoe0">${escape_html(fileIcon(f.name))}</span> <span class="ti-name svelte-129hoe0">${escape_html(f.name)}</span></button>`);
    }
    $$renderer2.push(`<!--]--></div> <button class="refresh-bar svelte-129hoe0" title="Refresh file tree">⟳ REFRESH DNA</button></div>`);
  });
}
function StatusBar($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let showStatus = false;
    let _graceTimer;
    onDestroy(() => clearTimeout(_graceTimer));
    let path = derived(() => "");
    let lang = derived(() => {
      const ext = path().split(".").at(-1)?.toLowerCase() ?? "";
      const m = {
        rs: "RUST",
        js: "JS",
        ts: "TS",
        svelte: "SVELTE",
        zig: "ZIG",
        toml: "TOML",
        md: "MD",
        json: "JSON",
        sh: "BASH",
        css: "CSS",
        html: "HTML"
      };
      return m[ext] ?? "TXT";
    });
    let isDirty = derived(() => ide.openTabs.find((t) => t.path === path())?.isDirty ?? false);
    let tempColor = derived(() => "#34d399");
    let isOnline = derived(() => true);
    $$renderer2.push(`<footer class="status-bar svelte-1piydef"><div class="sb-left svelte-1piydef"><span${attr_class("sb-item svelte-1piydef", void 0, { "sb-live": isOnline(), "sb-dead": showStatus })}>${escape_html("● INIT")}</span> <span class="sb-sep svelte-1piydef">│</span> <span class="sb-item svelte-1piydef">NEXUS v4.3.0</span> <span class="sb-sep svelte-1piydef">│</span> <span class="sb-item svelte-1piydef">ARM64 CROSTINI</span></div> <div class="sb-center svelte-1piydef">`);
    if (path()) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<span class="sb-path svelte-1piydef">${escape_html(path())}</span> <span class="sb-sep svelte-1piydef">·</span> <span class="sb-lang svelte-1piydef">${escape_html(lang())}</span> `);
      if (isDirty()) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<span class="sb-dirty svelte-1piydef">● MODIFIED</span>`);
      } else {
        $$renderer2.push("<!--[-1-->");
      }
      $$renderer2.push(`<!--]-->`);
    } else {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<span class="sb-none svelte-1piydef">NO FILE ACTIVE</span>`);
    }
    $$renderer2.push(`<!--]--></div> <div class="sb-right svelte-1piydef"><span class="sb-item svelte-1piydef"${attr_style(`color:${stringify(tempColor())}`)}>🌡 ${escape_html(systemVitals.temperature.toFixed(1))}°C</span> <span class="sb-sep svelte-1piydef">│</span> <span class="sb-item svelte-1piydef" style="color:#22d3ee">⚡ ${escape_html((0 * 100).toFixed(0))}%</span> <span class="sb-sep svelte-1piydef">│</span> <span class="sb-item svelte-1piydef" style="color:#a78bfa">◈ ${escape_html(0 .toFixed(2))}GB</span> <span class="sb-sep svelte-1piydef">│</span> <button class="sb-btn svelte-1piydef">${escape_html("◁ SIDEBAR")}</button> <span class="sb-sep svelte-1piydef">│</span> <button class="sb-btn svelte-1piydef">${escape_html("▷ GHOST")}</button></div></footer>`);
  });
}
function Terminal($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let inputVal = "";
    $$renderer2.push(`<div class="term-root svelte-fe92n4"><div class="term-hdr svelte-fe92n4"><span class="term-title svelte-fe92n4">▶ NEURAL_OUTPUT</span> `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--> <div class="term-actions svelte-fe92n4"><button class="t-btn svelte-fe92n4" title="Clear terminal">CLR</button> <button class="t-btn svelte-fe92n4">${escape_html("⤓ AUTO")}</button></div></div> <div class="term-output svelte-fe92n4"><!--[-->`);
    const each_array = ensure_array_like(terminal.logs);
    for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
      let log = each_array[$$index];
      $$renderer2.push(`<div${attr_class(`log-row ${stringify(log.cls)}`, "svelte-fe92n4")}><span class="log-ts svelte-fe92n4">${escape_html(log.timestamp)}</span> <span class="log-msg svelte-fe92n4">${escape_html(log.message)}</span></div>`);
    }
    $$renderer2.push(`<!--]--> `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div> <div class="term-inp-row svelte-fe92n4"><span class="prompt svelte-fe92n4">❯</span> <input${attr("value", inputVal)} class="term-inp svelte-fe92n4" placeholder="send directive..." autocomplete="off" spellcheck="false"/> <button class="send-btn svelte-fe92n4">SEND</button></div></div>`);
  });
}
function ModuleRenderer($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { activeId } = $$props;
    let m = derived(() => agentRegistry.modules.find((x) => x.id === activeId));
    if (m()) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="h-full border border-cyan-500/20 bg-cyan-950/10 flex flex-col font-mono text-xs"><div class="p-2 border-b border-cyan-500/20 bg-black flex justify-between text-cyan-400"><b>${escape_html(m().id)}</b><span class="opacity-50">PROVENANCE: ${escape_html(m().generatedBy || "UNKNOWN")}</span></div> <div class="p-4 flex-1 overflow-auto text-gray-300 whitespace-pre-wrap">${escape_html(m().render.source || `[${m().render.kind.toUpperCase()} MOCK]`)}</div> `);
      if (m().render.actions) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<div class="p-3 border-t border-cyan-500/20 flex gap-2 flex-wrap"><!--[-->`);
        const each_array = ensure_array_like(m().render.actions);
        for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
          let a = each_array[$$index];
          $$renderer2.push(`<button class="border border-cyan-500/30 px-3 py-1 hover:bg-cyan-500/20 text-cyan-400">${escape_html(a.label)}</button>`);
        }
        $$renderer2.push(`<!--]--></div>`);
      } else {
        $$renderer2.push("<!--[-1-->");
      }
      $$renderer2.push(`<!--]--></div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<div class="h-full flex items-center justify-center font-mono text-gray-700 tracking-widest">AWAITING DYNAMIC MODULE</div>`);
    }
    $$renderer2.push(`<!--]-->`);
  });
}
function IdePanel($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let autoSave = false;
    let enableVim = true;
    onDestroy(() => {
    });
    $$renderer2.push(`<div class="ide-shell svelte-1ibfzvu"><div class="tab-bar svelte-1ibfzvu"><!--[-->`);
    const each_array = ensure_array_like(ide.openTabs);
    for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
      let tab = each_array[$$index];
      $$renderer2.push(`<button type="button"${attr_class("ide-tab svelte-1ibfzvu", void 0, { "tab-active": ide.activeTab === tab.path })} role="tab"${attr("aria-selected", ide.activeTab === tab.path)}><span class="tab-name svelte-1ibfzvu">${escape_html(tab.path.split("/").at(-1))}</span> `);
      if (tab.isDirty) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<span class="tab-dirty svelte-1ibfzvu">●</span>`);
      } else {
        $$renderer2.push("<!--[-1-->");
      }
      $$renderer2.push(`<!--]--> <span class="tab-close svelte-1ibfzvu" role="button" tabindex="0" aria-label="Close tab">×</span></button>`);
    }
    $$renderer2.push(`<!--]--> <div class="tab-controls svelte-1ibfzvu"><button${attr_class("vim-toggle svelte-1ibfzvu", void 0, { "on": enableVim })}>VIM: ${escape_html("ON")}</button> <button${attr_class("vim-toggle svelte-1ibfzvu", void 0, { "on": autoSave })}>AUTO: ${escape_html("OFF")}</button> `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></div></div> <div class="editor-pane svelte-1ibfzvu">`);
    {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<div class="ide-welcome svelte-1ibfzvu"><div class="iw-glyph svelte-1ibfzvu">◈</div> <div class="iw-title svelte-1ibfzvu">AWAITING_INPUT</div></div>`);
    }
    $$renderer2.push(`<!--]--></div> <div class="ide-term-strip svelte-1ibfzvu">`);
    Terminal($$renderer2);
    $$renderer2.push(`<!----></div></div>`);
  });
}
function CommandPalette($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]-->`);
  });
}
function WorkspaceShell($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    $$renderer2.push(`<div class="workspace-root svelte-5mcnqz"><header class="workspace-header svelte-5mcnqz"><div class="brand svelte-5mcnqz">NEXUS_OS <span class="badge svelte-5mcnqz">v16.0</span></div> <div class="tabs svelte-5mcnqz"><button${attr_class("svelte-5mcnqz", void 0, {
      "active": (
        // WorkspaceShell.svelte — v1.1 [IMPORT_FIX]
        // FIX: Added missing import for terminal store — was referenced but not imported,
        //      causing a build failure in strict TypeScript mode.
        ui.activeView === "WORKSPACE"
      )
    })}>WORKSPACE</button> <button${attr_class("svelte-5mcnqz", void 0, { "active": ui.activeView === "TASKS" })}>TASK PIPELINE</button></div> <div class="hint svelte-5mcnqz">Ctrl+K for Commands</div></header> <div class="workspace-body svelte-5mcnqz">`);
    {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<aside class="sidebar border-r border-gray-800 w-64 svelte-5mcnqz"><div class="pane-header svelte-5mcnqz">EXPLORER</div> <div class="p-4 text-xs text-gray-500 italic">Project DNA Index</div></aside>`);
    }
    $$renderer2.push(`<!--]--> <main class="editor-pane flex-1 svelte-5mcnqz">`);
    {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="flex items-center justify-center h-full text-gray-700 text-sm italic border border-dashed border-gray-800 m-4">[CodeMirror Engine Active]</div>`);
    }
    $$renderer2.push(`<!--]--></main> `);
    {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<aside class="ai-console border-l border-gray-800 w-80 flex flex-col svelte-5mcnqz"><div class="pane-header flex justify-between svelte-5mcnqz"><span>GHOST CORTEX</span> <span class="text-emerald-400 text-[9px]">${escape_html("ONLINE")}</span></div> <div class="flex-1 overflow-y-auto p-4 space-y-4 font-mono text-[10px]"><!--[-->`);
      const each_array = ensure_array_like(terminal.logs);
      for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
        let log = each_array[$$index];
        $$renderer2.push(`<div${attr_class(`${stringify(log.cls)} break-words`, "svelte-5mcnqz")}>${escape_html(log.message)}</div>`);
      }
      $$renderer2.push(`<!--]--> `);
      {
        $$renderer2.push("<!--[-1-->");
      }
      $$renderer2.push(`<!--]--></div></aside>`);
    }
    $$renderer2.push(`<!--]--></div> <footer class="workspace-footer border-t border-gray-800 h-48 flex flex-col svelte-5mcnqz"><div class="pane-header flex-none svelte-5mcnqz">INTEGRATED TERMINAL</div> <div class="flex-1 overflow-hidden">`);
    Terminal($$renderer2);
    $$renderer2.push(`<!----></div></footer> `);
    CommandPalette($$renderer2);
    $$renderer2.push(`<!----></div>`);
  });
}
function HistoryPanel($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    $$renderer2.push(`<div class="panel-scroll history-panel svelte-1jltp3m"><div class="hist-hdr svelte-1jltp3m"><div class="hist-title svelte-1jltp3m">LAZARUS_VAULT</div> <button class="refresh-btn svelte-1jltp3m" title="Refresh snapshots">⟳</button></div> `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--> `);
    {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="snap-loading svelte-1jltp3m">SCANNING VAULT…</div>`);
    }
    $$renderer2.push(`<!--]--></div>`);
  });
}
function VitalsView($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let cpuHist = [];
    let tempHist = [];
    let ramHist = [];
    let pollTimer;
    async function tick() {
      cpuHist = [
        ...cpuHist.slice(-39),
        0 * 100
      ];
      tempHist = [...tempHist.slice(-39), systemVitals.temperature];
      ramHist = [...ramHist.slice(-39), 0];
    }
    function schedulePoll() {
      clearTimeout(pollTimer);
      if (document.visibilityState === "visible") {
        pollTimer = setTimeout(
          async () => {
            await tick();
            schedulePoll();
          },
          5e3
        );
      }
    }
    function onVisChange() {
      if (document.visibilityState === "visible") {
        tick();
        schedulePoll();
      } else {
        clearTimeout(pollTimer);
      }
    }
    onDestroy(() => {
      clearTimeout(pollTimer);
      document.removeEventListener("visibilitychange", onVisChange);
    });
    let tc = derived(() => "#34d399");
    let cc = derived(() => "#22d3ee");
    let ramMax = derived(() => Math.max(systemVitals.total_ram_gb, ...ramHist, 0.1));
    function sp(data, w, h, max) {
      if (data.length < 2) return "";
      return "M" + data.map((v, i) => `${i / (data.length - 1) * w},${h - v / max * h}`).join("L");
    }
    function arc(pct, r) {
      const a = Math.min(Math.max(pct, 0), 0.9999) * Math.PI;
      const x = 60 + r * Math.cos(Math.PI + a);
      const y = 60 + r * Math.sin(Math.PI + a);
      return `M ${60 - r} 60 A ${r} ${r} 0 ${a > Math.PI ? 1 : 0} 1 ${x} ${y}`;
    }
    $$renderer2.push(`<div style="display:flex;flex-direction:column;height:100%;padding:16px;gap:14px;background:#000;overflow-y:auto;font-family:'JetBrains Mono',monospace;"><div><div style="font-size:12px;font-weight:900;letter-spacing:.2em;color:#22d3ee;text-shadow:0 0 12px rgba(34,211,238,.4);">SYSTEM_VITALS</div> <div style="font-size:8px;color:rgba(0,255,255,.25);margin-top:2px;">REAL-TIME HARDWARE TELEMETRY // ARM64 CROSTINI</div></div> <div style="display:flex;gap:10px;"><!--[-->`);
    const each_array = ensure_array_like([
      {
        val: 0,
        max: 1,
        label: "CPU LOAD",
        fmt: (v) => `${(v * 100).toFixed(0)}%`,
        c: cc()
      },
      {
        val: systemVitals.temperature,
        max: 100,
        label: "THERMAL",
        fmt: (v) => `${v.toFixed(1)}°C`,
        c: tc()
      },
      {
        val: 0,
        max: ramMax(),
        label: "RAM USED",
        fmt: (v) => `${v.toFixed(1)}G`,
        c: "#a78bfa"
      }
    ]);
    for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
      let g = each_array[$$index];
      $$renderer2.push(`<div style="flex:1;display:flex;flex-direction:column;align-items:center;gap:4px;"><svg viewBox="0 0 120 70" style="width:100%;max-width:120px;"><path${attr("d", arc(1, 48))} fill="none" stroke="rgba(255,255,255,.05)" stroke-width="8" stroke-linecap="round"></path><path${attr("d", arc(g.val / g.max, 48))} fill="none"${attr("stroke", g.c)} stroke-width="8" stroke-linecap="round"${attr_style(`filter:drop-shadow(0 0 4px ${stringify(g.c)})`)}></path><text x="60" y="60" text-anchor="middle"${attr("fill", g.c)} font-family="JetBrains Mono,monospace" font-size="13" font-weight="900">${escape_html(g.fmt(g.val))}</text></svg> <div style="font-size:8px;font-weight:900;letter-spacing:.12em;color:rgba(255,255,255,.3);">${escape_html(g.label)}</div></div>`);
    }
    $$renderer2.push(`<!--]--></div> <!--[-->`);
    const each_array_1 = ensure_array_like([
      { data: cpuHist, max: 100, label: "CPU HISTORY", c: cc() },
      { data: tempHist, max: 100, label: "THERMAL HISTORY", c: tc() },
      {
        data: ramHist,
        max: ramMax(),
        label: "RAM HISTORY",
        c: "#a78bfa"
      }
    ]);
    for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
      let s = each_array_1[$$index_1];
      $$renderer2.push(`<div style="border:1px solid rgba(0,255,255,.06);padding:8px;background:rgba(0,0,0,.5);"><div${attr_style(`font-size:8px;font-weight:900;letter-spacing:.1em;color:${stringify(s.c)};margin-bottom:5px;`)}>${escape_html(s.label)}</div> <svg viewBox="0 0 160 40" style="width:100%;height:40px;display:block;" preserveAspectRatio="none">`);
      if (s.data.length >= 2) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<path${attr("d", `${stringify(sp(s.data, 160, 40, s.max))} L160,40 L0,40 Z`)}${attr("fill", s.c)} opacity=".2"></path><path${attr("d", sp(s.data, 160, 40, s.max))} fill="none"${attr("stroke", s.c)} stroke-width="1.5"${attr_style(`filter:drop-shadow(0 0 3px ${stringify(s.c)})`)}></path>`);
      } else {
        $$renderer2.push("<!--[-1-->");
        $$renderer2.push(`<text x="80" y="25" text-anchor="middle" fill="rgba(255,255,255,.15)" font-size="9" font-family="monospace">collecting data…</text>`);
      }
      $$renderer2.push(`<!--]--></svg></div>`);
    }
    $$renderer2.push(`<!--]--> <div style="border:1px solid rgba(0,255,255,.06);padding:10px;background:rgba(0,0,0,.5);"><div style="font-size:8px;font-weight:900;letter-spacing:.15em;color:rgba(0,255,255,.3);margin-bottom:6px;">RAW TELEMETRY</div> <pre style="margin:0;font-size:10px;color:#64748b;white-space:pre;">${escape_html(JSON.stringify(
      {
        cpu: `${(0 * 100).toFixed(2)}%`,
        temp: `${systemVitals.temperature.toFixed(2)}°C`,
        ram: `${0 .toFixed(3)} / ${systemVitals.total_ram_gb.toFixed(1)}GB`,
        res: 0 .toFixed(4)
      },
      null,
      2
    ))}</pre></div></div>`);
  });
}
function SecOps($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let scanning = false;
    const OPS = [
      {
        l: "SECURITY AUDIT",
        d: "audit all rust source files for security vulnerabilities unsafe blocks and hardcoded secrets",
        c: "#f87171"
      },
      {
        l: "PATH TRAVERSAL",
        d: "check all file path handling code for directory traversal vulnerabilities",
        c: "#fb923c"
      },
      {
        l: "DEPENDENCY SCAN",
        d: "check Cargo.toml dependencies for known vulnerable versions",
        c: "#fbbf24"
      },
      {
        l: "SECRET SCANNER",
        d: "scan all source files for hardcoded secrets API keys or passwords",
        c: "#a78bfa"
      },
      {
        l: "FFI AUDIT",
        d: "audit all unsafe and extern C and Zig FFI bindings for memory safety issues",
        c: "#22d3ee"
      },
      {
        l: "SANDBOX VERIFY",
        d: "verify the sandbox path validation logic cannot be bypassed",
        c: "#34d399"
      }
    ];
    $$renderer2.push(`<div style="display:flex;flex-direction:column;height:100%;background:#050000;font-family:'JetBrains Mono',monospace;"><div style="flex:none;display:flex;align-items:center;justify-content:space-between;padding:14px 16px;border-bottom:1px solid rgba(248,113,113,.12);background:#0a0000;"><div><div style="font-size:11px;font-weight:900;letter-spacing:.15em;color:#f87171;text-shadow:0 0 12px rgba(248,113,113,.4);">SECOPS_CORTEX</div> <div style="font-size:8px;color:rgba(248,113,113,.35);margin-top:3px;">BIO-DOME SECURITY LAYER // ZIRCON_IPC</div></div> <div${attr_style(`font-size:32px;opacity:${stringify(0.4)};transition:opacity .3s;${stringify("")}`)}>🛡</div></div> <div style="flex:1;overflow-y:auto;padding:14px;display:flex;flex-direction:column;gap:12px;"><div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:6px;"><!--[-->`);
    const each_array = ensure_array_like([
      ["SANDBOX", "ACTIVE", "#34d399"],
      ["ARBITER", "ARMED", "#34d399"],
      ["LAZARUS", "STANDBY", "#fbbf24"],
      ["MEMGUARD", "ACTIVE", "#34d399"],
      ["FFI_GATE", "LOCKED", "#34d399"],
      ["IPC_SHIELD", "NOMINAL", "#34d399"]
    ]);
    for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
      let [l, s, c] = each_array[$$index];
      $$renderer2.push(`<div style="display:flex;align-items:center;gap:6px;padding:8px;border:1px solid rgba(248,113,113,.07);background:rgba(248,113,113,.02);"><span${attr_style(`width:6px;height:6px;border-radius:50%;background:${stringify(c)};box-shadow:0 0 5px ${stringify(c)};flex:none;display:block;`)}></span> <span style="font-size:8px;color:rgba(255,255,255,.3);flex:1;">${escape_html(l)}</span> <span${attr_style(`font-size:8px;font-weight:900;color:${stringify(c)};`)}>${escape_html(s)}</span></div>`);
    }
    $$renderer2.push(`<!--]--></div> <div style="display:flex;flex-direction:column;gap:4px;"><!--[-->`);
    const each_array_1 = ensure_array_like(OPS);
    for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
      let op = each_array_1[$$index_1];
      $$renderer2.push(`<button${attr("disabled", scanning, true)}${attr_style(`display:flex;align-items:center;gap:10px;padding:10px 12px;border:1px solid rgba(255,255,255,.06);background:rgba(248,113,113,.02);cursor:pointer;transition:background .15s;text-align:left;width:100%;${stringify("")}`)}><span${attr_style(`flex:1;font-size:9px;font-weight:900;letter-spacing:.08em;color:${stringify(op.c)};font-family:'JetBrains Mono',monospace;`)}>${escape_html(op.l)}</span> <span style="font-size:14px;color:rgba(255,255,255,.2);">›</span></button>`);
    }
    $$renderer2.push(`<!--]--></div></div></div>`);
  });
}
function AgentLab($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let wasmCells = [];
    let pollTimer;
    onDestroy(() => clearInterval(pollTimer));
    $$renderer2.push(`<div class="h-full bg-black font-mono flex flex-col p-6 gap-6 overflow-hidden"><header class="flex justify-between items-end border-b border-cyan-500/20 pb-4"><div><h1 class="text-cyan-400 text-xl font-black tracking-widest italic">DISTRIBUTED_MESH</h1> <p class="text-cyan-900 text-[9px]">P2P ARM64 WORKLOAD ROUTING</p></div> <div class="text-[10px]">`);
    {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<span class="text-rose-500">○ STANDALONE_MODE</span>`);
    }
    $$renderer2.push(`<!--]--></div></header> <div class="flex-1 flex gap-6 min-h-0"><aside class="w-1/3 space-y-4"><h2 class="text-cyan-700 text-[10px] font-bold tracking-widest border-b border-cyan-900/30 pb-1">TARGET_NODE</h2> `);
    {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--></aside> <main class="flex-1 overflow-y-auto pr-2 space-y-3"><h2 class="text-purple-700 text-[10px] font-bold tracking-widest border-b border-purple-900/30 pb-1">WASM_REGISTRY</h2> `);
    const each_array = ensure_array_like(wasmCells);
    if (each_array.length !== 0) {
      $$renderer2.push("<!--[-->");
      for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
        let cell = each_array[$$index];
        $$renderer2.push(`<div class="bg-[#050505] border border-purple-900/20 p-4 hover:border-purple-500/40 transition-all flex justify-between items-center"><div><div class="text-purple-300 font-bold text-xs">${escape_html(cell.name)}</div> <div class="text-[9px] text-gray-600 mt-1">${escape_html(cell.hit_count)} DISPATCHES</div></div> <button class="px-3 py-1 border border-purple-600 text-purple-400 text-[9px] hover:bg-purple-600 hover:text-black">▶ DISPATCH</button></div>`);
      }
    } else {
      $$renderer2.push("<!--[!-->");
      $$renderer2.push(`<div class="text-center py-10 text-gray-700 text-[10px] italic">Registry empty. Synthesis required.</div>`);
    }
    $$renderer2.push(`<!--]--></main></div></div>`);
  });
}
function CognitiveMesh($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let resFreq = derived(() => (systemVitals.resonance * 1e3).toFixed(0));
    let pulseSpeed = derived(() => 2 - systemVitals.resonance * 1.5);
    $$renderer2.push(`<div class="mesh-v16 svelte-1k3dvvd"><div class="resonance-ring svelte-1k3dvvd"${attr_style(`animation-duration: ${stringify(pulseSpeed())}s`)}><div class="core-data svelte-1k3dvvd"><span class="freq svelte-1k3dvvd">${escape_html(resFreq())}Hz</span> <span class="label svelte-1k3dvvd">NEURAL_RESONANCE</span></div></div> <div class="stats-overlay svelte-1k3dvvd"><div class="stat svelte-1k3dvvd">VMO_DENSITY: ${escape_html(vmoTelemetry.active_vmos)}</div> <div class="stat svelte-1k3dvvd">THERMAL_BIAS: ${escape_html(systemVitals.temperature.toFixed(1))}°C</div></div></div>`);
  });
}
function TrinityMatrix($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let wasmEngine = null;
    let exportNames = [];
    let warnMsg = "";
    let hexGrid = Array(64).fill("00");
    let cpu = derived(() => 0);
    let temp = derived(() => systemVitals.temperature);
    let thermalVal = derived(() => {
      if (!wasmEngine || exportNames.length < 1) return 0;
      try {
        return wasmEngine[exportNames[0]](cpu(), temp());
      } catch {
        return 0;
      }
    });
    let hashVal = derived(() => {
      if (!wasmEngine || exportNames.length < 2) return 0;
      try {
        return wasmEngine[exportNames[1]](cpu(), temp());
      } catch {
        return 0;
      }
    });
    async function handleGenesis(event) {
      const b64 = event.detail;
      try {
        TaskService.log(">> [TRINITY] Intercepting WASM logic core...", "text-purple-400");
        const binaryString = window.atob(b64);
        const bytes = new Uint8Array(binaryString.length);
        for (let i = 0; i < binaryString.length; i++) bytes[i] = binaryString.charCodeAt(i);
        const { instance } = await WebAssembly.instantiate(bytes.buffer);
        const exports$1 = instance.exports;
        const fns = Object.entries(exports$1).filter(([, v]) => typeof v === "function").filter(([k]) => !k.startsWith("__") && !k.startsWith("_")).map(([k]) => k);
        if (fns.length === 0) {
          warnMsg = "No exported functions found in this WASM cell.";
          TaskService.log("!! [TRINITY] No usable exports in cell.", "text-rose-400");
          return;
        }
        wasmEngine = exports$1;
        exportNames = fns;
        warnMsg = fns.length < 2 ? `Only 1 export found: ${fns[0]}` : "";
        TaskService.log(`>> [TRINITY] Bound ${fns.length} export(s): ${fns.slice(0, 3).join(", ")}`, "text-emerald-400");
      } catch (e) {
        warnMsg = `V8 Engine Fault: ${e.message}`;
        TaskService.log(`!! [TRINITY] ${e.message}`, "text-rose-400");
      }
    }
    onDestroy(() => window.removeEventListener("nexus-genesis", handleGenesis));
    $$renderer2.push(`<div class="trinity-root svelte-11znq3m"><div class="header svelte-11znq3m">◈ TRINITY_MATRIX // NATIVE_WASM_ACCELERATED</div> `);
    if (warnMsg) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="warn-banner svelte-11znq3m">⚠ ${escape_html(warnMsg)}</div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
    }
    $$renderer2.push(`<!--]--> `);
    if (wasmEngine && exportNames.length >= 1) {
      $$renderer2.push("<!--[0-->");
      $$renderer2.push(`<div class="metrics svelte-11znq3m"><div class="metric-box svelte-11znq3m"${attr_style(`border-color:${stringify(thermalVal() > 0.8 ? "#f87171" : "#34d399")}`)}><span class="lbl svelte-11znq3m">${escape_html(exportNames[0]?.toUpperCase() ?? "FN_0")}</span> <span class="val svelte-11znq3m"${attr_style(`color:${stringify(thermalVal() > 0.8 ? "#f87171" : "#34d399")}`)}>${escape_html((thermalVal() * 100).toFixed(1))}%</span></div> `);
      if (exportNames.length >= 2) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`<div class="metric-box border-purple-500/50 svelte-11znq3m"><span class="lbl svelte-11znq3m">${escape_html(exportNames[1]?.toUpperCase() ?? "FN_1")}</span> <span class="val text-purple-400 svelte-11znq3m">${escape_html(hashVal().toFixed(4))}</span></div>`);
      } else {
        $$renderer2.push("<!--[-1-->");
      }
      $$renderer2.push(`<!--]--></div> <div class="hex-grid svelte-11znq3m"><!--[-->`);
      const each_array = ensure_array_like(hexGrid);
      for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
        let hex = each_array[$$index];
        $$renderer2.push(`<div class="hex-cell svelte-11znq3m"${attr_style(`opacity:${stringify(parseInt(hex, 16) / 255)}`)}>${escape_html(hex)}</div>`);
      }
      $$renderer2.push(`<!--]--></div>`);
    } else {
      $$renderer2.push("<!--[-1-->");
      $$renderer2.push(`<div class="standby svelte-11znq3m">`);
      if (warnMsg) {
        $$renderer2.push("<!--[0-->");
        $$renderer2.push(`CELL LOADED — INCOMPATIBLE EXPORTS`);
      } else {
        $$renderer2.push("<!--[-1-->");
        $$renderer2.push(`AWAITING KERNEL_GENESIS PAYLOAD...`);
      }
      $$renderer2.push(`<!--]--></div>`);
    }
    $$renderer2.push(`<!--]--></div>`);
  });
}
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let activeNav = "NEXUS_OS";
    let openInput = "";
    let activeDynamicId = "";
    const PANELS = {
      "IDE": IdePanel,
      "WORKSPACE": WorkspaceShell,
      "VITALS": VitalsView,
      "MEMORY": MemoryVault,
      "SECOPS": SecOps,
      "AGENT-LAB": AgentLab,
      "HISTORY": HistoryPanel
    };
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      $$renderer3.push(`<div class="nexus-root">`);
      NavHeader($$renderer3, {
        get activeNav() {
          return activeNav;
        },
        set activeNav($$value) {
          activeNav = $$value;
          $$settled = false;
        }
      });
      $$renderer3.push(`<!----> <div class="body-row">`);
      {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<aside class="sidebar-col"><div class="open-box"><div class="open-label">VIRTUAL_FS_LINK</div> <form class="open-row"><input${attr("value", openInput)} placeholder="src/main.rs" class="open-inp" autocomplete="off"/> <button type="submit" class="open-go">↵</button></form> `);
        {
          $$renderer3.push("<!--[-1-->");
        }
        $$renderer3.push(`<!--]--></div> `);
        Sidebar($$renderer3);
        $$renderer3.push(`<!----></aside>`);
      }
      $$renderer3.push(`<!--]--> <main class="main-col">`);
      if (activeNav === "NEXUS_OS") {
        $$renderer3.push("<!--[0-->");
        $$renderer3.push(`<div class="panel-scroll nexus-os-panel"><div class="os-hero"><div class="os-hero-glyph">◈</div> <div class="os-hero-text"><div class="os-hero-title">NEXUS SINGULARITY</div> <div class="os-hero-sub">ARM64_SUBSTRATE // CROSTINI_VIRTUALIZATION</div></div></div> <div class="os-grid"><div class="os-card"><div class="os-card-hdr">CORE_ARCHITECTURE</div> <div class="os-node-grid"><!--[-->`);
        const each_array = ensure_array_like([
          "AXUM_SRV",
          "GHOST_CRTX",
          "ZIG_MUSCLE",
          "VMO_POOL",
          "LAZARUS",
          "ARBITER"
        ]);
        for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
          let node = each_array[$$index];
          $$renderer3.push(`<div class="os-node"><span class="os-node-dot"></span> <span class="os-node-lbl">${escape_html(node)}</span></div>`);
        }
        $$renderer3.push(`<!--]--></div></div> <div class="os-card"><div class="os-card-hdr">SUBSTRATE_TELEMETRY</div> <div class="vmo-kv"><span>STATUS</span><span style="color:#22d3ee">${escape_html(nexusSocket.telemetry?.status || "IDLE")}</span></div> <div class="vmo-kv"><span>ACTIVE VMOs</span><span style="color:#34d399">${escape_html(nexusSocket.telemetry?.active_vmos || 0)}</span></div> <div class="vmo-kv"><span>RESONANCE</span><span style="color:#fbbf24">${escape_html((nexusSocket.telemetry?.resonance || 0).toFixed(2))}</span></div></div></div> <div class="os-term-wrap"><div class="panel-section-hdr">NEURAL_OUTPUT_STREAM</div> `);
        Terminal($$renderer3);
        $$renderer3.push(`<!----></div></div>`);
      } else if (activeNav === "DYNAMIC_UI") {
        $$renderer3.push("<!--[1-->");
        $$renderer3.push(`<div class="fill-panel flex-row"><aside class="sidebar-col w-[200px] p-[10px] gap-2"><div class="panel-section-hdr">DYNAMIC MODULES</div> <!--[-->`);
        const each_array_1 = ensure_array_like(agentRegistry.modules);
        for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
          let m = each_array_1[$$index_1];
          $$renderer3.push(`<button${attr_class("ws-btn", void 0, { "active": activeDynamicId === m.id })}>◈ ${escape_html(m.label)}</button>`);
        }
        $$renderer3.push(`<!--]--></aside> <main class="flex-1 p-[10px]">`);
        ModuleRenderer($$renderer3, { activeId: activeDynamicId });
        $$renderer3.push(`<!----></main></div>`);
      } else if (activeNav === "COGNITIVE_MESH") {
        $$renderer3.push("<!--[2-->");
        $$renderer3.push(`<div class="fill-panel flex-row gap-[10px] p-[10px]"><div class="flex-1 border border-cyan-500/10">`);
        CognitiveMesh($$renderer3);
        $$renderer3.push(`<!----></div> <div class="flex-1 border border-purple-500/20">`);
        TrinityMatrix($$renderer3);
        $$renderer3.push(`<!----></div></div>`);
      } else if (PANELS[activeNav]) {
        $$renderer3.push("<!--[3-->");
        const ActivePanel = PANELS[activeNav];
        $$renderer3.push(`<div class="fill-panel">`);
        if (ActivePanel) {
          $$renderer3.push("<!--[-->");
          ActivePanel($$renderer3, {});
          $$renderer3.push("<!--]-->");
        } else {
          $$renderer3.push("<!--[!-->");
          $$renderer3.push("<!--]-->");
        }
        $$renderer3.push(`</div>`);
      } else {
        $$renderer3.push("<!--[-1-->");
        $$renderer3.push(`<div class="panel-placeholder">[${escape_html(activeNav)}_OFFLINE]</div>`);
      }
      $$renderer3.push(`<!--]--></main> `);
      GhostBlade($$renderer3);
      $$renderer3.push(`<!----></div> `);
      StatusBar($$renderer3);
      $$renderer3.push(`<!----></div>`);
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
