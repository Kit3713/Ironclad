use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::errors::{Diagnostic, IroncladError, Result, Severity};

/// Backward-compatible validation for StorageFile (used by existing tests)
#[allow(dead_code)]
pub fn validate_storage(file: &StorageFile) -> Result<Vec<Diagnostic>> {
    // Convert to SourceFile for validation
    let mut declarations = Vec::new();
    for decl in &file.declarations {
        declarations.push(TopLevelDecl::Storage(decl.clone()));
    }
    if let Some(ref se) = file.selinux {
        declarations.push(TopLevelDecl::Selinux(se.clone()));
    }
    let source = SourceFile {
        imports: vec![],
        declarations,
    };
    validate(&source)
}

/// Run all structural validation passes on a parsed source file.
/// Validates storage, SELinux, users, and packages declarations.
/// Remaining domain validations (firewall, network, init) will be added in Phase 3.
pub fn validate(file: &SourceFile) -> Result<Vec<Diagnostic>> {
    let mut ctx = ValidationContext::default();

    for decl in &file.declarations {
        match decl {
            TopLevelDecl::Storage(storage) => match storage {
                StorageDecl::Disk(disk) => validate_disk(&mut ctx, disk),
                StorageDecl::MdRaid(md) => validate_mdraid(&mut ctx, md),
                StorageDecl::Zpool(zp) => validate_zpool(&mut ctx, zp),
                StorageDecl::Stratis(s) => validate_stratis(&mut ctx, s),
                StorageDecl::Multipath(mp) => validate_multipath(&mut ctx, mp),
                StorageDecl::Iscsi(iscsi) => validate_iscsi(&mut ctx, iscsi),
                StorageDecl::Nfs(nfs) => validate_nfs(&mut ctx, nfs),
                StorageDecl::Tmpfs(tmpfs) => validate_tmpfs(&mut ctx, tmpfs),
            },
            TopLevelDecl::Selinux(se) => validate_selinux(&mut ctx, se),
            TopLevelDecl::Users(u) => validate_users(&mut ctx, u),
            TopLevelDecl::Packages(p) => validate_packages(&mut ctx, p),
            // Remaining domains: validation deferred to Phase 3
            TopLevelDecl::Firewall(_)
            | TopLevelDecl::Network(_)
            | TopLevelDecl::Init(_)
            | TopLevelDecl::Class(_)
            | TopLevelDecl::System(_)
            | TopLevelDecl::Var(_) => {}
        }
    }

    if ctx.errors.is_empty() {
        Ok(ctx.warnings)
    } else {
        Err(IroncladError::ValidationError { errors: ctx.errors })
    }
}

#[derive(Default)]
struct ValidationContext {
    errors: Vec<Diagnostic>,
    warnings: Vec<Diagnostic>,
    mount_targets: HashMap<String, Span>,
    mdraid_members: HashSet<String>,
    block_names: HashMap<String, Span>,
    multipath_wwids: HashSet<String>,
    user_names: HashMap<String, Span>,
    user_uids: HashMap<i64, Span>,
    group_names: HashMap<String, Span>,
    group_gids: HashMap<i64, Span>,
    pkg_names: HashSet<String>,
    repo_names: HashSet<String>,
}

impl ValidationContext {
    fn error(
        &mut self,
        message: String,
        span: Option<Span>,
        block_name: Option<String>,
        hint: Option<String>,
    ) {
        self.errors.push(Diagnostic {
            severity: Severity::Error,
            message,
            span,
            block_name,
            hint,
        });
    }

    fn warning(
        &mut self,
        message: String,
        span: Option<Span>,
        block_name: Option<String>,
        hint: Option<String>,
    ) {
        self.warnings.push(Diagnostic {
            severity: Severity::Warning,
            message,
            span,
            block_name,
            hint,
        });
    }

    fn check_mount_unique(&mut self, target: &str, span: &Span, block_name: &str) {
        if let Some(prev) = self.mount_targets.get(target) {
            self.error(
                format!(
                    "duplicate mount target `{target}` (previously declared at line {})",
                    prev.line
                ),
                Some(span.clone()),
                Some(block_name.to_string()),
                Some(
                    "every mount target path must be unique across the entire source tree"
                        .to_string(),
                ),
            );
        } else {
            self.mount_targets.insert(target.to_string(), span.clone());
        }
    }

    fn check_block_name(&mut self, name: &str, span: &Span) {
        if RESERVED_KEYWORDS.contains(&name) {
            self.error(
                format!("`{name}` is a reserved keyword and cannot be used as a block name"),
                Some(span.clone()),
                None,
                None,
            );
        }
        if let Some(prev) = self.block_names.get(name) {
            self.warning(
                format!("block name `{name}` already used at line {}", prev.line),
                Some(span.clone()),
                None,
                Some("block names should be unique for clear reference".to_string()),
            );
        } else {
            self.block_names.insert(name.to_string(), span.clone());
        }
    }
}

