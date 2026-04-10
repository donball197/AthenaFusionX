#!/bin/bash
# Project Nexus Singularity - Ghost Purge v1.0
# Moves identified orphans to .cold_storage

ORPHANS=(
    "src/agents.rs" "src/build.rs" "src/calibrate.rs" "src/context.rs"
    "src/devops.rs" "src/filename.rs" "src/heartbeat.rs" "src/hive_mind.rs"
    "src/models.rs" "src/monitor.rs" "src/nano_agent.rs" "src/network.rs"
    "src/polyglot.rs" "src/refuel.rs" "src/safety_valve.rs" "src/sentry.rs"
    "src/stats.rs" "src/watcher.rs" "src/ast_route.rs" "src/frontend_inspector.rs"
    "src/frontend_autofix.rs" "src/backend_autofix.rs" "src/zig_kernel_autofix.rs"
    "src/backend_inspector.rs" "src/zig_kernel_inspector.rs" "src/rebuild.rs"
    "src/auto_healer.rs" "src/janitor.rs" "src/auditor.rs" "src/diagnostics.rs"
    "src/kernel_utils.rs" "src/validator.rs" "src/zenith_check.rs" "src/dynamic_broker.rs"
    "src/utils.rs" "src/pulse_check.rs" "src/demo_2.rs" "src/reflex_bridge.rs"
    "src/verification.rs" "src/command_executor.rs" "src/server_export_handler.rs"
)

mkdir -p .cold_storage

echo -e "\x1b[33m>> [PURGE] Archiving ghosts to .cold_storage...\x1b[0m"

for file in "${ORPHANS[@]}"; do
    if [ -f "$file" ]; then
        mv "$file" .cold_storage/
        echo "   [ARCHIVED] $file"
    fi
done

echo -e "\x1b[32m>> [COMPLETE] Workspace topology high-density mode active.\x1b[0m"
