//! Conversion from the compiler's AST to the backend-agnostic Manifest.
//!
//! This module walks the parsed AST (`StorageFile`) and produces a `Manifest`
//! stripped of source spans and compiler internals, suitable for CBOR
//! serialization and consumption by the emitter and runtime agent.

use crate::ast;
use ironclad_manifest::*;

pub fn storage_file_to_manifest(file: &ast::StorageFile) -> Manifest {
    Manifest {
        manifest_version: 1,
        storage: StorageManifest {
            declarations: file.declarations.iter().map(convert_storage_decl).collect(),
        },
        selinux: file.selinux.as_ref().map(convert_selinux_block),
    }
}

// ─── Storage Declarations ───────────────────────────────────

fn convert_storage_decl(decl: &ast::StorageDecl) -> StorageDeclManifest {
    match decl {
        ast::StorageDecl::Disk(d) => StorageDeclManifest::Disk(convert_disk(d)),
        ast::StorageDecl::MdRaid(m) => StorageDeclManifest::MdRaid(convert_mdraid(m)),
        ast::StorageDecl::Zpool(z) => StorageDeclManifest::Zpool(convert_zpool(z)),
        ast::StorageDecl::Stratis(s) => StorageDeclManifest::Stratis(convert_stratis(s)),
        ast::StorageDecl::Multipath(m) => StorageDeclManifest::Multipath(convert_multipath(m)),
        ast::StorageDecl::Iscsi(i) => StorageDeclManifest::Iscsi(convert_iscsi(i)),
        ast::StorageDecl::Nfs(n) => StorageDeclManifest::Nfs(convert_nfs(n)),
        ast::StorageDecl::Tmpfs(t) => StorageDeclManifest::Tmpfs(convert_tmpfs(t)),
    }
}

fn convert_disk(d: &ast::DiskBlock) -> DiskManifest {
    DiskManifest {
        device: d.device.clone(),
        properties: d.properties.iter().map(convert_property).collect(),
        children: d.children.iter().map(convert_partition_child).collect(),
    }
}

fn convert_mdraid(m: &ast::MdRaidBlock) -> MdRaidManifest {
    MdRaidManifest {
        name: m.name.clone(),
        properties: m.properties.iter().map(convert_property).collect(),
        children: m.children.iter().map(convert_partition_child).collect(),
    }
}

fn convert_zpool(z: &ast::ZpoolBlock) -> ZpoolManifest {
    ZpoolManifest {
        name: z.name.clone(),
        properties: z.properties.iter().map(convert_property).collect(),
        vdevs: z.vdevs.iter().map(convert_vdev).collect(),
        datasets: z.datasets.iter().map(convert_dataset).collect(),
        zvols: z.zvols.iter().map(convert_zvol).collect(),
    }
}

fn convert_stratis(s: &ast::StratisBlock) -> StratisManifest {
    StratisManifest {
        name: s.name.clone(),
        properties: s.properties.iter().map(convert_property).collect(),
        filesystems: s.filesystems.iter().map(convert_stratis_fs).collect(),
    }
}

fn convert_multipath(m: &ast::MultipathBlock) -> MultipathManifest {
    MultipathManifest {
        name: m.name.clone(),
        properties: m.properties.iter().map(convert_property).collect(),
        paths: m.paths.iter().map(convert_path_block).collect(),
        children: m.children.iter().map(convert_partition_child).collect(),
    }
}

fn convert_iscsi(i: &ast::IscsiBlock) -> IscsiManifest {
    IscsiManifest {
        name: i.name.clone(),
        properties: i.properties.iter().map(convert_property).collect(),
        children: i.children.iter().map(convert_partition_child).collect(),
    }
}

fn convert_nfs(n: &ast::NfsBlock) -> NfsManifest {
    NfsManifest {
        name: n.name.clone(),
        properties: n.properties.iter().map(convert_property).collect(),
        mount_block: n.mount_block.as_ref().map(convert_mount_block),
    }
}

fn convert_tmpfs(t: &ast::TmpfsBlock) -> TmpfsManifest {
    TmpfsManifest {
        name: t.name.clone(),
        properties: t.properties.iter().map(convert_property).collect(),
        mount_block: t.mount_block.as_ref().map(convert_mount_block),
    }
}

// ─── Children ───────────────────────────────────────────────