const RESERVED_KEYWORDS: &[&str] = &[
    "disk",
    "mdraid",
    "zpool",
    "vdev",
    "dataset",
    "zvol",
    "stratis",
    "filesystem",
    "multipath",
    "path",
    "iscsi",
    "nfs",
    "luks2",
    "luks1",
    "integrity",
    "lvm",
    "thin",
    "vdo",
    "ext4",
    "xfs",
    "btrfs",
    "fat32",
    "swap",
    "ntfs",
    "tmpfs",
    "raw",
    "subvol",
    "mount",
    "remaining",
    "none",
    "whole",
    "true",
    "false",
    "context",
    "fscontext",
    "defcontext",
    "rootcontext",
    "selinux",
];

// ─── Disk Validation ─────────────────────────────────────────

fn validate_disk(ctx: &mut ValidationContext, disk: &DiskBlock) {
    let label = get_property_str(&disk.properties, "label");

    if label.is_none() {
        ctx.error(
            "disk block requires a `label` property (gpt, msdos, or none)".to_string(),
            Some(disk.span.clone()),
            Some(disk.device.clone()),
            None,
        );
    }

    let is_whole_disk = label == Some("none");

    if is_whole_disk {
        if disk.children.len() != 1 {
            ctx.error(
                format!(
                    "disk with `label = none` must have exactly one child, found {}",
                    disk.children.len()
                ),
                Some(disk.span.clone()),
                Some(disk.device.clone()),
                Some("whole-disk devices use a single filesystem or luks block".to_string()),
            );
        }
        for child in &disk.children {
            check_no_partition_props(ctx, child, &disk.device);
        }
    } else {
        validate_partition_children(ctx, &disk.children, &disk.device);
    }

    for child in &disk.children {
        validate_partition_child(ctx, child);
    }
}

fn check_no_partition_props(ctx: &mut ValidationContext, child: &PartitionChild, disk_name: &str) {
    let props = match child {
        PartitionChild::Filesystem(f) => &f.properties,
        PartitionChild::Luks(l) => &l.properties,
        PartitionChild::Integrity(i) => &i.properties,
        PartitionChild::Lvm(l) => &l.properties,
        PartitionChild::Raw(r) => &r.properties,
        PartitionChild::Swap(s) => &s.properties,
    };

    for prop in props {
        if ["index", "size", "start", "end", "type"].contains(&prop.key.as_str()) {
            ctx.error(
                format!(
                    "property `{}` is not valid inside a whole-disk device (label = none)",
                    prop.key
                ),
                Some(prop.span.clone()),
                Some(disk_name.to_string()),
                Some("the block device is the filesystem's backing device directly".to_string()),
            );
        }
    }
}

fn validate_partition_children(
    ctx: &mut ValidationContext,
    children: &[PartitionChild],
    parent_name: &str,
) {
    let mut indices: HashMap<i64, Span> = HashMap::new();
    let mut remaining_count = 0;

    for child in children {
        let (props, span) = match child {
            PartitionChild::Filesystem(f) => (&f.properties, &f.span),
            PartitionChild::Luks(l) => (&l.properties, &l.span),
            PartitionChild::Integrity(i) => (&i.properties, &i.span),
            PartitionChild::Lvm(l) => (&l.properties, &l.span),
            PartitionChild::Raw(r) => (&r.properties, &r.span),
            PartitionChild::Swap(s) => (&s.properties, &s.span),
        };

        if let Some(idx) = get_property_int(props, "index") {
            if idx <= 0 {
                ctx.error(
                    format!("partition index must be positive, got {idx}"),
                    Some(span.clone()),
                    Some(parent_name.to_string()),
                    None,
                );
            }
            if let Some(prev) = indices.get(&idx) {
                ctx.error(
                    format!(
                        "duplicate partition index {idx} (previously at line {})",
                        prev.line
                    ),
                    Some(span.clone()),
                    Some(parent_name.to_string()),
                    None,
                );
            } else {
                indices.insert(idx, span.clone());
            }
        }

        if get_property_remaining(props) {
            remaining_count += 1;
        }
    }

    if remaining_count > 1 {
        ctx.error(
            format!(
                "only one `remaining` size is permitted per parent scope, found {remaining_count}"
            ),
            None,
            Some(parent_name.to_string()),
            None,
        );
    }
}

// ─── mdraid Validation ───────────────────────────────────────

