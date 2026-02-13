use std::time::Duration;
use std::thread;

pub fn heartbeat() {
    thread::spawn(|| {
        loop {
            // The 528Hz Harmonic Pulse (represented as a 1-second interval)
            print!("."); 
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
            thread::sleep(Duration::from_secs(1));
        }
    });
}