fn convert_partition_child(c: &ast::PartitionChild) -> PartitionChildManifest {
    match c {
        ast::PartitionChild::Filesystem(f) => {
            PartitionChildManifest::Filesystem(Box::new(convert_fs(f)))
        }
        ast::PartitionChild::Luks(l) => PartitionChildManifest::Luks(convert_luks(l)),
        ast::PartitionChild::Integrity(i) => {
            PartitionChildManifest::Integrity(convert_integrity(i))
        }
        ast::PartitionChild::Lvm(l) => PartitionChildManifest::Lvm(convert_lvm(l)),
        ast::PartitionChild::Raw(r) => PartitionChildManifest::Raw(convert_raw(r)),
        ast::PartitionChild::Swap(s) => PartitionChildManifest::Swap(convert_swap(s)),
    }
}

fn convert_luks_child(c: &ast::LuksChild) -> LuksChildManifest {
    match c {
        ast::LuksChild::Filesystem(f) => LuksChildManifest::Filesystem(Box::new(convert_fs(f))),
        ast::LuksChild::Lvm(l) => LuksChildManifest::Lvm(convert_lvm(l)),
        ast::LuksChild::Swap(s) => LuksChildManifest::Swap(convert_swap(s)),
    }
}

fn convert_integrity_child(c: &ast::IntegrityChild) -> IntegrityChildManifest {
    match c {
        ast::IntegrityChild::Filesystem(f) => {
            IntegrityChildManifest::Filesystem(Box::new(convert_fs(f)))
        }
        ast::IntegrityChild::Lvm(l) => IntegrityChildManifest::Lvm(convert_lvm(l)),
        ast::IntegrityChild::Swap(s) => IntegrityChildManifest::Swap(convert_swap(s)),
    }
}

fn convert_lvm_child(c: &ast::LvmChild) -> LvmChildManifest {
    match c {
        ast::LvmChild::Filesystem(f) => LvmChildManifest::Filesystem(Box::new(convert_fs(f))),
        ast::LvmChild::Swap(s) => LvmChildManifest::Swap(convert_swap(s)),
        ast::LvmChild::Thin(t) => LvmChildManifest::Thin(convert_thin(t)),
        ast::LvmChild::Vdo(v) => LvmChildManifest::Vdo(convert_vdo(v)),
    }
}

fn convert_thin_child(c: &ast::ThinChild) -> ThinChildManifest {
    match c {
        ast::ThinChild::Filesystem(f) => ThinChildManifest::Filesystem(Box::new(convert_fs(f))),
        ast::ThinChild::Swap(s) => ThinChildManifest::Swap(convert_swap(s)),
    }
}

fn convert_vdo_child(c: &ast::VdoChild) -> VdoChildManifest {
    match c {
        ast::VdoChild::Filesystem(f) => VdoChildManifest::Filesystem(Box::new(convert_fs(f))),
        ast::VdoChild::Swap(s) => VdoChildManifest::Swap(convert_swap(s)),
    }
}

fn convert_zvol_child(c: &ast::ZvolChild) -> ZvolChildManifest {
    match c {
        ast::ZvolChild::Swap(s) => ZvolChildManifest::Swap(convert_swap(s)),
        ast::ZvolChild::Filesystem(f) => ZvolChildManifest::Filesystem(Box::new(convert_fs(f))),
        ast::ZvolChild::Luks(l) => ZvolChildManifest::Luks(convert_luks(l)),
    }
}

// ─── Block Types ────────────────────────────────────────────

fn convert_fs(f: &ast::FsBlock) -> FsManifest {
    FsManifest {
        fs_type: convert_fs_type(f.fs_type),
        name: f.name.clone(),
        properties: f.properties.iter().map(convert_property).collect(),
        subvolumes: f.subvolumes.iter().map(convert_subvol).collect(),
        mount_block: f.mount_block.as_ref().map(convert_mount_block),
    }
}

fn convert_fs_type(t: ast::FsType) -> FsTypeManifest {
    match t {
        ast::FsType::Ext4 => FsTypeManifest::Ext4,
        ast::FsType::Xfs => FsTypeManifest::Xfs,
        ast::FsType::Btrfs => FsTypeManifest::Btrfs,
        ast::FsType::Fat32 => FsTypeManifest::Fat32,
        ast::FsType::Ntfs => FsTypeManifest::Ntfs,
    }
}

fn convert_subvol(s: &ast::SubvolBlock) -> SubvolManifest {
    SubvolManifest {
        name: s.name.clone(),
        properties: s.properties.iter().map(convert_property).collect(),
        mount_block: s.mount_block.as_ref().map(convert_mount_block),
    }
}