fn validate_mdraid(ctx: &mut ValidationContext, md: &MdRaidBlock) {
    ctx.check_block_name(&md.name, &md.span);

    if get_property_str(&md.properties, "level").is_none()
        && get_property_int(&md.properties, "level").is_none()
    {
        ctx.error(
            "mdraid block requires a `level` property".to_string(),
            Some(md.span.clone()),
            Some(md.name.clone()),
            Some("valid levels: 0, 1, 5, 6, 10".to_string()),
        );
    }

    if !md.properties.iter().any(|p| p.key == "disks") {
        ctx.error(
            "mdraid block requires a `disks` property".to_string(),
            Some(md.span.clone()),
            Some(md.name.clone()),
            None,
        );
    }

    for prop in &md.properties {
        if prop.key == "disks"
            && let Value::Array(items) = &prop.value
        {
            for item in items {
                if let Value::DevicePath(path) = item
                    && !ctx.mdraid_members.insert(path.clone())
                {
                    ctx.error(
                        format!("device `{path}` is already a member of another mdraid array"),
                        Some(prop.span.clone()),
                        Some(md.name.clone()),
                        None,
                    );
                }
            }
        }
    }

    for child in &md.children {
        validate_partition_child(ctx, child);
    }
}

// ─── ZFS Pool Validation ─────────────────────────────────────

fn validate_zpool(ctx: &mut ValidationContext, zp: &ZpoolBlock) {
    ctx.check_block_name(&zp.name, &zp.span);

    // Validate vdev member uniqueness within pool
    let mut vdev_members: HashSet<String> = HashSet::new();
    for vdev in &zp.vdevs {
        ctx.check_block_name(&vdev.name, &vdev.span);

        let vdev_type = get_property_str(&vdev.properties, "type");
        if vdev_type.is_none() {
            ctx.error(
                "vdev block requires a `type` property".to_string(),
                Some(vdev.span.clone()),
                Some(vdev.name.clone()),
                Some(
                    "valid types: mirror, raidz1, raidz2, raidz3, stripe, spare, log, cache"
                        .to_string(),
                ),
            );
        }

        if !vdev.properties.iter().any(|p| p.key == "members") {
            ctx.error(
                "vdev block requires a `members` property".to_string(),
                Some(vdev.span.clone()),
                Some(vdev.name.clone()),
                None,
            );
        }

        // Check member count minimums and uniqueness
        for prop in &vdev.properties {
            if prop.key == "members"
                && let Value::Array(items) = &prop.value
            {
                let member_count = items.len();
                if let Some(vt) = vdev_type {
                    let min = match vt {
                        "mirror" => 2,
                        "raidz1" => 3,
                        "raidz2" => 4,
                        "raidz3" => 5,
                        _ => 1,
                    };
                    if member_count < min {
                        ctx.error(
                            format!("vdev type `{vt}` requires at least {min} members, got {member_count}"),
                            Some(vdev.span.clone()),
                            Some(vdev.name.clone()),
                            None,
                        );
                    }
                }

                for item in items {
                    if let Value::DevicePath(path) = item
                        && !vdev_members.insert(path.clone())
                    {
                        ctx.error(
                            format!(
                                "device `{path}` appears in multiple vdevs within pool `{}`",
                                zp.name
                            ),
                            Some(prop.span.clone()),
                            Some(vdev.name.clone()),
                            None,
                        );
                    }
                }
            }
        }
    }

    for ds in &zp.datasets {
        validate_dataset(ctx, ds);
    }

    for zvol in &zp.zvols {
        ctx.check_block_name(&zvol.name, &zvol.span);
        if !zvol.properties.iter().any(|p| p.key == "size") {
            ctx.error(
                "zvol requires a `size` property".to_string(),
                Some(zvol.span.clone()),
                Some(zvol.name.clone()),
                None,
            );
        }
    }
}

fn validate_dataset(ctx: &mut ValidationContext, ds: &DatasetBlock) {
    ctx.check_block_name(&ds.name, &ds.span);

    // Check mountpoint uniqueness
    if let Some(mp) = get_property_str(&ds.properties, "mountpoint")
        && mp != "none"
    {
        ctx.check_mount_unique(mp, &ds.span, &ds.name);
    }

    for child in &ds.children {
        validate_dataset(ctx, child);
    }
}

// ─── Stratis Validation ──────────────────────────────────────

fn validate_stratis(ctx: &mut ValidationContext, s: &StratisBlock) {
    ctx.check_block_name(&s.name, &s.span);

    if !s.properties.iter().any(|p| p.key == "disks") {
        ctx.error(
            "stratis pool requires a `disks` property".to_string(),
            Some(s.span.clone()),
            Some(s.name.clone()),
            None,
        );
    }

    for fs in &s.filesystems {
        ctx.check_block_name(&fs.name, &fs.span);
        if let Some(mp) = get_property_str(&fs.properties, "mountpoint") {
            ctx.check_mount_unique(mp, &fs.span, &fs.name);
        }
        if let Some(ref mb) = fs.mount_block
            && let Some(ref target) = mb.target
        {
            ctx.check_mount_unique(target, &mb.span, &fs.name);
        }
    }
}

// ─── Multipath Validation ────────────────────────────────────

