pub struct MutationRisk {
    pub blast_radius: f32,
    pub confidence: f32,
    pub reversibility: f32,
}

impl MutationRisk {
    pub fn safe(&self) -> bool {
        self.blast_radius < 0.20 && self.confidence > 0.90 && self.reversibility > 0.90
    }
}