fn convert_luks(l: &ast::LuksBlock) -> LuksManifest {
    LuksManifest {
        version: match l.version {
            ast::LuksVersion::Luks1 => LuksVersionManifest::Luks1,
            ast::LuksVersion::Luks2 => LuksVersionManifest::Luks2,
        },
        name: l.name.clone(),
        properties: l.properties.iter().map(convert_property).collect(),
        children: l.children.iter().map(convert_luks_child).collect(),
    }
}

fn convert_integrity(i: &ast::IntegrityBlock) -> IntegrityManifest {
    IntegrityManifest {
        name: i.name.clone(),
        properties: i.properties.iter().map(convert_property).collect(),
        children: i.children.iter().map(convert_integrity_child).collect(),
    }
}

fn convert_lvm(l: &ast::LvmBlock) -> LvmManifest {
    LvmManifest {
        name: l.name.clone(),
        properties: l.properties.iter().map(convert_property).collect(),
        children: l.children.iter().map(convert_lvm_child).collect(),
    }
}

fn convert_thin(t: &ast::ThinBlock) -> ThinManifest {
    ThinManifest {
        name: t.name.clone(),
        properties: t.properties.iter().map(convert_property).collect(),
        children: t.children.iter().map(convert_thin_child).collect(),
    }
}

fn convert_vdo(v: &ast::VdoBlock) -> VdoManifest {
    VdoManifest {
        name: v.name.clone(),
        properties: v.properties.iter().map(convert_property).collect(),
        children: v.children.iter().map(convert_vdo_child).collect(),
    }
}

fn convert_vdev(v: &ast::VdevBlock) -> VdevManifest {
    VdevManifest {
        name: v.name.clone(),
        properties: v.properties.iter().map(convert_property).collect(),
    }
}

fn convert_dataset(d: &ast::DatasetBlock) -> DatasetManifest {
    DatasetManifest {
        name: d.name.clone(),
        properties: d.properties.iter().map(convert_property).collect(),
        children: d.children.iter().map(convert_dataset).collect(),
    }
}

fn convert_zvol(z: &ast::ZvolBlock) -> ZvolManifest {
    ZvolManifest {
        name: z.name.clone(),
        properties: z.properties.iter().map(convert_property).collect(),
        children: z.children.iter().map(convert_zvol_child).collect(),
    }
}

fn convert_stratis_fs(s: &ast::StratisFilesystem) -> StratisFilesystemManifest {
    StratisFilesystemManifest {
        name: s.name.clone(),
        properties: s.properties.iter().map(convert_property).collect(),
        mount_block: s.mount_block.as_ref().map(convert_mount_block),
    }
}

fn convert_path_block(p: &ast::PathBlock) -> PathManifest {
    PathManifest {
        device: p.device.clone(),
        properties: p.properties.iter().map(convert_property).collect(),
    }
}

fn convert_swap(s: &ast::SwapBlock) -> SwapManifest {
    SwapManifest {
        name: s.name.clone(),
        properties: s.properties.iter().map(convert_property).collect(),
    }
}

fn convert_raw(r: &ast::RawBlock) -> RawManifest {
    RawManifest {
        name: r.name.clone(),
        properties: r.properties.iter().map(convert_property).collect(),
    }
}

// ─── Mount ──────────────────────────────────────────────────

fn convert_mount_block(m: &ast::MountBlockExt) -> MountBlockManifest {
    MountBlockManifest {
        target: m.target.clone(),
        options: m.options.clone(),
        automount: m.automount,
        timeout: m.timeout,
        requires: m.requires.clone(),
        before: m.before.clone(),
        context: m.context.as_ref().map(convert_selinux_context),
        fscontext: m.fscontext.as_ref().map(convert_selinux_context),
        defcontext: m.defcontext.as_ref().map(convert_selinux_context),
        rootcontext: m.rootcontext.as_ref().map(convert_selinux_context),
    }
}

// ─── SELinux ────────────────────────────────────────────────

fn convert_selinux_block(s: &ast::SelinuxBlock) -> SelinuxManifest {
    SelinuxManifest {
        properties: s.properties.iter().map(convert_property).collect(),
        users: s.users.iter().map(convert_selinux_user).collect(),
        roles: s.roles.iter().map(convert_selinux_role).collect(),
        booleans: s.booleans.iter().map(convert_property).collect(),
    }
}

