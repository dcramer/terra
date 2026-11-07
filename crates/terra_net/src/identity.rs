//! Who a player is (D592): a key pair whose secret half stays on the player's machine and
//! whose public half is their name to a server, which keeps their character under it. A
//! server proves it by a challenge: it gives each new connection a number never given before,
//! and the hello must carry that number signed. A hello overheard cannot be used again, and
//! nobody can say they are someone whose secret they do not hold.
//!
//! Ed25519 (RFC 8032) by `ed25519-dalek`. This crate rolls no dice: the game gives a client
//! its secret, and a server the secret its challenges are drawn from.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use terra_gen::hash::splitmix64;

/// What a signed hello is told from any other signed thing by.
const DOMAIN: &[u8] = b"terra hello 1";

/// A player's key pair.
#[derive(Clone)]
pub struct Identity {
    key: SigningKey,
}

impl Identity {
    /// The identity of a secret: 32 bytes of the operating system's randomness, made once
    /// and kept on the player's machine.
    pub fn from_secret(secret: [u8; 32]) -> Identity {
        Identity {
            key: SigningKey::from_bytes(&secret),
        }
    }

    /// The public half: what a server knows the player by.
    pub fn public(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }

    /// Signs a hello for one connection's challenge.
    pub fn sign(&self, challenge: &[u8; 32], version: u64, name: &str) -> [u8; 64] {
        self.key
            .sign(&message(challenge, version, &self.public(), name))
            .to_bytes()
    }
}

/// Everything a hello says, so none of it can be changed under the signature.
fn message(challenge: &[u8; 32], version: u64, public: &[u8; 32], name: &str) -> Vec<u8> {
    let mut message = Vec::with_capacity(DOMAIN.len() + 72 + name.len());
    message.extend_from_slice(DOMAIN);
    message.extend_from_slice(challenge);
    message.extend_from_slice(&version.to_le_bytes());
    message.extend_from_slice(public);
    message.extend_from_slice(name.as_bytes());
    message
}

/// Whether a hello was signed, for this challenge, by the holder of the public key's secret.
pub fn proven(
    public: &[u8; 32],
    challenge: &[u8; 32],
    version: u64,
    name: &str,
    signature: &[u8; 64],
) -> bool {
    let Ok(key) = VerifyingKey::from_bytes(public) else {
        return false;
    };
    // Strict: no weak key and no second signature that also passes.
    key.verify_strict(
        &message(challenge, version, public, name),
        &Signature::from_bytes(signature),
    )
    .is_ok()
}

/// A server's challenge for its `count`th connection, from the secret the game gave it:
/// never the same twice, and not to be guessed without the secret.
pub fn challenge(secret: u64, count: u64) -> [u8; 32] {
    let mut challenge = [0; 32];
    for (index, chunk) in challenge.chunks_exact_mut(8).enumerate() {
        let draw = splitmix64(secret ^ splitmix64(count.wrapping_mul(4) + index as u64));
        chunk.copy_from_slice(&draw.to_le_bytes());
    }
    challenge
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_secrets_holder_signs_and_only_for_this_challenge() {
        let (ása, other) = (
            Identity::from_secret([7; 32]),
            Identity::from_secret([8; 32]),
        );
        let (first, second) = (challenge(99, 1), challenge(99, 2));
        assert_ne!(first, second);
        assert_ne!(first, challenge(100, 1));
        let signed = ása.sign(&first, 3, "Ása");
        assert!(proven(&ása.public(), &first, 3, "Ása", &signed));
        // Another's key, another challenge (a replay), another version, another name.
        assert!(!proven(&other.public(), &first, 3, "Ása", &signed));
        assert!(!proven(&ása.public(), &second, 3, "Ása", &signed));
        assert!(!proven(&ása.public(), &first, 4, "Ása", &signed));
        assert!(!proven(&ása.public(), &first, 3, "Asa", &signed));
        // Bytes that are no key and no signature.
        assert!(!proven(&[0xFF; 32], &first, 3, "Ása", &signed));
        assert!(!proven(&ása.public(), &first, 3, "Ása", &[0; 64]));
    }
}
