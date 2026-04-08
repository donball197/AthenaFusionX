use chrono::Utc;
use std::path::Path;

pub struct Hiram {
    pub integrity: i32,
    pub last_verified: Option<i64>,
    pub verification_interval: i64, 
}

impl Hiram {
    pub fn new(verification_interval: i64) -> Self {
        Self { integrity: 100, last_verified: None, verification_interval }
    }

    pub fn verify(&mut self) {
        let now = Utc::now().timestamp();
        if let Some(lv) = self.last_verified {
            if now - lv < self.verification_interval { return; }
        }

        // Real Integrity Check: Do the vital organs exist?
        let missions_ok = Path::new("missions").exists();
        let claims_ok = Path::new("claims").exists();

        if !missions_ok || !claims_ok {
            self.integrity -= 10;
            println!(">> [KERNEL] WARNING: System folders missing! Integrity: {}%", self.integrity);
        } else if self.integrity < 100 {
            self.integrity += 1;
        }

        println!(">> [KERNEL] Integrity Check: {}%", self.integrity);
        self.last_verified = Some(now);
    }
}
