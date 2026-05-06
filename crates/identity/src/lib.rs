use ed25519_dalek::{SigningKey, Signer};
pub struct Identity { pub key: SigningKey }
impl Identity {
    pub fn new() -> Self { Self { key: SigningKey::generate(&mut rand::rngs::OsRng) } }
}