fn validate_multipath(ctx: &mut ValidationContext, mp: &MultipathBlock) {
    ctx.check_block_name(&mp.name, &mp.span);

    // wwid required and unique
    let wwid = get_property_str(&mp.properties, "wwid");
    if wwid.is_none() {
        ctx.error(
            "multipath block requires a `wwid` property".to_string(),
            Some(mp.span.clone()),
            Some(mp.name.clone()),
            None,
        );
    } else if let Some(w) = wwid
        && !ctx.multipath_wwids.insert(w.to_string())
    {
        ctx.error(
            format!("duplicate multipath wwid `{w}`"),
            Some(mp.span.clone()),
            Some(mp.name.clone()),
            None,
        );
    }

    for child in &mp.children {
        validate_partition_child(ctx, child);
    }
}

// ─── iSCSI Validation ────────────────────────────────────────

fn validate_iscsi(ctx: &mut ValidationContext, iscsi: &IscsiBlock) {
    ctx.check_block_name(&iscsi.name, &iscsi.span);

    if get_property_str(&iscsi.properties, "target").is_none() {
        ctx.error(
            "iscsi block requires a `target` property".to_string(),
            Some(iscsi.span.clone()),
            Some(iscsi.name.clone()),
            None,
        );
    }

    if get_property_str(&iscsi.properties, "portal").is_none() {
        ctx.error(
            "iscsi block requires a `portal` property".to_string(),
            Some(iscsi.span.clone()),
            Some(iscsi.name.clone()),
            None,
        );
    }

    for child in &iscsi.children {
        validate_partition_child(ctx, child);
    }
}

// ─── NFS Validation ──────────────────────────────────────────

fn validate_nfs(ctx: &mut ValidationContext, nfs: &NfsBlock) {
    ctx.check_block_name(&nfs.name, &nfs.span);

    if get_property_str(&nfs.properties, "server").is_none() {
        ctx.error(
            "nfs block requires a `server` property".to_string(),
            Some(nfs.span.clone()),
            Some(nfs.name.clone()),
            None,
        );
    }

    if get_property_str(&nfs.properties, "export").is_none() {
        ctx.error(
            "nfs block requires an `export` property".to_string(),
            Some(nfs.span.clone()),
            Some(nfs.name.clone()),
            None,
        );
    }

    if let Some(ref mb) = nfs.mount_block
        && let Some(ref target) = mb.target
    {
        ctx.check_mount_unique(target, &mb.span, &nfs.name);
    }

    // Check inline mount
    for prop in &nfs.properties {
        if prop.key == "mount"
            && let Value::Mount(ref mount) = prop.value
        {
            ctx.check_mount_unique(&mount.target, &nfs.span, &nfs.name);
        }
    }
}

// ─── tmpfs Validation ────────────────────────────────────────

fn validate_tmpfs(ctx: &mut ValidationContext, tmpfs: &TmpfsBlock) {
    ctx.check_block_name(&tmpfs.name, &tmpfs.span);

    // tmpfs must declare a mount
    let has_inline_mount = tmpfs.properties.iter().any(|p| p.key == "mount");
    let has_ext_mount = tmpfs.mount_block.is_some();
    if !has_inline_mount && !has_ext_mount {
        ctx.error(
            "tmpfs block must declare a `mount`".to_string(),
            Some(tmpfs.span.clone()),
            Some(tmpfs.name.clone()),
            None,
        );
    }

    for prop in &tmpfs.properties {
        if prop.key == "mount"
            && let Value::Mount(ref mount) = prop.value
        {
            ctx.check_mount_unique(&mount.target, &tmpfs.span, &tmpfs.name);
        }
    }

    if let Some(ref mb) = tmpfs.mount_block
        && let Some(ref target) = mb.target
    {
        ctx.check_mount_unique(target, &mb.span, &tmpfs.name);
    }
}

// ─── SELinux Block Validation ────────────────────────────────

