use crate::vision;

pub fn run_calibration() {
    println!("--- [ AGENT 4: CALIBRATION MODE ] ---");
    println!("Establishing room baseline. Please remain still...");
    
    // Establishing a 20-cycle baseline
    for i in 1..=20 {
        let drift = vision::get_motion_delta();
        println!("Cycle {:02}/20 | Raw Drift: {:.4}", i, drift);
        
        // Visual feedback for the pulse
        if drift > 1.0 {
            println!("  >> [!] Motion Detected during cycle {}", i);
        }
    }
    
    println!("--- [ CALIBRATION COMPLETE ] ---");
    println!("Identify your 'Silent' average and set your threshold 0.5 above it.");
}