fn convert_selinux_user(u: &ast::SelinuxUserDecl) -> SelinuxUserManifest {
    SelinuxUserManifest {
        name: u.name.clone(),
        properties: u.properties.iter().map(convert_property).collect(),
    }
}

fn convert_selinux_role(r: &ast::SelinuxRoleDecl) -> SelinuxRoleManifest {
    SelinuxRoleManifest {
        name: r.name.clone(),
        properties: r.properties.iter().map(convert_property).collect(),
    }
}

fn convert_selinux_context(c: &ast::SelinuxContext) -> SelinuxContextManifest {
    SelinuxContextManifest {
        user: c.user.clone(),
        role: c.role.clone(),
        typ: c.typ.clone(),
        range: convert_mls_range(&c.range),
        raw: c.raw.clone(),
    }
}

fn convert_mls_range(r: &ast::MlsRange) -> MlsRangeManifest {
    MlsRangeManifest {
        low: SensitivityManifest { level: r.low.level },
        high: r
            .high
            .as_ref()
            .map(|h| SensitivityManifest { level: h.level }),
        categories: r.categories.clone(),
    }
}

// ─── Properties ─────────────────────────────────────────────

fn convert_property(p: &ast::Property) -> PropertyManifest {
    PropertyManifest {
        key: p.key.clone(),
        value: convert_value(&p.value),
    }
}

fn convert_value(v: &ast::Value) -> ValueManifest {
    match v {
        ast::Value::String(s) => ValueManifest::String(s.clone()),
        ast::Value::Integer(i) => ValueManifest::Integer(*i),
        ast::Value::Boolean(b) => ValueManifest::Boolean(*b),
        ast::Value::Size(s) => ValueManifest::Size(SizeValueManifest {
            amount: s.amount,
            unit: convert_size_unit(s.unit),
        }),
        ast::Value::Percentage(p) => ValueManifest::Percentage(*p),
        ast::Value::Remaining => ValueManifest::Remaining,
        ast::Value::Array(a) => ValueManifest::Array(a.iter().map(convert_value).collect()),
        ast::Value::Path(p) => ValueManifest::Path(p.clone()),
        ast::Value::DevicePath(p) => ValueManifest::DevicePath(p.clone()),
        ast::Value::Ident(i) => ValueManifest::Ident(i.clone()),
        ast::Value::Url(u) => ValueManifest::Url(u.clone()),
        ast::Value::Mount(m) => ValueManifest::Mount(convert_mount_expr(m)),
        ast::Value::SelinuxContext(c) => ValueManifest::SelinuxContext(convert_selinux_context(c)),
    }
}

fn convert_mount_expr(m: &ast::MountExpr) -> MountExprManifest {
    MountExprManifest {
        target: m.target.clone(),
        options: m.options.clone(),
        context: m.context.as_ref().map(convert_selinux_context),
    }
}