fn validate_selinux(ctx: &mut ValidationContext, se: &SelinuxBlock) {
    // Check mode validity
    if let Some(mode) = get_property_str(&se.properties, "mode")
        && !["enforcing", "permissive", "disabled"].contains(&mode)
    {
        ctx.error(
            format!("invalid SELinux mode `{mode}`"),
            Some(se.span.clone()),
            None,
            Some("valid modes: enforcing, permissive, disabled".to_string()),
        );
    }

    // Check policy validity
    if let Some(policy) = get_property_str(&se.properties, "policy")
        && !["targeted", "mls", "minimum"].contains(&policy)
    {
        ctx.error(
            format!("invalid SELinux policy `{policy}`"),
            Some(se.span.clone()),
            None,
            Some("valid policies: targeted, mls, minimum".to_string()),
        );
    }

    // Check floor validity
    if let Some(floor) = get_property_str(&se.properties, "floor")
        && !["baseline", "standard", "strict", "maximum"].contains(&floor)
    {
        ctx.error(
            format!("invalid security floor `{floor}`"),
            Some(se.span.clone()),
            None,
            Some("valid floors: baseline, standard, strict, maximum".to_string()),
        );
    }

    // At most one default user
    let default_count = se
        .users
        .iter()
        .filter(|u| {
            u.properties
                .iter()
                .any(|p| p.key == "default" && matches!(&p.value, Value::Boolean(true)))
        })
        .count();
    if default_count > 1 {
        ctx.error(
            format!("at most one SELinux user may have `default = true`, found {default_count}"),
            Some(se.span.clone()),
            None,
            None,
        );
    }

    // system_u must be declared
    if !se.users.is_empty() && !se.users.iter().any(|u| u.name == "system_u") {
        ctx.error(
            "`system_u` must be declared as an SELinux user".to_string(),
            Some(se.span.clone()),
            None,
            Some("system_u is required by all SELinux policies".to_string()),
        );
    }

    // Validate user declarations
    for user in &se.users {
        if !user.properties.iter().any(|p| p.key == "roles") {
            ctx.error(
                format!("SELinux user `{}` requires a `roles` property", user.name),
                Some(user.span.clone()),
                Some(user.name.clone()),
                None,
            );
        }
    }
}

// ─── Recursive Child Validation ──────────────────────────────

fn validate_partition_child(ctx: &mut ValidationContext, child: &PartitionChild) {
    match child {
        PartitionChild::Filesystem(f) => validate_fs(ctx, f),
        PartitionChild::Luks(l) => validate_luks(ctx, l),
        PartitionChild::Integrity(i) => validate_integrity(ctx, i),
        PartitionChild::Lvm(l) => validate_lvm(ctx, l),
        PartitionChild::Raw(r) => {
            ctx.check_block_name(&r.name, &r.span);
        }
        PartitionChild::Swap(s) => {
            ctx.check_block_name(&s.name, &s.span);
        }
    }
}

fn validate_fs(ctx: &mut ValidationContext, fs: &FsBlock) {
    ctx.check_block_name(&fs.name, &fs.span);

    for prop in &fs.properties {
        if prop.key == "mount"
            && let Value::Mount(ref mount) = prop.value
        {
            ctx.check_mount_unique(&mount.target, &fs.span, &fs.name);
            validate_mount_selinux_inline(ctx, mount, fs.fs_type, &fs.name, &fs.span);
        }
    }

    if let Some(ref mb) = fs.mount_block {
        if let Some(ref target) = mb.target {
            ctx.check_mount_unique(target, &mb.span, &fs.name);
        }
        validate_mount_selinux_ext(ctx, mb, fs.fs_type, &fs.name);
    }

    if !fs.subvolumes.is_empty() && fs.fs_type != FsType::Btrfs {
        ctx.error(
            format!(
                "subvolume blocks are only valid inside btrfs, not {}",
                fs.fs_type
            ),
            Some(fs.span.clone()),
            Some(fs.name.clone()),
            None,
        );
    }

    for sv in &fs.subvolumes {
        validate_subvol(ctx, sv, fs.fs_type);
    }
}

fn validate_subvol(ctx: &mut ValidationContext, sv: &SubvolBlock, parent_fs: FsType) {
    for prop in &sv.properties {
        if prop.key == "mount"
            && let Value::Mount(ref mount) = prop.value
        {
            ctx.check_mount_unique(&mount.target, &sv.span, &sv.name);
        }
    }

    if let Some(ref mb) = sv.mount_block {
        if let Some(ref target) = mb.target {
            ctx.check_mount_unique(target, &mb.span, &sv.name);
        }
        validate_mount_selinux_ext(ctx, mb, parent_fs, &sv.name);
    }
}

fn validate_luks(ctx: &mut ValidationContext, luks: &LuksBlock) {
    ctx.check_block_name(&luks.name, &luks.span);

    let fs_count = luks
        .children
        .iter()
        .filter(|c| matches!(c, LuksChild::Filesystem(_)))
        .count();
    let lvm_count = luks
        .children
        .iter()
        .filter(|c| matches!(c, LuksChild::Lvm(_)))
        .count();

    if lvm_count == 0 && fs_count > 1 {
        ctx.error(
            format!(
                "luks block without lvm may contain at most one filesystem child, found {fs_count}"
            ),
            Some(luks.span.clone()),
            Some(luks.name.clone()),
            Some("LUKS opens to a single block device".to_string()),
        );
    }

    for child in &luks.children {
        match child {
            LuksChild::Filesystem(f) => validate_fs(ctx, f),
            LuksChild::Lvm(l) => validate_lvm(ctx, l),
            LuksChild::Swap(s) => {
                ctx.check_block_name(&s.name, &s.span);
            }
        }
    }
}

