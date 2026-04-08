use std::process::Command;
use crate::nexus_error::NexusError;

/// ZigAutofix: The interface for low-level ARM64 optimizations.
pub struct ZigAutofix;

impl ZigAutofix {
    /// align_memory_strands: Calls the Zig-compiled binary to purge fragmentation.
    pub fn align_memory_strands() -> Result<String, NexusError> {
        println!("\x1b[33m>> [ZIG] Initiating Hardware-Level Realignment...\x1b[0m");

        // Attempt to run the Zig realignment tool
        let output = Command::new("zig")
            .args(["run", "src/kernel/realignment.zig", "--optimize", "ReleaseFast"])
            .output()
            .map_err(|e| NexusError::Other(format!("Zig binary fault: {}", e)))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            Err(NexusError::Other(format!("Zig Realignment Failed: {}", err)))
        }
    }
}
