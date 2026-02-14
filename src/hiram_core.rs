use chrono::Utc;

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
        if self.integrity < 100 { self.integrity += 1; }
        println!(">> [KERNEL] Integrity: {}% (Throttled)", self.integrity);
        self.last_verified = Some(now);
    }
}
