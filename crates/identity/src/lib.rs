use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

pub struct Identity {
    pub key: SigningKey,
}

impl Identity {
    pub fn new() -> Self {
        let mut csprng = OsRng;
        let key = SigningKey::generate(&mut csprng);
        Self { key }
    }
}

impl Default for Identity {
    fn default() -> Self {
        Self::new()
    }
}
