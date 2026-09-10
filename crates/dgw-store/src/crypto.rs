use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use dgw_engine::creator::Creator;
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use rand::{rngs::OsRng, RngCore};
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop};

const HKDF_SALT: &[u8] = b"dgw-mapping-v1";
const HKDF_INFO_ENC: &[u8] = b"dgw-mapping-v1-enc";
const HKDF_INFO_MAC: &[u8] = b"dgw-mapping-v1-mac";
const NONCE_LEN: usize = 12;
const GCM_TAG_LEN: usize = 16;

type HmacSha256 = Hmac<Sha256>;

#[derive(Zeroize, ZeroizeOnDrop)]
pub(crate) struct DerivedKeys {
    pub enc: [u8; 32],
    pub mac: [u8; 32],
}

#[derive(Debug)]
pub(crate) struct DecryptError;

pub(crate) fn derive_keys(master_key: &[u8; 32]) -> DerivedKeys {
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), master_key);
    let mut enc = [0u8; 32];
    let mut mac = [0u8; 32];
    hk.expand(HKDF_INFO_ENC, &mut enc)
        .expect("HKDF expand enc (32 bytes is always valid)");
    hk.expand(HKDF_INFO_MAC, &mut mac)
        .expect("HKDF expand mac (32 bytes is always valid)");
    DerivedKeys { enc, mac }
}

/// AES-256-GCM with a random 12-byte nonce prefixed: nonce || ciphertext || tag.
pub(crate) fn encrypt(enc: &[u8; 32], plaintext: &[u8]) -> Vec<u8> {
    let cipher = Aes256Gcm::new_from_slice(enc).expect("AES-256-GCM key is 32 bytes");
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .expect("AES-256-GCM encrypt");
    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    out
}

pub(crate) fn decrypt(enc: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>, DecryptError> {
    if ciphertext.len() < NONCE_LEN + GCM_TAG_LEN {
        return Err(DecryptError);
    }
    let (nonce_bytes, rest) = ciphertext.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new_from_slice(enc).map_err(|_| DecryptError)?;
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher.decrypt(nonce, rest).map_err(|_| DecryptError)
}

/// HMAC-SHA256(mac, creator.0 || plaintext bytes).
pub(crate) fn lookup_hmac(mac: &[u8; 32], creator: &Creator, plaintext: &[u8]) -> [u8; 32] {
    let mut h = <HmacSha256 as Mac>::new_from_slice(mac).expect("HMAC-SHA256 accepts 32-byte keys");
    h.update(&creator.0);
    h.update(plaintext);
    let result = h.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dgw_engine::creator::anonymous_creator;

    #[test]
    fn roundtrip_cell() {
        let mk = [7u8; 32];
        let keys = derive_keys(&mk);
        let ct = encrypt(&keys.enc, b"13800138000");
        assert_ne!(ct, b"13800138000");
        assert_eq!(decrypt(&keys.enc, &ct).unwrap(), b"13800138000");
    }

    #[test]
    fn hmac_index_needs_mac_key() {
        let a = derive_keys(&[1u8; 32]);
        let b = derive_keys(&[2u8; 32]);
        let p = b"13800138000";
        assert_ne!(
            lookup_hmac(&a.mac, &anonymous_creator(), p),
            lookup_hmac(&b.mac, &anonymous_creator(), p)
        );
    }
}
