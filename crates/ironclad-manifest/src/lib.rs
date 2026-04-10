//! Ironclad Manifest — backend-agnostic serialization of a resolved system.
//!
//! The manifest is the intermediate representation between the compiler's
//! resolved AST and the toolchain emitter. It contains only declared system
//! state, stripped of source spans and compiler internals.

pub mod signing;

use serde::{Deserialize, Serialize};
use std::fmt;

// ─── Manifest Root ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub manifest_version: u32,
    pub storage: StorageManifest,
    pub selinux: Option<SelinuxManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorageManifest {
    pub declarations: Vec<StorageDeclManifest>,
}

// ─── Top-Level Storage Declaration ──────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StorageDeclManifest {
    Disk(DiskManifest),
    MdRaid(MdRaidManifest),
    Zpool(ZpoolManifest),
    Stratis(StratisManifest),
    Multipath(MultipathManifest),
    Iscsi(IscsiManifest),
    Nfs(NfsManifest),
    Tmpfs(TmpfsManifest),
}

// ─── Disk ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiskManifest {
    pub device: String,
    pub properties: Vec<PropertyManifest>,
    pub children: Vec<PartitionChildManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PartitionChildManifest {
    Filesystem(Box<FsManifest>),
    Luks(LuksManifest),
    Integrity(IntegrityManifest),
    Lvm(LvmManifest),
    Raw(RawManifest),
    Swap(SwapManifest),
}

// ─── mdraid ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MdRaidManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
    pub children: Vec<PartitionChildManifest>,
}

// ─── ZFS Pool ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZpoolManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
    pub vdevs: Vec<VdevManifest>,
    pub datasets: Vec<DatasetManifest>,
    pub zvols: Vec<ZvolManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VdevManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DatasetManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
    pub children: Vec<DatasetManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZvolManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
    pub children: Vec<ZvolChildManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ZvolChildManifest {
    Swap(SwapManifest),
    Filesystem(Box<FsManifest>),
    Luks(LuksManifest),
}

// ─── Stratis ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StratisManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
    pub filesystems: Vec<StratisFilesystemManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StratisFilesystemManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
    pub mount_block: Option<MountBlockManifest>,
}

// ─── Multipath ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MultipathManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
    pub paths: Vec<PathManifest>,
    pub children: Vec<PartitionChildManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PathManifest {
    pub device: String,
    pub properties: Vec<PropertyManifest>,
}

// ─── iSCSI ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IscsiManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
    pub children: Vec<PartitionChildManifest>,
}

// ─── NFS ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NfsManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
    pub mount_block: Option<MountBlockManifest>,
}

// ─── tmpfs ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TmpfsManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
    pub mount_block: Option<MountBlockManifest>,
}

// ─── Filesystem ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FsManifest {
    pub fs_type: FsTypeManifest,
    pub name: String,
    pub properties: Vec<PropertyManifest>,
    pub subvolumes: Vec<SubvolManifest>,
    pub mount_block: Option<MountBlockManifest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FsTypeManifest {
    Ext4,
    Xfs,
    Btrfs,
    Fat32,
    Ntfs,
}

impl fmt::Display for FsTypeManifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FsTypeManifest::Ext4 => write!(f, "ext4"),
            FsTypeManifest::Xfs => write!(f, "xfs"),
            FsTypeManifest::Btrfs => write!(f, "btrfs"),
            FsTypeManifest::Fat32 => write!(f, "fat32"),
            FsTypeManifest::Ntfs => write!(f, "ntfs"),
        }
    }
}

// ─── Subvolume ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubvolManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
    pub mount_block: Option<MountBlockManifest>,
}

// ─── LUKS ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LuksManifest {
    pub version: LuksVersionManifest,
    pub name: String,
    pub properties: Vec<PropertyManifest>,
    pub children: Vec<LuksChildManifest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LuksVersionManifest {
    Luks1,
    Luks2,
}