fn validate_integrity(ctx: &mut ValidationContext, integ: &IntegrityBlock) {
    ctx.check_block_name(&integ.name, &integ.span);

    for child in &integ.children {
        match child {
            IntegrityChild::Filesystem(f) => validate_fs(ctx, f),
            IntegrityChild::Lvm(l) => validate_lvm(ctx, l),
            IntegrityChild::Swap(s) => {
                ctx.check_block_name(&s.name, &s.span);
            }
        }
    }
}

fn validate_lvm(ctx: &mut ValidationContext, lvm: &LvmBlock) {
    ctx.check_block_name(&lvm.name, &lvm.span);

    let mut remaining_count = 0;

    for child in &lvm.children {
        match child {
            LvmChild::Filesystem(f) => {
                validate_fs(ctx, f);
                if get_property_remaining(&f.properties) {
                    remaining_count += 1;
                }
            }
            LvmChild::Swap(s) => {
                ctx.check_block_name(&s.name, &s.span);
                if get_property_remaining(&s.properties) {
                    remaining_count += 1;
                }
            }
            LvmChild::Thin(t) => {
                validate_thin(ctx, t, &lvm.name);
                if get_property_remaining(&t.properties) {
                    remaining_count += 1;
                }
            }
            LvmChild::Vdo(v) => {
                validate_vdo(ctx, v, &lvm.name);
                if get_property_remaining(&v.properties) {
                    remaining_count += 1;
                }
            }
        }
    }

    if remaining_count > 1 {
        ctx.error(
            format!(
                "only one `remaining` size is permitted per lvm scope, found {remaining_count}"
            ),
            Some(lvm.span.clone()),
            Some(lvm.name.clone()),
            None,
        );
    }
}

fn validate_thin(ctx: &mut ValidationContext, thin: &ThinBlock, vg_name: &str) {
    ctx.check_block_name(&thin.name, &thin.span);

    if !thin.properties.iter().any(|p| p.key == "size") {
        ctx.error(
            "thin pool requires a `size` property".to_string(),
            Some(thin.span.clone()),
            Some(thin.name.clone()),
            None,
        );
    }

    let pool_size = get_property_size_bytes(&thin.properties, "size");
    if let Some(pool_bytes) = pool_size {
        let total_virtual: u64 = thin
            .children
            .iter()
            .filter_map(|c| match c {
                ThinChild::Filesystem(f) => get_property_size_bytes(&f.properties, "size"),
                ThinChild::Swap(s) => get_property_size_bytes(&s.properties, "size"),
            })
            .sum();

        let warn_pct = get_property_pct(&thin.properties, "overcommit_warn").unwrap_or(80);
        let deny_pct = get_property_pct(&thin.properties, "overcommit_deny");

        if pool_bytes > 0 {
            let usage_pct = (total_virtual * 100) / pool_bytes;
            if let Some(deny) = deny_pct
                && usage_pct > deny
            {
                ctx.error(
                    format!(
                        "thin pool `{}` virtual allocation ({usage_pct}%) exceeds overcommit_deny threshold ({deny}%)",
                        thin.name
                    ),
                    Some(thin.span.clone()),
                    Some(vg_name.to_string()),
                    None,
                );
            }
            if usage_pct > warn_pct {
                ctx.warning(
                    format!(
                        "thin pool `{}` virtual allocation ({usage_pct}%) exceeds overcommit_warn threshold ({warn_pct}%)",
                        thin.name
                    ),
                    Some(thin.span.clone()),
                    Some(vg_name.to_string()),
                    None,
                );
            }
        }
    }

    for child in &thin.children {
        match child {
            ThinChild::Filesystem(f) => validate_fs(ctx, f),
            ThinChild::Swap(s) => {
                ctx.check_block_name(&s.name, &s.span);
            }
        }
    }
}

fn validate_vdo(ctx: &mut ValidationContext, vdo: &VdoBlock, vg_name: &str) {
    ctx.check_block_name(&vdo.name, &vdo.span);

    // VDO requires size and virtual_size
    if !vdo.properties.iter().any(|p| p.key == "size") {
        ctx.error(
            "vdo block requires a `size` property".to_string(),
            Some(vdo.span.clone()),
            Some(vdo.name.clone()),
            None,
        );
    }
    if !vdo.properties.iter().any(|p| p.key == "virtual_size") {
        ctx.error(
            "vdo block requires a `virtual_size` property".to_string(),
            Some(vdo.span.clone()),
            Some(vdo.name.clone()),
            None,
        );
    }

    // virtual_size >= size
    let phys = get_property_size_bytes(&vdo.properties, "size");
    let virt = get_property_size_bytes(&vdo.properties, "virtual_size");
    if let (Some(p), Some(v)) = (phys, virt)
        && v < p
    {
        ctx.error(
            format!(
                "vdo `virtual_size` must be >= `size` (physical: {p} bytes, virtual: {v} bytes)"
            ),
            Some(vdo.span.clone()),
            Some(vdo.name.clone()),
            None,
        );
    }

    // Physical size must be at least 5G
    if let Some(p) = phys
        && p < 5 * 1024 * 1024 * 1024
    {
        ctx.error(
            "vdo physical size must be at least 5G".to_string(),
            Some(vdo.span.clone()),
            Some(vg_name.to_string()),
            None,
        );
    }

    for child in &vdo.children {
        match child {
            VdoChild::Filesystem(f) => validate_fs(ctx, f),
            VdoChild::Swap(s) => {
                ctx.check_block_name(&s.name, &s.span);
            }
        }
    }
}

