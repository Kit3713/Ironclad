//! Ironclad Emitter — toolchain generation from a signed manifest.
//!
//! This crate defines the `Emitter` trait and the `ToolchainPlan` that drives
//! the build pipeline. Each build target (ISO, chroot, image, bare, delta)
//! implements the `Emitter` trait to produce target-specific artifacts.

use std::fmt;
use std::path::PathBuf;

use ironclad_manifest::Manifest;
use ironclad_manifest::signing::SignedManifest;

// ─── Build Target ───────────────────────────────────────────

/// The target environment for the build toolchain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildTarget {
    /// Build from a certified minimal ISO (Kickstart/Anaconda).
    Iso,
    /// Build into a chroot directory using dnf --installroot.
    Chroot,
    /// Build an OCI container image via bootc Containerfile.
    Image,
    /// Emit the full toolchain for execution on a bare disk.
    Bare,
    /// Emit a delta toolchain from an old manifest to the current declaration.
    Delta,
}

impl fmt::Display for BuildTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildTarget::Iso => write!(f, "iso"),
            BuildTarget::Chroot => write!(f, "chroot"),
            BuildTarget::Image => write!(f, "image"),
            BuildTarget::Bare => write!(f, "bare"),
            BuildTarget::Delta => write!(f, "delta"),
        }
    }
}

impl std::str::FromStr for BuildTarget {
    type Err = EmitError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "iso" => Ok(BuildTarget::Iso),
            "chroot" => Ok(BuildTarget::Chroot),
            "image" => Ok(BuildTarget::Image),
            "bare" => Ok(BuildTarget::Bare),
            "delta" => Ok(BuildTarget::Delta),
            _ => Err(EmitError::InvalidTarget(s.to_string())),
        }
    }
}

// ─── Toolchain Plan ─────────────────────────────────────────

/// A complete plan for emitting a build toolchain.
pub struct ToolchainPlan {
    pub manifest: Manifest,
    pub signed_manifest: SignedManifest,
    pub target: BuildTarget,
    pub output_dir: PathBuf,
}

// ─── Emitter Trait ──────────────────────────────────────────

/// Trait for toolchain emitters. Each build phase implements this to
/// generate target-specific artifacts from the manifest.
pub trait Emitter {
    type Output;
    fn emit(&self, plan: &ToolchainPlan) -> Result<Self::Output, EmitError>;
}

// ─── Manifest Emitter ───────────────────────────────────────

/// The initial emitter that writes the signed manifest to the output directory.
/// This is the baseline emitter that all build targets include.
pub struct ManifestEmitter;

impl Emitter for ManifestEmitter {
    type Output = PathBuf;

    fn emit(&self, plan: &ToolchainPlan) -> Result<PathBuf, EmitError> {
        std::fs::create_dir_all(&plan.output_dir).map_err(EmitError::IoError)?;

        let manifest_path = plan.output_dir.join("ironclad-manifest.signed");
        ironclad_manifest::signing::write_signed_manifest(&plan.signed_manifest, &manifest_path)
            .map_err(|e| EmitError::ManifestError(e.to_string()))?;

        Ok(manifest_path)
    }
}

// ─── Errors ─────────────────────────────────────────────────

#[derive(Debug)]
pub enum EmitError {
    InvalidTarget(String),
    ManifestError(String),
    IoError(std::io::Error),
}

impl fmt::Display for EmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EmitError::InvalidTarget(t) => {
                write!(
                    f,
                    "invalid build target `{t}` (expected: iso, chroot, image, bare, delta)"
                )
            }
            EmitError::ManifestError(msg) => write!(f, "manifest error: {msg}"),
            EmitError::IoError(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for EmitError {}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ironclad_manifest::signing;
    use ironclad_manifest::*;

    fn test_plan() -> ToolchainPlan {
        let manifest = Manifest {
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
        };

        let cbor = serialize_manifest(&manifest).unwrap();
        let signed = signing::sign_manifest(&cbor).unwrap();

        let output_dir = std::env::temp_dir()
            .join("ironclad-test-emit")
            .join(format!("{}", std::process::id()));

        ToolchainPlan {
            manifest,
            signed_manifest: signed,
            target: BuildTarget::Iso,
            output_dir,
        }
    }

    #[test]
    fn build_target_from_str() {
        assert_eq!("iso".parse::<BuildTarget>().unwrap(), BuildTarget::Iso);
        assert_eq!(
            "chroot".parse::<BuildTarget>().unwrap(),
            BuildTarget::Chroot
        );
        assert_eq!("image".parse::<BuildTarget>().unwrap(), BuildTarget::Image);
        assert_eq!("bare".parse::<BuildTarget>().unwrap(), BuildTarget::Bare);
        assert_eq!("delta".parse::<BuildTarget>().unwrap(), BuildTarget::Delta);
        assert!("invalid".parse::<BuildTarget>().is_err());
    }

    #[test]
    fn build_target_display() {
        assert_eq!(BuildTarget::Iso.to_string(), "iso");
        assert_eq!(BuildTarget::Chroot.to_string(), "chroot");
    }

    #[test]
    fn manifest_emitter_writes_file() {
        let plan = test_plan();
        let emitter = ManifestEmitter;
        let result = emitter.emit(&plan).unwrap();

        assert!(result.exists());
        assert!(result.ends_with("ironclad-manifest.signed"));

        // Verify the written file is valid
        let read_back = signing::read_signed_manifest(&result).unwrap();
        let verified = signing::verify_manifest(&read_back).unwrap();
        assert_eq!(verified, plan.manifest);

        // Cleanup
        let _ = std::fs::remove_dir_all(&plan.output_dir);
    }
}