impl fmt::Display for LuksVersionManifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LuksVersionManifest::Luks1 => write!(f, "luks1"),
            LuksVersionManifest::Luks2 => write!(f, "luks2"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LuksChildManifest {
    Filesystem(Box<FsManifest>),
    Lvm(LvmManifest),
    Swap(SwapManifest),
}

// ─── Integrity ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntegrityManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
    pub children: Vec<IntegrityChildManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IntegrityChildManifest {
    Filesystem(Box<FsManifest>),
    Lvm(LvmManifest),
    Swap(SwapManifest),
}

// ─── LVM ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LvmManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
    pub children: Vec<LvmChildManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LvmChildManifest {
    Filesystem(Box<FsManifest>),
    Swap(SwapManifest),
    Thin(ThinManifest),
    Vdo(VdoManifest),
}

// ─── Thin Pool ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThinManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
    pub children: Vec<ThinChildManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThinChildManifest {
    Filesystem(Box<FsManifest>),
    Swap(SwapManifest),
}

// ─── VDO ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VdoManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
    pub children: Vec<VdoChildManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VdoChildManifest {
    Filesystem(Box<FsManifest>),
    Swap(SwapManifest),
}

// ─── Swap ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwapManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
}

// ─── Raw ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
}

// ─── Mount ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MountExprManifest {
    pub target: String,
    pub options: Vec<String>,
    pub context: Option<SelinuxContextManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MountBlockManifest {
    pub target: Option<String>,
    pub options: Vec<String>,
    pub automount: Option<bool>,
    pub timeout: Option<i64>,
    pub requires: Vec<String>,
    pub before: Vec<String>,
    pub context: Option<SelinuxContextManifest>,
    pub fscontext: Option<SelinuxContextManifest>,
    pub defcontext: Option<SelinuxContextManifest>,
    pub rootcontext: Option<SelinuxContextManifest>,
}

