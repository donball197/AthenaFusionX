use std::hash::Hasher;

pub struct FnvHasher(u64);
impl FnvHasher {
    pub fn new() -> Self { Self(0xcbf29ce484222325) }
}
impl Hasher for FnvHasher {
    fn finish(&self) -> u64 { self.0 }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}
impl Default for FnvHasher { fn default() -> Self { Self::new() } }
