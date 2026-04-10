//! Ed25519 signing and verification for Ironclad manifests.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::{Manifest, ManifestError, deserialize_manifest};

/// A signed manifest envelope containing the CBOR payload, signature, and public key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedManifest {
    pub version: u32,
    pub public_key: Vec<u8>,
    pub signature: Vec<u8>,
    pub payload: Vec<u8>,
}

/// Sign a CBOR-encoded manifest, generating a new Ed25519 keypair.
pub fn sign_manifest(manifest_cbor: &[u8]) -> Result<SignedManifest, ManifestError> {
    let signing_key = SigningKey::generate(&mut OsRng);
    let signature = signing_key.sign(manifest_cbor);
    let verifying_key = signing_key.verifying_key();

    Ok(SignedManifest {
        version: 1,
        public_key: verifying_key.as_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
        payload: manifest_cbor.to_vec(),
    })
}

/// Sign a CBOR-encoded manifest with a provided signing key.
pub fn sign_manifest_with_key(
    manifest_cbor: &[u8],
    signing_key: &SigningKey,
) -> Result<SignedManifest, ManifestError> {
    let signature = signing_key.sign(manifest_cbor);
    let verifying_key = signing_key.verifying_key();

    Ok(SignedManifest {
        version: 1,
        public_key: verifying_key.as_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
        payload: manifest_cbor.to_vec(),
    })
}

/// Verify a signed manifest's signature and deserialize the payload.
pub fn verify_manifest(signed: &SignedManifest) -> Result<Manifest, ManifestError> {
    let public_key_bytes: [u8; 32] =
        signed.public_key.as_slice().try_into().map_err(|_| {
            ManifestError::VerificationError("invalid public key length".to_string())
        })?;

    let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)
        .map_err(|e| ManifestError::VerificationError(format!("invalid public key: {e}")))?;

    let sig_bytes: [u8; 64] =
        signed.signature.as_slice().try_into().map_err(|_| {
            ManifestError::VerificationError("invalid signature length".to_string())
        })?;

    let signature = Signature::from_bytes(&sig_bytes);

    verifying_key
        .verify(&signed.payload, &signature)
        .map_err(|e| {
            ManifestError::VerificationError(format!("signature verification failed: {e}"))
        })?;

    deserialize_manifest(&signed.payload)
}

/// Write a signed manifest to disk as a CBOR-encoded file.
pub fn write_signed_manifest(signed: &SignedManifest, path: &Path) -> Result<(), ManifestError> {
    let mut buf = Vec::new();
    ciborium::into_writer(signed, &mut buf)
        .map_err(|e| ManifestError::SerializationError(e.to_string()))?;
    std::fs::write(path, &buf)?;
    Ok(())
}

/// Read a signed manifest from a CBOR-encoded file on disk.
pub fn read_signed_manifest(path: &Path) -> Result<SignedManifest, ManifestError> {
    let bytes = std::fs::read(path)?;
    ciborium::from_reader(bytes.as_slice())
        .map_err(|e| ManifestError::DeserializationError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DiskManifest, Manifest, PropertyManifest, StorageDeclManifest, StorageManifest,
        ValueManifest, serialize_manifest,
    };

    fn test_manifest() -> Manifest {
        Manifest {
            manifest_version: 1,
            storage: StorageManifest {
                declarations: vec![StorageDeclManifest::Disk(DiskManifest {
                    device: "/dev/sda".to_string(),
                    properties: vec![PropertyManifest {
                        key: "label".to_string(),
                        value: ValueManifest::Ident("gpt".to_string()),
                    }],
                    children: vec![],
                })],
            },
            selinux: None,
        }
    }

    #[test]
    fn sign_and_verify() {
        let manifest = test_manifest();
        let cbor = serialize_manifest(&manifest).unwrap();
        let signed = sign_manifest(&cbor).unwrap();
        let verified = verify_manifest(&signed).unwrap();
        assert_eq!(manifest, verified);
    }

    #[test]
    fn tampered_payload_rejected() {
        let manifest = test_manifest();
        let cbor = serialize_manifest(&manifest).unwrap();
        let mut signed = sign_manifest(&cbor).unwrap();

        // Tamper with the payload
        if let Some(byte) = signed.payload.last_mut() {
            *byte ^= 0xFF;
        }

        let result = verify_manifest(&signed);
        assert!(result.is_err());
    }

    #[test]
    fn tampered_signature_rejected() {
        let manifest = test_manifest();
        let cbor = serialize_manifest(&manifest).unwrap();
        let mut signed = sign_manifest(&cbor).unwrap();

        // Tamper with the signature
        signed.signature[0] ^= 0xFF;

        let result = verify_manifest(&signed);
        assert!(result.is_err());
    }

    #[test]
    fn wrong_public_key_rejected() {
        let manifest = test_manifest();
        let cbor = serialize_manifest(&manifest).unwrap();
        let mut signed = sign_manifest(&cbor).unwrap();

        // Replace with a different key
        let other_key = SigningKey::generate(&mut OsRng);
        signed.public_key = other_key.verifying_key().as_bytes().to_vec();

        let result = verify_manifest(&signed);
        assert!(result.is_err());
    }

    #[test]
    fn file_round_trip() {
        let manifest = test_manifest();
        let cbor = serialize_manifest(&manifest).unwrap();
        let signed = sign_manifest(&cbor).unwrap();

        let dir = std::env::temp_dir().join("ironclad-test-signing");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ironclad-manifest.signed");

        write_signed_manifest(&signed, &path).unwrap();
        let read_back = read_signed_manifest(&path).unwrap();
        let verified = verify_manifest(&read_back).unwrap();
        assert_eq!(manifest, verified);

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }
}