// ─── SELinux Context ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelinuxContextManifest {
    pub user: String,
    pub role: String,
    pub typ: String,
    pub range: MlsRangeManifest,
    pub raw: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MlsRangeManifest {
    pub low: SensitivityManifest,
    pub high: Option<SensitivityManifest>,
    pub categories: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SensitivityManifest {
    pub level: u32,
}

// ─── SELinux System Block ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelinuxManifest {
    pub properties: Vec<PropertyManifest>,
    pub users: Vec<SelinuxUserManifest>,
    pub roles: Vec<SelinuxRoleManifest>,
    pub booleans: Vec<PropertyManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelinuxUserManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelinuxRoleManifest {
    pub name: String,
    pub properties: Vec<PropertyManifest>,
}

// ─── Properties ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PropertyManifest {
    pub key: String,
    pub value: ValueManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ValueManifest {
    String(String),
    Integer(i64),
    Boolean(bool),
    Size(SizeValueManifest),
    Percentage(u64),
    Remaining,
    Array(Vec<ValueManifest>),
    Path(String),
    DevicePath(String),
    Ident(String),
    Url(String),
    Mount(MountExprManifest),
    SelinuxContext(SelinuxContextManifest),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SizeValueManifest {
    pub amount: u64,
    pub unit: SizeUnitManifest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SizeUnitManifest {
    B,
    K,
    M,
    G,
    T,
}

// ─── CBOR Serialization ────────────────────────────────────

#[derive(Debug)]
pub enum ManifestError {
    SerializationError(String),
    DeserializationError(String),
    SigningError(String),
    VerificationError(String),
    IoError(std::io::Error),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::SerializationError(msg) => write!(f, "serialization error: {msg}"),
            ManifestError::DeserializationError(msg) => write!(f, "deserialization error: {msg}"),
            ManifestError::SigningError(msg) => write!(f, "signing error: {msg}"),
            ManifestError::VerificationError(msg) => write!(f, "verification error: {msg}"),
            ManifestError::IoError(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for ManifestError {}

impl From<std::io::Error> for ManifestError {
    fn from(e: std::io::Error) -> Self {
        ManifestError::IoError(e)
    }
}

pub fn serialize_manifest(manifest: &Manifest) -> Result<Vec<u8>, ManifestError> {
    let mut buf = Vec::new();
    ciborium::into_writer(manifest, &mut buf)
        .map_err(|e| ManifestError::SerializationError(e.to_string()))?;
    Ok(buf)
}

pub fn deserialize_manifest(bytes: &[u8]) -> Result<Manifest, ManifestError> {
    ciborium::from_reader(bytes).map_err(|e| ManifestError::DeserializationError(e.to_string()))
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> Manifest {
        Manifest {
            manifest_version: 1,
            storage: StorageManifest {
                declarations: vec![StorageDeclManifest::Disk(DiskManifest {
                    device: "/dev/sda".to_string(),
                    properties: vec![PropertyManifest {
                        key: "label".to_string(),
                        value: ValueManifest::Ident("gpt".to_string()),
                    }],
                    children: vec![
                        PartitionChildManifest::Filesystem(Box::new(FsManifest {
                            fs_type: FsTypeManifest::Fat32,
                            name: "efi".to_string(),
                            properties: vec![
                                PropertyManifest {
                                    key: "index".to_string(),
                                    value: ValueManifest::Integer(1),
                                },
                                PropertyManifest {
                                    key: "size".to_string(),
                                    value: ValueManifest::Size(SizeValueManifest {
                                        amount: 1,
                                        unit: SizeUnitManifest::G,
                                    }),
                                },
                            ],
                            subvolumes: vec![],
                            mount_block: None,
                        })),
                        PartitionChildManifest::Luks(LuksManifest {
                            version: LuksVersionManifest::Luks2,
                            name: "system".to_string(),
                            properties: vec![],
                            children: vec![LuksChildManifest::Lvm(LvmManifest {
                                name: "vg0".to_string(),
                                properties: vec![],
                                children: vec![
                                    LvmChildManifest::Filesystem(Box::new(FsManifest {
                                        fs_type: FsTypeManifest::Ext4,
                                        name: "root".to_string(),
                                        properties: vec![PropertyManifest {
                                            key: "size".to_string(),
                                            value: ValueManifest::Size(SizeValueManifest {
                                                amount: 50,
                                                unit: SizeUnitManifest::G,
                                            }),
                                        }],
                                        subvolumes: vec![],
                                        mount_block: None,
                                    })),
                                    LvmChildManifest::Swap(SwapManifest {
                                        name: "swap0".to_string(),
                                        properties: vec![PropertyManifest {
                                            key: "size".to_string(),
                                            value: ValueManifest::Size(SizeValueManifest {
                                                amount: 16,
                                                unit: SizeUnitManifest::G,
                                            }),
                                        }],
                                    }),
                                ],
                            })],
                        }),
                    ],
                })],
            },
            selinux: None,
        }
    }

    #[test]
    fn cbor_round_trip() {
        let manifest = sample_manifest();
        let bytes = serialize_manifest(&manifest).expect("serialization should succeed");
        let deserialized = deserialize_manifest(&bytes).expect("deserialization should succeed");
        assert_eq!(manifest, deserialized);
    }

    #[test]
    fn manifest_version_preserved() {
        let manifest = Manifest {
            manifest_version: 42,
            storage: StorageManifest {
                declarations: vec![],
            },
            selinux: None,
        };
        let bytes = serialize_manifest(&manifest).expect("serialization should succeed");
        let deserialized = deserialize_manifest(&bytes).expect("deserialization should succeed");
        assert_eq!(deserialized.manifest_version, 42);
    }

    #[test]
    fn cbor_round_trip_with_selinux() {
        let manifest = Manifest {
            manifest_version: 1,
            storage: StorageManifest {
                declarations: vec![],
            },
            selinux: Some(SelinuxManifest {
                properties: vec![PropertyManifest {
                    key: "mode".to_string(),
                    value: ValueManifest::Ident("enforcing".to_string()),
                }],
                users: vec![SelinuxUserManifest {
                    name: "admin_u".to_string(),
                    properties: vec![PropertyManifest {
                        key: "roles".to_string(),
                        value: ValueManifest::Array(vec![
                            ValueManifest::Ident("staff_r".to_string()),
                            ValueManifest::Ident("sysadm_r".to_string()),
                        ]),
                    }],
                }],
                roles: vec![],
                booleans: vec![PropertyManifest {
                    key: "httpd_can_network_connect".to_string(),
                    value: ValueManifest::Boolean(true),
                }],
            }),
        };
        let bytes = serialize_manifest(&manifest).expect("serialization should succeed");
        let deserialized = deserialize_manifest(&bytes).expect("deserialization should succeed");
        assert_eq!(manifest, deserialized);
    }

    #[test]
    fn cbor_round_trip_complex_storage() {
        let manifest = sample_manifest();
        let bytes = serialize_manifest(&manifest).expect("serialization should succeed");
        assert!(!bytes.is_empty());
        let deserialized = deserialize_manifest(&bytes).expect("deserialization should succeed");

        // Verify structure
        assert_eq!(deserialized.storage.declarations.len(), 1);
        if let StorageDeclManifest::Disk(disk) = &deserialized.storage.declarations[0] {
            assert_eq!(disk.device, "/dev/sda");
            assert_eq!(disk.children.len(), 2);
            if let PartitionChildManifest::Luks(luks) = &disk.children[1] {
                assert_eq!(luks.version, LuksVersionManifest::Luks2);
                assert_eq!(luks.children.len(), 1);
            } else {
                panic!("expected LUKS child");
            }
        } else {
            panic!("expected Disk declaration");
        }
    }
}