fn convert_size_unit(u: ast::SizeUnit) -> SizeUnitManifest {
    match u {
        ast::SizeUnit::B => SizeUnitManifest::B,
        ast::SizeUnit::K => SizeUnitManifest::K,
        ast::SizeUnit::M => SizeUnitManifest::M,
        ast::SizeUnit::G => SizeUnitManifest::G,
        ast::SizeUnit::T => SizeUnitManifest::T,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_storage;
    use crate::validate::validate;

    #[test]
    fn convert_simple_disk() {
        let input = r#"
disk /dev/sda {
    label = gpt

    ext4 root {
        index = 1
        size = 50G
        mount = /
    }
}
"#;
        let ast = parse_storage(input).expect("parse");
        let _warnings = validate(&ast).expect("validate");
        let manifest = storage_file_to_manifest(&ast);

        assert_eq!(manifest.manifest_version, 1);
        assert_eq!(manifest.storage.declarations.len(), 1);
        assert!(manifest.selinux.is_none());

        if let StorageDeclManifest::Disk(disk) = &manifest.storage.declarations[0] {
            assert_eq!(disk.device, "/dev/sda");
            assert_eq!(disk.properties.len(), 1);
            assert_eq!(disk.properties[0].key, "label");
            assert_eq!(disk.children.len(), 1);
            if let PartitionChildManifest::Filesystem(fs) = &disk.children[0] {
                assert_eq!(fs.fs_type, FsTypeManifest::Ext4);
                assert_eq!(fs.name, "root");
            } else {
                panic!("expected filesystem child");
            }
        } else {
            panic!("expected disk declaration");
        }
    }

    #[test]
    fn convert_server_layout() {
        let input = r#"
disk /dev/sda {
    label = gpt

    fat32 efi {
        index = 1
        size = 1G
        type = ef00
        mount = /boot/efi [nodev, nosuid, noexec] context system_u:object_r:boot_t:s0
    }

    ext4 boot {
        index = 2
        size = 1G
        mount = /boot [nodev, nosuid, noexec]
    }
}

disk /dev/nvme0n1 {
    label = gpt

    luks2 system {
        index = 1
        size = remaining
        cipher = aes-xts-plain64
        key_size = 512
        tpm2 = true

        lvm vg_system {
            btrfs root {
                size = remaining
                compress = zstd:1

                subvol @ { mount = / }
                subvol @home { mount = /home [nodev, nosuid] }
                subvol @var { mount = /var [nodev, nosuid, noexec] }
                subvol @snapshots { mount = /.snapshots }
            }

            swap swap0 { size = 32G }
        }
    }
}
"#;
        let ast = parse_storage(input).expect("parse");
        let _warnings = validate(&ast).expect("validate");
        let manifest = storage_file_to_manifest(&ast);

        assert_eq!(manifest.storage.declarations.len(), 2);

        // First disk — EFI + boot
        if let StorageDeclManifest::Disk(d) = &manifest.storage.declarations[0] {
            assert_eq!(d.device, "/dev/sda");
            assert_eq!(d.children.len(), 2);
        } else {
            panic!("expected disk");
        }

        // Second disk — LUKS + LVM + btrfs subvols
        if let StorageDeclManifest::Disk(d) = &manifest.storage.declarations[1] {
            assert_eq!(d.device, "/dev/nvme0n1");
            assert_eq!(d.children.len(), 1);
            if let PartitionChildManifest::Luks(luks) = &d.children[0] {
                assert_eq!(luks.version, LuksVersionManifest::Luks2);
                assert_eq!(luks.children.len(), 1);
                if let LuksChildManifest::Lvm(lvm) = &luks.children[0] {
                    assert_eq!(lvm.name, "vg_system");
                    assert_eq!(lvm.children.len(), 2); // btrfs + swap
                    if let LvmChildManifest::Filesystem(fs) = &lvm.children[0] {
                        assert_eq!(fs.fs_type, FsTypeManifest::Btrfs);
                        assert_eq!(fs.subvolumes.len(), 4);
                    } else {
                        panic!("expected filesystem");
                    }
                } else {
                    panic!("expected LVM");
                }
            } else {
                panic!("expected LUKS");
            }
        } else {
            panic!("expected disk");
        }
    }

    #[test]
    fn convert_manifest_cbor_round_trip() {
        let input = r#"
disk /dev/sda {
    label = gpt

    ext4 root {
        index = 1
        size = 50G
        mount = /
    }

    swap swap0 {
        index = 2
        size = 16G
    }
}
"#;
        let ast = parse_storage(input).expect("parse");
        let _warnings = validate(&ast).expect("validate");
        let manifest = storage_file_to_manifest(&ast);

        let cbor = ironclad_manifest::serialize_manifest(&manifest).expect("serialize");
        let deserialized = ironclad_manifest::deserialize_manifest(&cbor).expect("deserialize");
        assert_eq!(manifest, deserialized);
    }

    #[test]
    fn convert_with_selinux() {
        let input = r#"
disk /dev/sda {
    label = gpt
    ext4 root { index = 1; size = 50G; mount = / }
}

selinux {
    mode = enforcing
    type = targeted

    user system_u {
        roles = [system_r]
        range = s0-s15:c0.c1023
    }

    user admin_u {
        roles = [staff_r, sysadm_r]
        range = s0-s15:c0.c1023
        default = true
    }

    role staff_r {
        types = [staff_t, user_t]
    }

    booleans {
        httpd_can_network_connect = true
        samba_enable_home_dirs = false
    }
}
"#;
        let ast = parse_storage(input).expect("parse");
        let _warnings = validate(&ast).expect("validate");
        let manifest = storage_file_to_manifest(&ast);

        assert!(manifest.selinux.is_some());
        let se = manifest.selinux.as_ref().unwrap();
        assert_eq!(se.users.len(), 2);
        assert_eq!(se.users[0].name, "system_u");
        assert_eq!(se.users[1].name, "admin_u");
        assert_eq!(se.roles.len(), 1);
        assert_eq!(se.roles[0].name, "staff_r");
        assert_eq!(se.booleans.len(), 2);
    }
}
