use serde::Serialize;
use std::fmt;

pub use ironclad_diagnostics::Span;

/// Root of the storage AST — a collection of top-level declarations
#[derive(Debug, Clone, Serialize)]
pub struct StorageFile {
    pub declarations: Vec<StorageDecl>,
    pub selinux: Option<SelinuxBlock>,
}

/// Top-level storage declaration
#[derive(Debug, Clone, Serialize)]
pub enum StorageDecl {
    Disk(DiskBlock),
    MdRaid(MdRaidBlock),
    Zpool(ZpoolBlock),
    Stratis(StratisBlock),
    Multipath(MultipathBlock),
    Iscsi(IscsiBlock),
    Nfs(NfsBlock),
    Tmpfs(TmpfsBlock),
}

// ─── Disk ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct DiskBlock {
    pub device: String,
    pub properties: Vec<Property>,
    pub children: Vec<PartitionChild>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub enum PartitionChild {
    Filesystem(Box<FsBlock>),
    Luks(LuksBlock),
    Integrity(IntegrityBlock),
    Lvm(LvmBlock),
    Raw(RawBlock),
    Swap(SwapBlock),
}

// ─── mdraid ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct MdRaidBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub children: Vec<PartitionChild>,
    pub span: Span,
}

// ─── ZFS Pool ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ZpoolBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub vdevs: Vec<VdevBlock>,
    pub datasets: Vec<DatasetBlock>,
    pub zvols: Vec<ZvolBlock>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct VdevBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatasetBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub children: Vec<DatasetBlock>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct ZvolBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub children: Vec<ZvolChild>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub enum ZvolChild {
    Swap(SwapBlock),
    Filesystem(Box<FsBlock>),
    Luks(LuksBlock),
}

// ─── Stratis ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct StratisBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub filesystems: Vec<StratisFilesystem>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct StratisFilesystem {
    pub name: String,
    pub properties: Vec<Property>,
    pub mount_block: Option<MountBlockExt>,
    pub span: Span,
}

// ─── Multipath ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct MultipathBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub paths: Vec<PathBlock>,
    pub children: Vec<PartitionChild>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct PathBlock {
    pub device: String,
    pub properties: Vec<Property>,
    pub span: Span,
}

// ─── iSCSI ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct IscsiBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub children: Vec<PartitionChild>,
    pub span: Span,
}

// ─── NFS ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct NfsBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub mount_block: Option<MountBlockExt>,
    pub span: Span,
}

// ─── tmpfs ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct TmpfsBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub mount_block: Option<MountBlockExt>,
    pub span: Span,
}

// ─── Filesystem ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct FsBlock {
    pub fs_type: FsType,
    pub name: String,
    pub properties: Vec<Property>,
    pub subvolumes: Vec<SubvolBlock>,
    pub mount_block: Option<MountBlockExt>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FsType {
    Ext4,
    Xfs,
    Btrfs,
    Fat32,
    Ntfs,
}

impl FsType {
    pub fn supports_xattr(&self) -> bool {
        matches!(self, FsType::Ext4 | FsType::Xfs | FsType::Btrfs)
    }
}

impl fmt::Display for FsType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FsType::Ext4 => write!(f, "ext4"),
            FsType::Xfs => write!(f, "xfs"),
            FsType::Btrfs => write!(f, "btrfs"),
            FsType::Fat32 => write!(f, "fat32"),
            FsType::Ntfs => write!(f, "ntfs"),
        }
    }
}

// ─── Subvolume ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct SubvolBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub mount_block: Option<MountBlockExt>,
    pub span: Span,
}

// ─── LUKS ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct LuksBlock {
    pub version: LuksVersion,
    pub name: String,
    pub properties: Vec<Property>,
    pub children: Vec<LuksChild>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LuksVersion {
    Luks1,
    Luks2,
}

