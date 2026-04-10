use serde::Serialize;
use std::fmt;

pub use ironclad_diagnostics::Span;

// ─── Source File (Full Language Root) ────────────────────────

/// Root of the full language AST — imports and top-level declarations
#[derive(Debug, Clone, Serialize)]
pub struct SourceFile {
    pub imports: Vec<ImportStmt>,
    pub declarations: Vec<TopLevelDecl>,
}

/// Top-level declaration in an Ironclad source file
#[derive(Debug, Clone, Serialize)]
#[allow(clippy::large_enum_variant)]
pub enum TopLevelDecl {
    Class(ClassDecl),
    System(SystemDecl),
    Var(VarDecl),
    Storage(StorageDecl),
    Selinux(SelinuxBlock),
    Firewall(FirewallBlock),
    Network(NetworkBlock),
    Packages(PackagesBlock),
    Users(UsersBlock),
    Init(InitBlock),
}

// ─── Core Language ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ClassDecl {
    pub name: String,
    pub parent: Option<String>,
    pub body: Vec<ClassBodyItem>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemDecl {
    pub name: String,
    pub parent: Option<String>,
    pub body: Vec<ClassBodyItem>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub enum ClassBodyItem {
    Var(VarDecl),
    Apply(ApplyStmt),
    If(IfBlock),
    For(ForBlock),
    Domain(Box<TopLevelDecl>),
    Property(Property),
}

#[derive(Debug, Clone, Serialize)]
pub struct VarDecl {
    pub name: String,
    pub value: Value,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportStmt {
    pub path: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplyStmt {
    pub class_name: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct IfBlock {
    pub condition: String,
    pub body: Vec<ClassBodyItem>,
    pub elif_branches: Vec<ElifBranch>,
    pub else_body: Option<Vec<ClassBodyItem>>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct ElifBranch {
    pub condition: String,
    pub body: Vec<ClassBodyItem>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForBlock {
    pub var_name: String,
    pub iterable: String,
    pub body: Vec<ClassBodyItem>,
    pub span: Span,
}

// ─── Storage File (Backward Compat) ─────────────────────────

/// Storage-only AST root — backward compatible with Phase 1 prototype
#[allow(dead_code)]
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

impl fmt::Display for MountExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.target)?;
        if !self.options.is_empty() {
            write!(f, " [{}]", self.options.join(", "))?;
        }
        if let Some(ref ctx) = self.context {
            write!(f, " {ctx}")?;
        }
        Ok(())
    }
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

impl fmt::Display for SelinuxContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
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

// ─── Firewall Domain ────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct FirewallBlock {
    pub properties: Vec<Property>,
    pub tables: Vec<FwTableBlock>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct FwTableBlock {
    pub family: String,
    pub name: String,
    pub properties: Vec<Property>,
    pub chains: Vec<FwChainBlock>,
    pub sets: Vec<FwSetBlock>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct FwChainBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub rules: Vec<FwRuleBlock>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct FwRuleBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub matches: Vec<FwMatchBlock>,
    pub log: Option<FwLogBlock>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct FwMatchBlock {
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct FwLogBlock {
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct FwSetBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub span: Span,
}

// ─── Network Domain ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct NetworkBlock {
    pub properties: Vec<Property>,
    pub interfaces: Vec<NetInterfaceBlock>,
    pub bonds: Vec<NetBondBlock>,
    pub bridges: Vec<NetBridgeBlock>,
    pub vlans: Vec<NetVlanBlock>,
    pub dns: Option<NetDnsBlock>,
    pub routes: Option<NetRoutesBlock>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetInterfaceBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub ip: Option<NetIpBlock>,
    pub ip6: Option<NetIp6Block>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetIpBlock {
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetIp6Block {
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetBondBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub ip: Option<NetIpBlock>,
    pub ip6: Option<NetIp6Block>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetBridgeBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub ip: Option<NetIpBlock>,
    pub ip6: Option<NetIp6Block>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetVlanBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub ip: Option<NetIpBlock>,
    pub ip6: Option<NetIp6Block>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetDnsBlock {
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetRoutesBlock {
    pub properties: Vec<Property>,
    pub routes: Vec<NetRouteBlock>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetRouteBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub span: Span,
}

// ─── Packages Domain ────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct PackagesBlock {
    pub properties: Vec<Property>,
    pub repos: Vec<PkgRepoBlock>,
    pub packages: Vec<PkgBlock>,
    pub groups: Vec<PkgGroupBlock>,
    pub modules: Vec<PkgModuleBlock>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct PkgRepoBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct PkgBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct PkgGroupBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct PkgModuleBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub span: Span,
}

// ─── Users Domain ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct UsersBlock {
    pub properties: Vec<Property>,
    pub users: Vec<UserBlock>,
    pub groups: Vec<UserGroupBlock>,
    pub policy: Option<PolicyBlock>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserGroupBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct PolicyBlock {
    pub properties: Vec<Property>,
    pub complexity: Option<ComplexityBlock>,
    pub lockout: Option<LockoutBlock>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComplexityBlock {
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct LockoutBlock {
    pub properties: Vec<Property>,
    pub span: Span,
}

// ─── Init / Services Domain ────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct InitBlock {
    pub backend: String,
    pub properties: Vec<Property>,
    pub services: Vec<ServiceBlock>,
    pub sockets: Vec<SocketBlock>,
    pub timers: Vec<TimerBlock>,
    pub targets: Vec<TargetBlock>,
    pub defaults: Option<DefaultsBlock>,
    pub journal: Option<JournalBlock>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub hardening: Option<HardeningBlock>,
    pub resource_control: Option<ResourceControlBlock>,
    pub logging: Option<LoggingBlock>,
    pub environment: Option<EnvironmentBlock>,
    pub install: Option<InstallBlock>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct SocketBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimerBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct TargetBlock {
    pub name: String,
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct DefaultsBlock {
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct JournalBlock {
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct HardeningBlock {
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceControlBlock {
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoggingBlock {
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentBlock {
    pub properties: Vec<Property>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallBlock {
    pub properties: Vec<Property>,
    pub span: Span,
}

// ─── From impls for generic parsing ─────────────────────────

macro_rules! impl_from_named_props {
    ($t:ty) => {
        impl From<(String, Vec<Property>, Span)> for $t {
            fn from((name, properties, span): (String, Vec<Property>, Span)) -> Self {
                Self {
                    name,
                    properties,
                    span,
                }
            }
        }
    };
}

impl_from_named_props!(PkgRepoBlock);
impl_from_named_props!(PkgBlock);
impl_from_named_props!(PkgModuleBlock);

// ─── Properties ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct Property {
    pub key: String,
    pub value: Value,
    pub span: Span,
}

impl fmt::Display for Property {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} = {}", self.key, self.value)
    }
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

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::String(s) => write!(f, "\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            Value::Integer(n) => write!(f, "{n}"),
            Value::Boolean(b) => write!(f, "{b}"),
            Value::Size(sv) => write!(f, "{sv}"),
            Value::Percentage(p) => write!(f, "{p}%"),
            Value::Remaining => write!(f, "remaining"),
            Value::Array(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Value::Path(p) => write!(f, "{p}"),
            Value::DevicePath(p) => write!(f, "{p}"),
            Value::Ident(s) => write!(f, "{s}"),
            Value::Url(u) => write!(f, "{u}"),
            Value::Mount(m) => write!(f, "{m}"),
            Value::SelinuxContext(c) => write!(f, "{c}"),
        }
    }
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

impl fmt::Display for SizeUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SizeUnit::B => write!(f, "B"),
            SizeUnit::K => write!(f, "K"),
            SizeUnit::M => write!(f, "M"),
            SizeUnit::G => write!(f, "G"),
            SizeUnit::T => write!(f, "T"),
        }
    }
}

impl fmt::Display for SizeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.amount, self.unit)
    }
}
