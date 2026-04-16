import re

filepath = 'ui/src/routes/+page.svelte'
with open(filepath, 'r') as f:
    content = f.read()

# 1. Inject JS Logic (Targeting the exact string LAZARUS / HISTORY without caring about whitespace)
js_patch = """
// ── CRISIS SIMULATION INJECTOR ─────────────────────────────────────────
async function injectCrisis() {
    pushTermLine('>> [SYSTEM_ALERT] INITIATING HARDWARE CRISIS SIMULATION...', 'text-rose-500');
    try {
        const r = await fetch('/api/fs/read?path=' + encodeURIComponent('data/synthetic/resilience_v1.json'));
        if (!r.ok) throw new Error('Could not load resilience dataset.');
        const data = await r.json();
        const scenarios = JSON.parse(data.content);
        const crisis = scenarios[Math.floor(Math.random() * scenarios.length)];
        
        setTimeout(() => {
            pushTermLine(`!! [ALARM] HAZARD: ${crisis.hazard}`, 'text-rose-500 font-bold bg-rose-500/10');
            pushTermLine(`!! [TELEMETRY_DUMP] ${crisis.telemetry_trigger}`, 'text-rose-400');
        }, 500);

        setTimeout(() => {
            runDirective(`URGENT: ${crisis.hazard} detected. System telemetry reads: ${crisis.telemetry_trigger}. You must execute the appropriate healing action immediately to prevent hardware failure.`);
        }, 1500);
    } catch(e) {
        pushTermLine(`!! [SIM_FAULT] ${e.message}`, 'text-rose-500');
    }
}

// ── LAZARUS / HISTORY"""

if "injectCrisis" not in content:
    content = re.sub(r'// ── LAZARUS / HISTORY.*', js_patch, content)

# 2. Inject HTML Button (Targeting Terminal embedded comment)
html_patch = """
                        <div class="ws-group" style="--gc:#f43f5e; grid-column: 1 / -1;">
                            <div class="ws-group-title">KAGGLE_SIMULATION</div>
                            <button class="ws-btn" style="background: rgba(244,63,94,0.15); border-color: #f43f5e; color: #f43f5e; font-weight: 900; text-align: center; padding: 12px; font-size: 10px;" onclick={injectCrisis}>
                                ⚠ INJECT HARDWARE CRISIS (KAGGLE DEMO)
                            </button>
                        </div>
                    </div>

                    """

if "KAGGLE_SIMULATION" not in content:
    content = re.sub(r'</div>\s*', html_patch, content)

with open(filepath, 'w') as f:
    f.write(content)

print("\x1b[32m>> [PATCH_V2_SUCCESS] Suture applied via Regex\x1b[0m")