// ─── SELinux Mount Validation ────────────────────────────────

fn validate_mount_selinux_inline(
    ctx: &mut ValidationContext,
    mount: &MountExpr,
    fs_type: FsType,
    block_name: &str,
    span: &Span,
) {
    if mount.context.is_some() && fs_type.supports_xattr() {
        ctx.warning(
            format!(
                "`context` on xattr-capable filesystem {} silently overrides all xattr labels",
                fs_type
            ),
            Some(span.clone()),
            Some(block_name.to_string()),
            Some(
                "consider using `defcontext` or `rootcontext` in an extended mount block instead"
                    .to_string(),
            ),
        );
    }
}

fn validate_mount_selinux_ext(
    ctx: &mut ValidationContext,
    mount: &MountBlockExt,
    fs_type: FsType,
    block_name: &str,
) {
    if mount.context.is_some() {
        let has_others =
            mount.fscontext.is_some() || mount.defcontext.is_some() || mount.rootcontext.is_some();
        if has_others {
            ctx.error(
                "`context` is mutually exclusive with `fscontext`, `defcontext`, and `rootcontext`".to_string(),
                Some(mount.span.clone()),
                Some(block_name.to_string()),
                Some(
                    "`context` overrides all xattr labels; the other three are for per-file labeling".to_string(),
                ),
            );
        }

        if fs_type.supports_xattr() {
            ctx.warning(
                format!(
                    "`context` on xattr-capable filesystem {} silently overrides all xattr labels",
                    fs_type
                ),
                Some(mount.span.clone()),
                Some(block_name.to_string()),
                Some("consider using `defcontext` or `rootcontext` instead".to_string()),
            );
        }
    }
}

// ─── Users Block Validation ─────────────────────────────────

fn validate_users(ctx: &mut ValidationContext, users: &UsersBlock) {
    // Validate password policy
    if let Some(ref policy) = users.policy {
        validate_password_policy(ctx, policy);
    }

    // Validate user declarations
    for user in &users.users {
        // Unique user name
        if let Some(prev) = ctx.user_names.get(&user.name) {
            ctx.error(
                format!(
                    "duplicate user name `{}` (previously declared at line {})",
                    user.name, prev.line
                ),
                Some(user.span.clone()),
                Some(user.name.clone()),
                None,
            );
        } else {
            ctx.user_names
                .insert(user.name.clone(), user.span.clone());
        }

        // UID must be positive and unique
        if let Some(uid) = get_property_int(&user.properties, "uid") {
            if uid <= 0 {
                ctx.error(
                    format!("user `{}` has invalid uid {uid} (must be positive)", user.name),
                    Some(user.span.clone()),
                    Some(user.name.clone()),
                    None,
                );
            } else if let Some(prev) = ctx.user_uids.get(&uid) {
                ctx.error(
                    format!(
                        "duplicate uid {uid} on user `{}` (previously assigned at line {})",
                        user.name, prev.line
                    ),
                    Some(user.span.clone()),
                    Some(user.name.clone()),
                    None,
                );
            } else {
                ctx.user_uids.insert(uid, user.span.clone());
            }
        }
    }

    // Validate group declarations
    for group in &users.groups {
        // Unique group name
        if let Some(prev) = ctx.group_names.get(&group.name) {
            ctx.error(
                format!(
                    "duplicate group name `{}` (previously declared at line {})",
                    group.name, prev.line
                ),
                Some(group.span.clone()),
                Some(group.name.clone()),
                None,
            );
        } else {
            ctx.group_names
                .insert(group.name.clone(), group.span.clone());
        }

        // GID must be positive and unique
        if let Some(gid) = get_property_int(&group.properties, "gid") {
            if gid <= 0 {
                ctx.error(
                    format!(
                        "group `{}` has invalid gid {gid} (must be positive)",
                        group.name
                    ),
                    Some(group.span.clone()),
                    Some(group.name.clone()),
                    None,
                );
            } else if let Some(prev) = ctx.group_gids.get(&gid) {
                ctx.error(
                    format!(
                        "duplicate gid {gid} on group `{}` (previously assigned at line {})",
                        group.name, prev.line
                    ),
                    Some(group.span.clone()),
                    Some(group.name.clone()),
                    None,
                );
            } else {
                ctx.group_gids.insert(gid, group.span.clone());
            }
        }
    }
}

