#!/usr/bin/env bash
# =============================================================================
# NEXUS — system_monitor.rs fix for sysinfo 0.31 API
# Error 1: Components::refresh() takes 0 arguments (not bool)
# Error 2: Component::temperature() returns f32 directly (not Option<f32>)
# Run from project root: bash nexus_fix_sysinfo.sh
# =============================================================================
set -euo pipefail

echo "[ FIX ] system_monitor.rs — correcting sysinfo 0.31 API calls..."

cat > bins/ide_backend/src/system_monitor.rs << 'RUST_EOF'
use sysinfo::{Components, System};

pub struct Vitals {
    pub cpu_load:     f32,
    pub ram_gb:       f32,
    pub total_ram_gb: f32,
    pub temperature:  f32,
}

pub struct SystemMonitor {
    sys:        System,
    components: Components,
}

impl SystemMonitor {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self {
            sys,
            components: Components::new_with_refreshed_list(),
        }
    }

    pub fn get_vitals(&mut self) -> Vitals {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();

        // sysinfo 0.31: refresh() takes no arguments.
        self.components.refresh();

        let cpu_load     = self.sys.global_cpu_usage();
        let ram_gb       = self.sys.used_memory()  as f32 / 1_073_741_824.0;
        let total_ram_gb = self.sys.total_memory() as f32 / 1_073_741_824.0;

        // sysinfo 0.31: temperature() returns f32 directly (no Option wrapper).
        // Filter out 0.0 readings (sensors not exposed in Crostini containers).
        let temps: Vec<f32> = self.components
            .iter()
            .map(|c| c.temperature())
            .filter(|&t| t > 0.0)
            .collect();

        let temperature = if temps.is_empty() {
            0.0
        } else {
            temps.iter().sum::<f32>() / temps.len() as f32
        };

        Vitals { cpu_load, ram_gb, total_ram_gb, temperature }
    }
}

impl Default for SystemMonitor {
    fn default() -> Self { Self::new() }
}
RUST_EOF

echo "    ✔ system_monitor.rs — sysinfo 0.31 API corrected"
echo ""
echo "  Changes made:"
echo "    self.components.refresh(true)  →  self.components.refresh()"
echo "    .filter_map(|c| c.temperature())  →  .map(|c| c.temperature()).filter(|&t| t > 0.0)"
echo ""
echo "  Verify with:"
echo "    cargo check -p nexus_ide_backend -j 1"