impl fmt::Display for LuksVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LuksVersion::Luks1 => write!(f, "luks1"),
            LuksVersion::Luks2 => write!(f, "luks2"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum LuksChild {
    Filesystem(Box<FsBlock>),
    Lvm(LvmBlock),
    Swap(SwapBlock),
}

// ─── Integrity ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct IntegrityBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub children: Vec<IntegrityChild>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub enum IntegrityChild {
    Filesystem(Box<FsBlock>),
    Lvm(LvmBlock),
    Swap(SwapBlock),
}

// ─── LVM ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct LvmBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub children: Vec<LvmChild>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub enum LvmChild {
    Filesystem(Box<FsBlock>),
    Swap(SwapBlock),
    Thin(ThinBlock),
    Vdo(VdoBlock),
}

// ─── Thin Pool ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ThinBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub children: Vec<ThinChild>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub enum ThinChild {
    Filesystem(Box<FsBlock>),
    Swap(SwapBlock),
}

// ─── VDO ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct VdoBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub children: Vec<VdoChild>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub enum VdoChild {
    Filesystem(Box<FsBlock>),
    Swap(SwapBlock),
}

// ─── Swap ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct SwapBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub span: Span,
}

// ─── Raw ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RawBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub span: Span,
}

// ─── Mount ───────────────────────────────────────────────────

/// Inline mount expression: `mount = /path [opts] context ...`
#[derive(Debug, Clone, Serialize)]
pub struct MountExpr {
    pub target: String,
    pub options: Vec<String>,
    pub context: Option<SelinuxContext>,
}

/// Extended mount block
#[derive(Debug, Clone, Serialize)]
pub struct MountBlockExt {
    pub target: Option<String>,
    pub options: Vec<String>,
    pub automount: Option<bool>,
    pub timeout: Option<i64>,
    pub requires: Vec<String>,
    pub before: Vec<String>,
    pub context: Option<SelinuxContext>,
    pub fscontext: Option<SelinuxContext>,
    pub defcontext: Option<SelinuxContext>,
    pub rootcontext: Option<SelinuxContext>,
    pub span: Span,
}

// ─── SELinux Context ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct SelinuxContext {
    pub user: String,
    pub role: String,
    pub typ: String,
    pub range: MlsRange,
    pub raw: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MlsRange {
    pub low: Sensitivity,
    pub high: Option<Sensitivity>,
    pub categories: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sensitivity {
    pub level: u32,
}

// ─── SELinux System Block ────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct SelinuxBlock {
    pub properties: Vec<Property>,
    pub users: Vec<SelinuxUserDecl>,
    pub roles: Vec<SelinuxRoleDecl>,
    pub booleans: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct SelinuxUserDecl {
    pub name: String,
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct SelinuxRoleDecl {
    pub name: String,
    pub properties: Vec<Property>,
    pub span: Span,
}

// ─── Properties ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct Property {
    pub key: String,
    pub value: Value,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub enum Value {
    String(String),
    Integer(i64),
    Boolean(bool),
    Size(SizeValue),
    Percentage(u64),
    Remaining,
    Array(Vec<Value>),
    Path(String),
    DevicePath(String),
    Ident(String),
    Url(String),
    Mount(MountExpr),
    SelinuxContext(SelinuxContext),
}

#[derive(Debug, Clone, Serialize)]
pub struct SizeValue {
    pub amount: u64,
    pub unit: SizeUnit,
}

impl SizeValue {
    pub fn to_bytes(&self) -> u64 {
        self.amount * self.unit.multiplier()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SizeUnit {
    B,
    K,
    M,
    G,
    T,
}

impl SizeUnit {
    pub fn multiplier(&self) -> u64 {
        match self {
            SizeUnit::B => 1,
            SizeUnit::K => 1024,
            SizeUnit::M => 1024 * 1024,
            SizeUnit::G => 1024 * 1024 * 1024,
            SizeUnit::T => 1024 * 1024 * 1024 * 1024,
        }
    }
}