fn validate_password_policy(ctx: &mut ValidationContext, policy: &PolicyBlock) {
    if let Some(ref complexity) = policy.complexity
        && let Some(min_len) = get_property_int(&complexity.properties, "min_length")
        && min_len <= 0
    {
        ctx.error(
            format!("password policy `min_length` must be positive, got {min_len}"),
            Some(complexity.span.clone()),
            None,
            None,
        );
    }

    if let Some(ref lockout) = policy.lockout {
        if let Some(attempts) = get_property_int(&lockout.properties, "attempts")
            && attempts <= 0
        {
            ctx.error(
                format!("lockout policy `attempts` must be positive, got {attempts}"),
                Some(lockout.span.clone()),
                None,
                None,
            );
        }

        if let Some(lockout_time) = get_property_int(&lockout.properties, "lockout_time")
            && lockout_time <= 0
        {
            ctx.error(
                format!(
                    "lockout policy `lockout_time` must be positive, got {lockout_time}"
                ),
                Some(lockout.span.clone()),
                None,
                None,
            );
        }
    }
}

// ─── Packages Block Validation ──────────────────────────────

fn validate_packages(ctx: &mut ValidationContext, pkgs: &PackagesBlock) {
    // Validate repository declarations
    for repo in &pkgs.repos {
        // Unique repo name
        if !ctx.repo_names.insert(repo.name.clone()) {
            ctx.error(
                format!("duplicate repository name `{}`", repo.name),
                Some(repo.span.clone()),
                Some(repo.name.clone()),
                None,
            );
        }

        // Repo must have baseurl or metalink
        let has_baseurl = get_property_str(&repo.properties, "baseurl").is_some();
        let has_metalink = get_property_str(&repo.properties, "metalink").is_some();
        if !has_baseurl && !has_metalink {
            ctx.error(
                format!(
                    "repository `{}` requires a `baseurl` or `metalink` property",
                    repo.name
                ),
                Some(repo.span.clone()),
                Some(repo.name.clone()),
                None,
            );
        }

        // Warn if gpgcheck not set
        let has_gpgcheck = repo.properties.iter().any(|p| p.key == "gpgcheck");
        if !has_gpgcheck {
            ctx.warning(
                format!(
                    "repository `{}` does not set `gpgcheck` — GPG verification is recommended",
                    repo.name
                ),
                Some(repo.span.clone()),
                Some(repo.name.clone()),
                Some("add `gpgcheck = true` to enable GPG signature verification".to_string()),
            );
        }
    }

    // Validate package declarations
    for pkg in &pkgs.packages {
        // Unique package name
        if !ctx.pkg_names.insert(pkg.name.clone()) {
            ctx.error(
                format!("duplicate package `{}`", pkg.name),
                Some(pkg.span.clone()),
                Some(pkg.name.clone()),
                None,
            );
        }

        // Valid state
        if let Some(state) = get_property_str(&pkg.properties, "state")
            && !["present", "absent", "latest"].contains(&state)
        {
            ctx.error(
                format!("invalid package state `{state}` for `{}`", pkg.name),
                Some(pkg.span.clone()),
                Some(pkg.name.clone()),
                Some("valid states: present, absent, latest".to_string()),
            );
        }
    }
}

// ─── Property Helpers ────────────────────────────────────────

fn get_property_str<'a>(props: &'a [Property], key: &str) -> Option<&'a str> {
    props
        .iter()
        .find(|p| p.key == key)
        .and_then(|p| match &p.value {
            Value::Ident(s)
            | Value::String(s)
            | Value::DevicePath(s)
            | Value::Path(s)
            | Value::Url(s) => Some(s.as_str()),
            _ => None,
        })
}

fn get_property_int(props: &[Property], key: &str) -> Option<i64> {
    props
        .iter()
        .find(|p| p.key == key)
        .and_then(|p| match &p.value {
            Value::Integer(n) => Some(*n),
            Value::Ident(s) => s.parse().ok(),
            _ => None,
        })
}

fn get_property_remaining(props: &[Property]) -> bool {
    props
        .iter()
        .any(|p| p.key == "size" && matches!(&p.value, Value::Remaining))
}

fn get_property_size_bytes(props: &[Property], key: &str) -> Option<u64> {
    props
        .iter()
        .find(|p| p.key == key)
        .and_then(|p| match &p.value {
            Value::Size(sv) => Some(sv.to_bytes()),
            _ => None,
        })
}

fn get_property_pct(props: &[Property], key: &str) -> Option<u64> {
    props
        .iter()
        .find(|p| p.key == key)
        .and_then(|p| match &p.value {
            Value::Percentage(n) => Some(*n),
            _ => None,
        })
}
