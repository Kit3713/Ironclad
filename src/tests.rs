use crate::ast::*;
use crate::parser::{parse_source, parse_storage};
use crate::validate::{validate, validate_storage};

// ─── Basic Parsing Tests ─────────────────────────────────────

#[test]
fn parse_simple_disk() {
    let input = r#"
disk /dev/sda {
    label = gpt
}
"#;
    let ast = parse_storage(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 1);
    match &ast.declarations[0] {
        StorageDecl::Disk(d) => {
            assert_eq!(d.device, "/dev/sda");
            assert_eq!(d.properties.len(), 1);
            assert_eq!(d.properties[0].key, "label");
        }
        _ => panic!("expected disk"),
    }
}

#[test]
fn parse_disk_with_partitions() {
    let input = r#"
disk /dev/sda {
    label = gpt

    fat32 efi {
        index = 1
        size = 1G
        type = ef00
        mount = /boot/efi [nodev, nosuid, noexec]
    }

    ext4 boot {
        index = 2
        size = 1G
        mount = /boot [nodev, nosuid, noexec]
    }

    luks2 system {
        index = 3
        size = remaining
        cipher = aes-xts-plain64
        key_size = 512

        lvm vg0 {
            ext4 root { size = 50G; mount = / }
            swap swap0 { size = 16G }

            thin pool0 {
                size = 200G

                xfs data { size = 300G; mount = /srv }
            }
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 1);

    if let StorageDecl::Disk(d) = &ast.declarations[0] {
        assert_eq!(d.children.len(), 3);

        // EFI partition
        if let PartitionChild::Filesystem(f) = &d.children[0] {
            assert_eq!(f.fs_type, FsType::Fat32);
            assert_eq!(f.name, "efi");
        } else {
            panic!("expected fat32");
        }

        // Boot partition
        if let PartitionChild::Filesystem(f) = &d.children[1] {
            assert_eq!(f.fs_type, FsType::Ext4);
            assert_eq!(f.name, "boot");
        } else {
            panic!("expected ext4");
        }

        // LUKS partition with LVM
        if let PartitionChild::Luks(l) = &d.children[2] {
            assert_eq!(l.version, LuksVersion::Luks2);
            assert_eq!(l.name, "system");
            assert_eq!(l.children.len(), 1);

            if let LuksChild::Lvm(lvm) = &l.children[0] {
                assert_eq!(lvm.name, "vg0");
                assert_eq!(lvm.children.len(), 3); // root, swap, thin
            } else {
                panic!("expected lvm inside luks");
            }
        } else {
            panic!("expected luks2");
        }
    } else {
        panic!("expected disk");
    }
}

#[test]
fn parse_btrfs_with_subvolumes() {
    let input = r#"
disk /dev/nvme0n1 {
    label = gpt

    btrfs root {
        index = 1
        size = remaining
        compress = zstd:1

        subvol @ {
            mount = /
        }

        subvol @home {
            mount = /home [nodev, nosuid, compress=zstd:3]
            quota = 50G
        }

        subvol @var_log {
            mount = /var/log [nodev, nosuid, noexec]
            quota = 10G
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    if let StorageDecl::Disk(d) = &ast.declarations[0] {
        if let PartitionChild::Filesystem(f) = &d.children[0] {
            assert_eq!(f.fs_type, FsType::Btrfs);
            assert_eq!(f.subvolumes.len(), 3);
            assert_eq!(f.subvolumes[0].name, "@");
            assert_eq!(f.subvolumes[1].name, "@home");
            assert_eq!(f.subvolumes[2].name, "@var_log");
        } else {
            panic!("expected btrfs");
        }
    }
}

#[test]
fn parse_mdraid() {
    let input = r#"
mdraid md0 {
    level = 10
    disks = [/dev/sda, /dev/sdb, /dev/sdc, /dev/sdd]
    chunk = 512K

    luks2 encrypted_array {
        cipher = aes-xts-plain64
        key_size = 512
        tpm2 = true

        lvm vg_data {
            xfs databases {
                size = 500G
                mount = /var/lib/postgres [nodev, nosuid]
            }
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 1);

    if let StorageDecl::MdRaid(md) = &ast.declarations[0] {
        assert_eq!(md.name, "md0");
        assert_eq!(md.children.len(), 1);
    }
}

#[test]
fn parse_whole_disk() {
    let input = r#"
disk /dev/sdc {
    label = none

    xfs scratch {
        mount = /mnt/scratch
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let warnings = validate_storage(&ast).expect("should validate");
    assert!(warnings.is_empty());
}

#[test]
fn parse_mount_with_selinux_context() {
    let input = r#"
disk /dev/sda {
    label = gpt

    fat32 efi {
        index = 1
        size = 1G
        type = ef00
        mount = /boot/efi [nodev, nosuid, noexec] context system_u:object_r:boot_t:s0
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    if let StorageDecl::Disk(d) = &ast.declarations[0] {
        if let PartitionChild::Filesystem(f) = &d.children[0] {
            let mount_prop = f.properties.iter().find(|p| p.key == "mount").unwrap();
            if let Value::Mount(ref m) = mount_prop.value {
                assert_eq!(m.target, "/boot/efi");
                assert_eq!(m.options.len(), 3);
                assert!(m.context.is_some());
                let ctx = m.context.as_ref().unwrap();
                assert_eq!(ctx.user, "system_u");
                assert_eq!(ctx.role, "object_r");
                assert_eq!(ctx.typ, "boot_t");
                assert_eq!(ctx.range.low.level, 0);
            } else {
                panic!("expected mount value");
            }
        }
    }
}

#[test]
fn parse_extended_mount_block() {
    let input = r#"
disk /dev/sda {
    label = gpt

    ext4 data {
        index = 1
        size = 500G

        mount {
            target = /srv/data
            options = [nodev, nosuid, noexec]
            automount = false
            timeout = 30
            requires = [network-online.target]
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    if let StorageDecl::Disk(d) = &ast.declarations[0] {
        if let PartitionChild::Filesystem(f) = &d.children[0] {
            let mb = f.mount_block.as_ref().expect("should have mount block");
            assert_eq!(mb.target.as_deref(), Some("/srv/data"));
            assert_eq!(mb.options.len(), 3);
            assert_eq!(mb.automount, Some(false));
            assert_eq!(mb.timeout, Some(30));
            assert_eq!(mb.requires.len(), 1);
        }
    }
}

#[test]
fn parse_extended_mount_with_selinux() {
    let input = r#"
disk /dev/sda {
    label = gpt

    ext4 containers {
        index = 1
        size = 200G

        mount {
            target = /var/lib/containers
            options = [nodev, nosuid]
            defcontext = system_u:object_r:container_var_lib_t:s0
            rootcontext = system_u:object_r:container_var_lib_t:s0
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    if let StorageDecl::Disk(d) = &ast.declarations[0] {
        if let PartitionChild::Filesystem(f) = &d.children[0] {
            let mb = f.mount_block.as_ref().unwrap();
            assert!(mb.defcontext.is_some());
            assert!(mb.rootcontext.is_some());
            assert!(mb.context.is_none());
        }
    }
}

#[test]
fn parse_mls_sensitivity_range() {
    let input = r#"
disk /dev/sda {
    label = gpt

    ext4 shared_data {
        index = 1
        size = 100G

        mount {
            target = /srv/shared
            options = [nodev, nosuid, noexec]
            defcontext = system_u:object_r:shared_content_t:s0-s9:c0.c255
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    if let StorageDecl::Disk(d) = &ast.declarations[0] {
        if let PartitionChild::Filesystem(f) = &d.children[0] {
            let mb = f.mount_block.as_ref().unwrap();
            let ctx = mb.defcontext.as_ref().unwrap();
            assert_eq!(ctx.range.low.level, 0);
            assert_eq!(ctx.range.high.as_ref().unwrap().level, 9);
            assert_eq!(ctx.range.categories.as_deref(), Some("c0.c255"));
        }
    }
}

// ─── Multi-Disk Scenario from Spec ───────────────────────────

#[test]
fn parse_multi_disk_scenario() {
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
    let ast = parse_storage(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 2);

    // Validate it
    let warnings = validate_storage(&ast).expect("should validate without errors");
    // May have warnings for context on fat32, that's fine
    let _ = warnings;
}

// ─── Validation Tests ────────────────────────────────────────

#[test]
fn validate_rejects_missing_disk_label() {
    let input = r#"
disk /dev/sda {
    ext4 root {
        size = 50G
        mount = /
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "should reject disk without label");
}

#[test]
fn validate_rejects_whole_disk_multiple_children() {
    let input = r#"
disk /dev/sda {
    label = none

    ext4 root { size = 50G; mount = / }
    swap swap0 { size = 16G }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "label=none with 2 children should fail");
}

#[test]
fn validate_rejects_duplicate_mount_targets() {
    let input = r#"
disk /dev/sda {
    label = gpt

    ext4 root1 {
        index = 1
        size = 50G
        mount = /data
    }

    ext4 root2 {
        index = 2
        size = 50G
        mount = /data
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "duplicate mount targets should fail");
}

#[test]
fn validate_rejects_duplicate_partition_index() {
    let input = r#"
disk /dev/sda {
    label = gpt

    ext4 part_a {
        index = 1
        size = 50G
        mount = /a
    }

    ext4 part_b {
        index = 1
        size = 50G
        mount = /b
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "duplicate partition index should fail");
}

#[test]
fn validate_rejects_multiple_remaining() {
    let input = r#"
disk /dev/sda {
    label = gpt

    ext4 part_a {
        index = 1
        size = remaining
        mount = /a
    }

    ext4 part_b {
        index = 2
        size = remaining
        mount = /b
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "multiple remaining should fail");
}

#[test]
fn validate_rejects_subvol_outside_btrfs() {
    let input = r#"
disk /dev/sda {
    label = gpt

    ext4 root {
        index = 1
        size = 50G

        subvol @test {
            mount = /test
        }
    }
}
"#;
    // This should fail at parse time because subvol is only valid in btrfs,
    // but our grammar allows it in fs_body — so validation catches it.
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "subvol in ext4 should fail validation");
}

#[test]
fn validate_selinux_context_mutual_exclusivity() {
    let input = r#"
disk /dev/sda {
    label = gpt

    ext4 data {
        index = 1
        size = 100G

        mount {
            target = /srv/data
            context = system_u:object_r:data_t:s0
            defcontext = system_u:object_r:data_t:s0
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "context + defcontext together should fail");
}

#[test]
fn validate_luks_max_one_fs_without_lvm() {
    let input = r#"
disk /dev/sda {
    label = gpt

    luks2 system {
        index = 1
        size = remaining

        ext4 root { size = 50G; mount = / }
        ext4 home { size = 100G; mount = /home }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(
        result.is_err(),
        "luks without lvm should allow at most 1 fs"
    );
}

#[test]
fn validate_mdraid_no_shared_disks() {
    let input = r#"
mdraid md0 {
    level = 1
    disks = [/dev/sda1, /dev/sdb1]

    ext4 boot {
        mount = /boot
    }
}

mdraid md1 {
    level = 1
    disks = [/dev/sda1, /dev/sdc1]

    ext4 data {
        mount = /data
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "/dev/sda1 in two arrays should fail");
}

// ─── Raw Block ───────────────────────────────────────────────

#[test]
fn parse_raw_block() {
    let input = r#"
disk /dev/sda {
    label = gpt

    raw bios_boot {
        index = 1
        size = 1M
        type = ef02
    }

    ext4 root {
        index = 2
        size = remaining
        mount = /
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    if let StorageDecl::Disk(d) = &ast.declarations[0] {
        assert_eq!(d.children.len(), 2);
        assert!(matches!(&d.children[0], PartitionChild::Raw(_)));
    }
}

// ─── Swap Block ──────────────────────────────────────────────

#[test]
fn parse_swap_with_properties() {
    let input = r#"
disk /dev/sda {
    label = gpt

    swap swap0 {
        index = 1
        size = 32G
        priority = 100
        discard = true
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    if let StorageDecl::Disk(d) = &ast.declarations[0] {
        if let PartitionChild::Swap(s) = &d.children[0] {
            assert_eq!(s.name, "swap0");
            assert_eq!(s.properties.len(), 4);
        }
    }
}

// ─── Explicit Positioning ────────────────────────────────────

#[test]
fn parse_explicit_positioning() {
    let input = r#"
disk /dev/sdb {
    label = gpt

    xfs fast_tier {
        index = 1
        start = 1M
        end = 500G
        type = 8300
        mount = /srv/fast
    }

    xfs slow_tier {
        index = 2
        start = 500G
        end = -1
        type = 8300
        mount = /srv/bulk
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 1);
}

// ─── Comments ────────────────────────────────────────────────

#[test]
fn parse_with_comments() {
    let input = r#"
# This is a system disk
disk /dev/sda {
    label = gpt  # GPT partition table

    # EFI partition
    fat32 efi {
        index = 1
        size = 1G
        mount = /boot/efi
    }
}
"#;
    let ast = parse_storage(input).expect("should parse with comments");
    assert_eq!(ast.declarations.len(), 1);
}

// ─── NTFS Block ──────────────────────────────────────────────

#[test]
fn parse_ntfs() {
    let input = r#"
disk /dev/sda {
    label = gpt

    ntfs shared {
        index = 1
        size = 100G
        label = "SHARED"
        mount = /mnt/shared [nodev, nosuid, noexec]
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    if let StorageDecl::Disk(d) = &ast.declarations[0] {
        if let PartitionChild::Filesystem(f) = &d.children[0] {
            assert_eq!(f.fs_type, FsType::Ntfs);
        }
    }
}

// ─── Full RAID+LVM+Encryption Stack from Spec ───────────────

#[test]
fn parse_raid_lvm_encryption_stack() {
    let input = r#"
mdraid md0 {
    level = 1
    disks = [/dev/sda1, /dev/sdb1]
    metadata = 1.0

    ext4 boot {
        mount = /boot [nodev, nosuid, noexec]
    }
}

mdraid md1 {
    level = 10
    disks = [/dev/sda2, /dev/sdb2, /dev/sdc1, /dev/sdd1]
    chunk = 512K

    luks2 encrypted_array {
        cipher = aes-xts-plain64
        key_size = 512
        tpm2 = true

        lvm vg_data {
            thin pool0 {
                size = 90%

                xfs databases {
                    size = 500G
                    su = 256K
                    sw = 4
                    mount = /var/lib/postgres [nodev, nosuid]
                }

                xfs objects {
                    size = 2T
                    mount = /srv/objects [nodev, nosuid, noexec]
                }
            }

            ext4 logs {
                size = remaining
                mount = /var/log [nodev, nosuid, noexec]
            }
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse complex stack");
    assert_eq!(ast.declarations.len(), 2);
    let warnings = validate_storage(&ast).expect("should validate");
    let _ = warnings;
}

// ─── Error Reporting Quality ─────────────────────────────────

#[test]
fn error_reports_include_location() {
    let input = "disk /dev/sda { label = gpt }\ndisk /dev/sda { }";
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    match result {
        Err(crate::errors::IroncladError::ValidationError { errors }) => {
            // Should have error about missing label on second disk
            assert!(!errors.is_empty());
            let first = &errors[0];
            assert!(first.span.is_some());
        }
        _ => {
            // Second disk missing label — should be an error
            panic!("expected validation error");
        }
    }
}

// ─── Whole-Disk Encryption ───────────────────────────────────

#[test]
fn parse_whole_disk_encryption() {
    let input = r#"
disk /dev/sdd {
    label = none

    luks2 secure_scratch {
        xfs encrypted_data {
            mount = /mnt/secure
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let warnings = validate_storage(&ast).expect("should validate");
    let _ = warnings;
}

// ─── luks1 ───────────────────────────────────────────────────

#[test]
fn parse_luks1() {
    let input = r#"
disk /dev/sda {
    label = gpt

    luks1 legacy_boot {
        index = 2
        size = 1G
        cipher = aes-xts-plain64
        key_size = 256

        ext4 boot_enc {
            mount = /boot
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    if let StorageDecl::Disk(d) = &ast.declarations[0] {
        if let PartitionChild::Luks(l) = &d.children[0] {
            assert_eq!(l.version, LuksVersion::Luks1);
        }
    }
}

// ─── ZFS Pool ───────────────────────────────────────────────

#[test]
fn parse_zpool_basic() {
    let input = r#"
zpool tank {
    ashift = 12
    compression = lz4

    vdev data0 {
        type = mirror
        members = [/dev/sda, /dev/sdb]
    }

    dataset root {
        mountpoint = /tank
        quota = 100G
    }

    zvol swap_vol {
        size = 16G
        blocksize = 8K

        swap zswap {}
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 1);

    if let StorageDecl::Zpool(zp) = &ast.declarations[0] {
        assert_eq!(zp.name, "tank");
        assert_eq!(zp.vdevs.len(), 1);
        assert_eq!(zp.vdevs[0].name, "data0");
        assert_eq!(zp.datasets.len(), 1);
        assert_eq!(zp.datasets[0].name, "root");
        assert_eq!(zp.zvols.len(), 1);
        assert_eq!(zp.zvols[0].name, "swap_vol");
    } else {
        panic!("expected zpool");
    }
}

#[test]
fn parse_zpool_nested_datasets() {
    let input = r#"
zpool data {
    vdev stripe0 {
        type = stripe
        members = [/dev/sda]
    }

    dataset home {
        mountpoint = /home

        dataset alice {
            quota = 50G
        }

        dataset bob {
            quota = 50G
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    if let StorageDecl::Zpool(zp) = &ast.declarations[0] {
        assert_eq!(zp.datasets.len(), 1);
        assert_eq!(zp.datasets[0].children.len(), 2);
        assert_eq!(zp.datasets[0].children[0].name, "alice");
        assert_eq!(zp.datasets[0].children[1].name, "bob");
    } else {
        panic!("expected zpool");
    }
}

#[test]
fn validate_zpool_vdev_member_count() {
    let input = r#"
zpool bad {
    vdev m0 {
        type = mirror
        members = [/dev/sda]
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "mirror with 1 member should fail");
}

#[test]
fn validate_zpool_zvol_requires_size() {
    let input = r#"
zpool pool0 {
    vdev v0 {
        type = stripe
        members = [/dev/sda]
    }

    zvol myvol {
        blocksize = 8K
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "zvol without size should fail");
}

// ─── Stratis ────────────────────────────────────────────────

#[test]
fn parse_stratis_basic() {
    let input = r#"
stratis mypool {
    disks = [/dev/sda, /dev/sdb]
    encrypted = true

    filesystem data {
        size_limit = 500G

        mount {
            target = /srv/data
            options = [nodev, nosuid]
        }
    }

    filesystem logs {
        size_limit = 50G

        mount {
            target = /var/log
            options = [nodev, nosuid, noexec]
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 1);

    if let StorageDecl::Stratis(s) = &ast.declarations[0] {
        assert_eq!(s.name, "mypool");
        assert_eq!(s.filesystems.len(), 2);
        assert_eq!(s.filesystems[0].name, "data");
        assert_eq!(s.filesystems[1].name, "logs");
        assert!(s.filesystems[0].mount_block.is_some());
    } else {
        panic!("expected stratis");
    }
}

#[test]
fn validate_stratis_requires_disks() {
    let input = r#"
stratis nopool {
    encrypted = true

    filesystem fs0 {
        mount {
            target = /mnt/data
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "stratis without disks should fail");
}

// ─── Multipath ──────────────────────────────────────────────

#[test]
fn parse_multipath_basic() {
    let input = r#"
multipath san0 {
    wwid = "3600508b1001c5a7380000300000e0000"
    policy = round-robin

    path /dev/sda {
        priority = 1
    }

    path /dev/sdb {
        priority = 2
    }

    luks2 encrypted_san {
        cipher = aes-xts-plain64

        lvm vg_san {
            xfs san_data {
                size = remaining
                mount = /srv/san
            }
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 1);

    if let StorageDecl::Multipath(mp) = &ast.declarations[0] {
        assert_eq!(mp.name, "san0");
        assert_eq!(mp.paths.len(), 2);
        assert_eq!(mp.paths[0].device, "/dev/sda");
        assert_eq!(mp.children.len(), 1);
    } else {
        panic!("expected multipath");
    }
}

#[test]
fn validate_multipath_requires_wwid() {
    let input = r#"
multipath bad_mp {
    policy = round-robin

    path /dev/sda {}
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "multipath without wwid should fail");
}

#[test]
fn validate_multipath_unique_wwid() {
    let input = r#"
multipath mp0 {
    wwid = "3600508b1001c5a7380000300000e0000"
    path /dev/sda {}
}

multipath mp1 {
    wwid = "3600508b1001c5a7380000300000e0000"
    path /dev/sdb {}
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "duplicate wwid should fail");
}

// ─── iSCSI ──────────────────────────────────────────────────

#[test]
fn parse_iscsi_basic() {
    let input = r#"
iscsi remote_storage {
    target = iqn.2024.com.example:storage
    portal = 192.168.1.100
    auth = chap
    username = "iscsi_user"

    ext4 iscsi_data {
        mount = /mnt/iscsi [nodev, nosuid]
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 1);

    if let StorageDecl::Iscsi(iscsi) = &ast.declarations[0] {
        assert_eq!(iscsi.name, "remote_storage");
        assert_eq!(iscsi.children.len(), 1);
    } else {
        panic!("expected iscsi");
    }
}

#[test]
fn validate_iscsi_requires_target_and_portal() {
    let input = r#"
iscsi bad_iscsi {
    auth = chap
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "iscsi without target/portal should fail");
}

// ─── NFS ────────────────────────────────────────────────────

#[test]
fn parse_nfs_basic() {
    let input = r#"
nfs shared {
    server = nfs-server.example.com
    export = /exports/data
    version = 4.2

    mount {
        target = /mnt/nfs
        options = [nodev, nosuid, noexec]
        requires = [network-online.target]
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 1);

    if let StorageDecl::Nfs(nfs) = &ast.declarations[0] {
        assert_eq!(nfs.name, "shared");
        assert!(nfs.mount_block.is_some());
        let mb = nfs.mount_block.as_ref().unwrap();
        assert_eq!(mb.target.as_deref(), Some("/mnt/nfs"));
        assert_eq!(mb.requires.len(), 1);
    } else {
        panic!("expected nfs");
    }
}

#[test]
fn validate_nfs_requires_server_and_export() {
    let input = r#"
nfs bad_nfs {
    version = 4.2

    mount {
        target = /mnt/nfs
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "nfs without server/export should fail");
}

// ─── tmpfs ──────────────────────────────────────────────────

#[test]
fn parse_tmpfs_basic() {
    let input = r#"
tmpfs runtime {
    size = 2G
    mode = 1777

    mount {
        target = /tmp
        options = [nodev, nosuid, noexec]
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 1);

    if let StorageDecl::Tmpfs(t) = &ast.declarations[0] {
        assert_eq!(t.name, "runtime");
        assert!(t.mount_block.is_some());
    } else {
        panic!("expected tmpfs");
    }
}

#[test]
fn validate_tmpfs_requires_mount() {
    let input = r#"
tmpfs bad {
    size = 1G
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "tmpfs without mount should fail");
}

// ─── Integrity ──────────────────────────────────────────────

#[test]
fn parse_integrity_block() {
    let input = r#"
disk /dev/sda {
    label = gpt

    integrity secured {
        index = 1
        size = 500G
        algorithm = crc32c

        ext4 verified_data {
            mount = /srv/secure [nodev, nosuid]
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    if let StorageDecl::Disk(d) = &ast.declarations[0] {
        assert_eq!(d.children.len(), 1);
        if let PartitionChild::Integrity(i) = &d.children[0] {
            assert_eq!(i.name, "secured");
            assert_eq!(i.children.len(), 1);
        } else {
            panic!("expected integrity block");
        }
    }
}

// ─── VDO ────────────────────────────────────────────────────

#[test]
fn parse_vdo_in_lvm() {
    let input = r#"
disk /dev/sda {
    label = gpt

    lvm vg0 {
        index = 1
        size = remaining

        vdo dedup_pool {
            size = 100G
            virtual_size = 1T
            deduplication = true
            compression = true

            xfs dedup_data {
                mount = /srv/dedup
            }
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    if let StorageDecl::Disk(d) = &ast.declarations[0] {
        if let PartitionChild::Lvm(lvm) = &d.children[0] {
            assert_eq!(lvm.children.len(), 1);
            if let LvmChild::Vdo(v) = &lvm.children[0] {
                assert_eq!(v.name, "dedup_pool");
                assert_eq!(v.children.len(), 1);
            } else {
                panic!("expected vdo");
            }
        } else {
            panic!("expected lvm");
        }
    }
}

#[test]
fn validate_vdo_requires_size_and_virtual_size() {
    let input = r#"
disk /dev/sda {
    label = gpt

    lvm vg0 {
        index = 1
        size = remaining

        vdo bad_vdo {
            deduplication = true
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "vdo without size/virtual_size should fail");
}

#[test]
fn validate_vdo_virtual_size_gte_size() {
    let input = r#"
disk /dev/sda {
    label = gpt

    lvm vg0 {
        index = 1
        size = remaining

        vdo bad_vdo {
            size = 100G
            virtual_size = 50G
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "vdo virtual_size < size should fail");
}

// ─── SELinux System Block ───────────────────────────────────

#[test]
fn parse_selinux_block() {
    let input = r#"
selinux {
    mode = enforcing
    policy = targeted
    floor = standard

    user system_u {
        roles = [system_r, unconfined_r]
        level = s0-s0:c0.c1023
    }

    user staff_u {
        roles = [staff_r, sysadm_r]
        level = s0-s0:c0.c1023
        default = true
    }

    role sysadm_r {
        types = [sysadm_t]
        level = s0-s0:c0.c1023
    }

    booleans {
        httpd_can_network_connect = true
        container_manage_cgroup = true
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    assert!(ast.selinux.is_some());
    let se = ast.selinux.as_ref().unwrap();
    assert_eq!(se.users.len(), 2);
    assert_eq!(se.users[0].name, "system_u");
    assert_eq!(se.users[1].name, "staff_u");
    assert_eq!(se.roles.len(), 1);
    assert_eq!(se.roles[0].name, "sysadm_r");
    assert!(!se.booleans.is_empty());
}

#[test]
fn validate_selinux_invalid_mode() {
    let input = r#"
selinux {
    mode = invalid_mode
    policy = targeted

    user system_u {
        roles = [system_r]
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "invalid selinux mode should fail");
}

#[test]
fn validate_selinux_requires_system_u() {
    let input = r#"
selinux {
    mode = enforcing
    policy = targeted

    user staff_u {
        roles = [staff_r]
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "selinux without system_u should fail");
}

#[test]
fn validate_selinux_at_most_one_default_user() {
    let input = r#"
selinux {
    mode = enforcing
    policy = targeted

    user system_u {
        roles = [system_r]
        default = true
    }

    user staff_u {
        roles = [staff_r]
        default = true
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "multiple default users should fail");
}

#[test]
fn validate_selinux_user_requires_roles() {
    let input = r#"
selinux {
    mode = enforcing
    policy = targeted

    user system_u {
        level = s0
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    let result = validate_storage(&ast);
    assert!(result.is_err(), "selinux user without roles should fail");
}

// ─── Combined Scenario ──────────────────────────────────────

#[test]
fn parse_mixed_declarations() {
    let input = r#"
disk /dev/sda {
    label = gpt

    fat32 efi {
        index = 1
        size = 1G
        mount = /boot/efi
    }
}

nfs home_share {
    server = fileserver.local
    export = /exports/home
    version = 4.2

    mount {
        target = /home
        options = [nodev, nosuid]
    }
}

tmpfs tmp_area {
    size = 4G

    mount {
        target = /tmp
        options = [nodev, nosuid, noexec]
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 3);
    assert!(matches!(&ast.declarations[0], StorageDecl::Disk(_)));
    assert!(matches!(&ast.declarations[1], StorageDecl::Nfs(_)));
    assert!(matches!(&ast.declarations[2], StorageDecl::Tmpfs(_)));

    let warnings = validate_storage(&ast).expect("should validate");
    let _ = warnings;
}

#[test]
fn parse_zvol_with_luks() {
    let input = r#"
zpool secure_pool {
    vdev v0 {
        type = mirror
        members = [/dev/sda, /dev/sdb]
    }

    zvol encrypted_vol {
        size = 50G

        luks2 zvol_crypt {
            cipher = aes-xts-plain64

            ext4 secure_data {
                mount = /srv/secure
            }
        }
    }
}
"#;
    let ast = parse_storage(input).expect("should parse");
    if let StorageDecl::Zpool(zp) = &ast.declarations[0] {
        assert_eq!(zp.zvols.len(), 1);
        assert_eq!(zp.zvols[0].children.len(), 1);
        if let ZvolChild::Luks(l) = &zp.zvols[0].children[0] {
            assert_eq!(l.name, "zvol_crypt");
        } else {
            panic!("expected luks inside zvol");
        }
    }
}

// ─── Core Language Tests ────────────────────────────────────

#[test]
fn parse_class_basic() {
    let input = r#"
class hardened_base {
    selinux_mode = enforcing
    encryption = true
}
"#;
    let ast = parse_source(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 1);
    if let TopLevelDecl::Class(c) = &ast.declarations[0] {
        assert_eq!(c.name, "hardened_base");
        assert!(c.parent.is_none());
        assert_eq!(c.body.len(), 2);
    } else {
        panic!("expected class");
    }
}

#[test]
fn parse_class_extends() {
    let input = r#"
class base {
    mode = enforcing
}

class server extends base {
    packages = true
}
"#;
    let ast = parse_source(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 2);
    if let TopLevelDecl::Class(c) = &ast.declarations[1] {
        assert_eq!(c.name, "server");
        assert_eq!(c.parent.as_deref(), Some("base"));
    } else {
        panic!("expected class");
    }
}

#[test]
fn parse_system_basic() {
    let input = r#"
system web_server {
    hostname = web01
    role = production
}
"#;
    let ast = parse_source(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 1);
    if let TopLevelDecl::System(s) = &ast.declarations[0] {
        assert_eq!(s.name, "web_server");
        assert!(s.parent.is_none());
        assert_eq!(s.body.len(), 2);
    } else {
        panic!("expected system");
    }
}

#[test]
fn parse_system_extends() {
    let input = r#"
class base { mode = enforcing }
system prod_server extends base { hostname = web01 }
"#;
    let ast = parse_source(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 2);
    if let TopLevelDecl::System(s) = &ast.declarations[1] {
        assert_eq!(s.name, "prod_server");
        assert_eq!(s.parent.as_deref(), Some("base"));
    } else {
        panic!("expected system");
    }
}

#[test]
fn parse_var_decl() {
    let input = r#"
var root_size = 50G
var hostname = "web01"
var enable_ssh = true
"#;
    let ast = parse_source(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 3);
    if let TopLevelDecl::Var(v) = &ast.declarations[0] {
        assert_eq!(v.name, "root_size");
    } else {
        panic!("expected var");
    }
}

#[test]
fn parse_import_stmt() {
    let input = r#"
import "base/hardened.ic"
import "roles/webserver.ic"
"#;
    let ast = parse_source(input).expect("should parse");
    assert_eq!(ast.imports.len(), 2);
    assert_eq!(ast.imports[0].path, "base/hardened.ic");
    assert_eq!(ast.imports[1].path, "roles/webserver.ic");
}

#[test]
fn parse_apply_stmt() {
    let input = r#"
class base { mode = enforcing }
system myserver {
    apply base
    hostname = web01
}
"#;
    let ast = parse_source(input).expect("should parse");
    if let TopLevelDecl::System(s) = &ast.declarations[1] {
        assert_eq!(s.body.len(), 2);
        if let ClassBodyItem::Apply(a) = &s.body[0] {
            assert_eq!(a.class_name, "base");
        } else {
            panic!("expected apply");
        }
    } else {
        panic!("expected system");
    }
}

#[test]
fn parse_if_block() {
    let input = r#"
class configurable {
    if production {
        replicas = 3
    } elif staging {
        replicas = 2
    } else {
        replicas = 1
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    if let TopLevelDecl::Class(c) = &ast.declarations[0] {
        assert_eq!(c.body.len(), 1);
        if let ClassBodyItem::If(ifb) = &c.body[0] {
            assert_eq!(ifb.condition, "production");
            assert_eq!(ifb.elif_branches.len(), 1);
            assert_eq!(ifb.elif_branches[0].condition, "staging");
            assert!(ifb.else_body.is_some());
        } else {
            panic!("expected if block");
        }
    } else {
        panic!("expected class");
    }
}

#[test]
fn parse_for_block() {
    let input = r#"
class multi_disk {
    for dev in disks {
        label = gpt
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    if let TopLevelDecl::Class(c) = &ast.declarations[0] {
        assert_eq!(c.body.len(), 1);
        if let ClassBodyItem::For(fb) = &c.body[0] {
            assert_eq!(fb.var_name, "dev");
            assert_eq!(fb.iterable, "disks");
            assert_eq!(fb.body.len(), 1);
        } else {
            panic!("expected for block");
        }
    } else {
        panic!("expected class");
    }
}

#[test]
fn parse_class_with_storage() {
    let input = r#"
class storage_base {
    disk /dev/sda {
        label = gpt
        ext4 root {
            index = 1
            size = 50G
            mount = /
        }
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    if let TopLevelDecl::Class(c) = &ast.declarations[0] {
        assert_eq!(c.body.len(), 1);
        if let ClassBodyItem::Domain(d) = &c.body[0] {
            assert!(matches!(**d, TopLevelDecl::Storage(_)));
        } else {
            panic!("expected domain block");
        }
    } else {
        panic!("expected class");
    }
}

// ─── Firewall Domain Tests ──────────────────────────────────

#[test]
fn parse_firewall_basic() {
    let input = r#"
firewall {
    table inet filter {
        chain input {
            type = filter
            hook = input
            priority = 0
            policy = drop

            rule allow_established {
                match {
                    ct_state = established
                }
                action = accept
            }

            rule allow_ssh {
                match {
                    protocol = tcp
                    dport = 22
                }
                action = accept
            }
        }
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 1);
    if let TopLevelDecl::Firewall(fw) = &ast.declarations[0] {
        assert_eq!(fw.tables.len(), 1);
        assert_eq!(fw.tables[0].family, "inet");
        assert_eq!(fw.tables[0].name, "filter");
        assert_eq!(fw.tables[0].chains.len(), 1);
        assert_eq!(fw.tables[0].chains[0].name, "input");
        assert_eq!(fw.tables[0].chains[0].rules.len(), 2);
        assert_eq!(fw.tables[0].chains[0].rules[0].name, "allow_established");
        assert_eq!(fw.tables[0].chains[0].rules[0].matches.len(), 1);
    } else {
        panic!("expected firewall");
    }
}

// ─── Network Domain Tests ───────────────────────────────────

#[test]
fn parse_network_basic() {
    let input = r#"
network {
    backend = networkmanager

    interface eth0 {
        type = ethernet

        ip {
            address = "10.0.0.10/24"
            gateway = "10.0.0.1"
        }
    }

    dns {
        servers = ["8.8.8.8", "8.8.4.4"]
    }

    routes {
        route vpn_subnet {
            destination = "192.168.100.0/24"
            gateway = "10.0.0.254"
        }
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    if let TopLevelDecl::Network(net) = &ast.declarations[0] {
        assert_eq!(net.interfaces.len(), 1);
        assert_eq!(net.interfaces[0].name, "eth0");
        assert!(net.interfaces[0].ip.is_some());
        assert!(net.dns.is_some());
        assert!(net.routes.is_some());
        assert_eq!(net.routes.as_ref().unwrap().routes.len(), 1);
    } else {
        panic!("expected network");
    }
}

// ─── Packages Domain Tests ──────────────────────────────────

#[test]
fn parse_packages_basic() {
    let input = r#"
packages {
    repo baseos {
        name = "BaseOS"
        baseurl = "https://mirror.example.com/baseos"
        gpgcheck = true
    }

    pkg httpd {
        version = "2.4.57"
        state = present
    }

    pkg telnet {
        state = absent
    }

    group "Development Tools" {
        state = present
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    if let TopLevelDecl::Packages(pkgs) = &ast.declarations[0] {
        assert_eq!(pkgs.repos.len(), 1);
        assert_eq!(pkgs.repos[0].name, "baseos");
        assert_eq!(pkgs.packages.len(), 2);
        assert_eq!(pkgs.packages[0].name, "httpd");
        assert_eq!(pkgs.groups.len(), 1);
        assert_eq!(pkgs.groups[0].name, "Development Tools");
    } else {
        panic!("expected packages");
    }
}

// ─── Users Domain Tests ─────────────────────────────────────

#[test]
fn parse_users_basic() {
    let input = r#"
users {
    policy {
        complexity {
            min_length = 12
            require_uppercase = true
        }

        lockout {
            attempts = 5
            lockout_time = 900
        }
    }

    user admin {
        uid = 1000
        groups = [wheel, sudo]
        shell = /bin/bash
        home = /home/admin
    }

    group developers {
        gid = 2000
        members = [admin]
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    if let TopLevelDecl::Users(u) = &ast.declarations[0] {
        assert!(u.policy.is_some());
        let policy = u.policy.as_ref().unwrap();
        assert!(policy.complexity.is_some());
        assert!(policy.lockout.is_some());
        assert_eq!(u.users.len(), 1);
        assert_eq!(u.users[0].name, "admin");
        assert_eq!(u.groups.len(), 1);
        assert_eq!(u.groups[0].name, "developers");
    } else {
        panic!("expected users");
    }
}

// ─── Init / Services Domain Tests ───────────────────────────

#[test]
fn parse_init_systemd() {
    let input = r#"
init systemd {
    service sshd {
        type = notify
        exec_start = /usr/sbin/sshd
        enabled = true

        hardening {
            protect_system = strict
            no_new_privileges = true
        }
    }

    service chronyd {
        type = forking
        exec_start = /usr/sbin/chronyd
        enabled = true
    }

    timer backup {
        on_calendar = "daily"
        persistent = true
    }

    defaults {
        restart = on-failure
        restart_sec = 5
    }

    journal {
        storage = persistent
        max_use = 500M
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    if let TopLevelDecl::Init(init) = &ast.declarations[0] {
        assert_eq!(init.backend, "systemd");
        assert_eq!(init.services.len(), 2);
        assert_eq!(init.services[0].name, "sshd");
        assert!(init.services[0].hardening.is_some());
        assert_eq!(init.timers.len(), 1);
        assert_eq!(init.timers[0].name, "backup");
        assert!(init.defaults.is_some());
        assert!(init.journal.is_some());
    } else {
        panic!("expected init");
    }
}

// ─── Users Validation Tests ─────────────────────────────────

#[test]
fn validate_users_accepts_valid_config() {
    let input = r#"
users {
    policy {
        complexity {
            min_length = 12
            require_uppercase = true
        }
        lockout {
            attempts = 5
            lockout_time = 900
        }
    }

    user admin {
        uid = 1000
        groups = [wheel, sudo]
        shell = /bin/bash
    }

    user deployer {
        uid = 1001
        groups = [webops]
        shell = /bin/bash
    }

    group webops {
        gid = 2000
    }

    group developers {
        gid = 2001
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    let warnings = validate(&ast).expect("should validate");
    let _ = warnings;
}

#[test]
fn validate_users_rejects_duplicate_usernames() {
    let input = r#"
users {
    user admin {
        uid = 1000
    }

    user admin {
        uid = 1001
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    let result = validate(&ast);
    assert!(result.is_err(), "duplicate user names should fail");
}

#[test]
fn validate_users_rejects_duplicate_uid() {
    let input = r#"
users {
    user alice {
        uid = 1001
    }

    user bob {
        uid = 1001
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    let result = validate(&ast);
    assert!(result.is_err(), "duplicate UIDs should fail");
}

#[test]
fn validate_users_rejects_negative_uid() {
    let input = r#"
users {
    user baduser {
        uid = -1
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    let result = validate(&ast);
    assert!(result.is_err(), "negative uid should fail");
}

#[test]
fn validate_users_rejects_duplicate_group_names() {
    let input = r#"
users {
    group devs {
        gid = 2000
    }

    group devs {
        gid = 2001
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    let result = validate(&ast);
    assert!(result.is_err(), "duplicate group names should fail");
}

#[test]
fn validate_users_rejects_duplicate_gid() {
    let input = r#"
users {
    group alpha {
        gid = 2000
    }

    group beta {
        gid = 2000
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    let result = validate(&ast);
    assert!(result.is_err(), "duplicate GIDs should fail");
}

#[test]
fn validate_users_rejects_invalid_min_length() {
    let input = r#"
users {
    policy {
        complexity {
            min_length = 0
        }
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    let result = validate(&ast);
    assert!(result.is_err(), "min_length=0 should fail");
}

#[test]
fn validate_users_rejects_invalid_lockout_attempts() {
    let input = r#"
users {
    policy {
        lockout {
            attempts = 0
            lockout_time = 900
        }
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    let result = validate(&ast);
    assert!(result.is_err(), "attempts=0 should fail");
}

// ─── Packages Validation Tests ──────────────────────────────

#[test]
fn validate_packages_accepts_valid_config() {
    let input = r#"
packages {
    repo baseos {
        name = "BaseOS"
        baseurl = "https://mirror.example.com/baseos"
        gpgcheck = true
    }

    pkg httpd {
        version = "2.4.57"
        state = present
    }

    pkg telnet {
        state = absent
    }

    group "Development Tools" {
        state = present
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    let warnings = validate(&ast).expect("should validate");
    let _ = warnings;
}

#[test]
fn validate_packages_rejects_duplicate_repo() {
    let input = r#"
packages {
    repo baseos {
        baseurl = "https://mirror1.example.com/baseos"
        gpgcheck = true
    }

    repo baseos {
        baseurl = "https://mirror2.example.com/baseos"
        gpgcheck = true
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    let result = validate(&ast);
    assert!(result.is_err(), "duplicate repo names should fail");
}

#[test]
fn validate_packages_rejects_repo_without_baseurl() {
    let input = r#"
packages {
    repo orphan {
        name = "Orphan Repo"
        gpgcheck = true
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    let result = validate(&ast);
    assert!(
        result.is_err(),
        "repo without baseurl or metalink should fail"
    );
}

#[test]
fn validate_packages_rejects_duplicate_pkg() {
    let input = r#"
packages {
    pkg httpd {
        state = present
    }

    pkg httpd {
        state = latest
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    let result = validate(&ast);
    assert!(result.is_err(), "duplicate package names should fail");
}

#[test]
fn validate_packages_rejects_invalid_state() {
    let input = r#"
packages {
    pkg badpkg {
        state = removed
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    let result = validate(&ast);
    assert!(result.is_err(), "invalid package state should fail");
}

#[test]
fn validate_packages_warns_missing_gpgcheck() {
    let input = r#"
packages {
    repo norepo {
        baseurl = "https://example.com/repo"
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    let warnings = validate(&ast).expect("should validate (warning only)");
    assert!(
        !warnings.is_empty(),
        "missing gpgcheck should produce a warning"
    );
}

// ─── Full System Test ───────────────────────────────────────

#[test]
fn parse_full_system() {
    let input = r#"
import "base/hardened.ic"

var env = production

class hardened_base {
    selinux_mode = enforcing
}

system web_server extends hardened_base {
    disk /dev/sda {
        label = gpt
        ext4 root { index = 1; size = 50G; mount = / }
    }

    firewall {
        table inet filter {
            chain input {
                policy = drop
                rule allow_ssh {
                    match { protocol = tcp; dport = 22 }
                    action = accept
                }
            }
        }
    }

    network {
        interface eth0 {
            type = ethernet
            ip { address = "10.0.0.10/24" }
        }
    }

    packages {
        pkg httpd { state = present }
        pkg nginx { state = present }
    }

    users {
        user webadmin {
            uid = 1001
            groups = [wheel]
        }
    }

    init systemd {
        service httpd {
            type = notify
            exec_start = /usr/sbin/httpd
            enabled = true
        }
    }
}
"#;
    let ast = parse_source(input).expect("should parse");
    assert_eq!(ast.imports.len(), 1);
    assert_eq!(ast.declarations.len(), 3); // var, class, system

    if let TopLevelDecl::System(sys) = &ast.declarations[2] {
        assert_eq!(sys.name, "web_server");
        assert_eq!(sys.parent.as_deref(), Some("hardened_base"));
        // Count domain blocks inside the system
        let domain_count = sys
            .body
            .iter()
            .filter(|b| matches!(b, ClassBodyItem::Domain(_)))
            .count();
        assert_eq!(domain_count, 6); // disk, firewall, network, packages, users, init
    } else {
        panic!("expected system");
    }
}

#[test]
fn parse_class_inheritance_chain() {
    let input = r#"
class base {
    security = high
}

class server extends base {
    role = server
}

system prod extends server {
    hostname = prod01
}
"#;
    let ast = parse_source(input).expect("should parse");
    assert_eq!(ast.declarations.len(), 3);
    if let TopLevelDecl::Class(c) = &ast.declarations[0] {
        assert_eq!(c.name, "base");
        assert!(c.parent.is_none());
    } else {
        panic!("expected class");
    }
    if let TopLevelDecl::Class(c) = &ast.declarations[1] {
        assert_eq!(c.name, "server");
        assert_eq!(c.parent.as_deref(), Some("base"));
    } else {
        panic!("expected class");
    }
    if let TopLevelDecl::System(s) = &ast.declarations[2] {
        assert_eq!(s.name, "prod");
        assert_eq!(s.parent.as_deref(), Some("server"));
    } else {
        panic!("expected system");
    }
}

// ─── Display Tests ──────────────────────────────────────────

use ironclad_diagnostics::Span;

fn dummy_span() -> Span {
    Span {
        start: 0,
        end: 0,
        line: 1,
        col: 1,
    }
}

#[test]
fn display_value_string() {
    assert_eq!(Value::String("hello".into()).to_string(), r#""hello""#);
    assert_eq!(
        Value::String(r#"say "hi""#.into()).to_string(),
        r#""say \"hi\"""#
    );
    assert_eq!(
        Value::String("back\\slash".into()).to_string(),
        r#""back\\slash""#
    );
}

#[test]
fn display_value_integer() {
    assert_eq!(Value::Integer(42).to_string(), "42");
    assert_eq!(Value::Integer(-1).to_string(), "-1");
    assert_eq!(Value::Integer(0).to_string(), "0");
}

#[test]
fn display_value_boolean() {
    assert_eq!(Value::Boolean(true).to_string(), "true");
    assert_eq!(Value::Boolean(false).to_string(), "false");
}

#[test]
fn display_value_size() {
    let sv = SizeValue {
        amount: 20,
        unit: SizeUnit::G,
    };
    assert_eq!(Value::Size(sv).to_string(), "20G");

    let sv = SizeValue {
        amount: 512,
        unit: SizeUnit::M,
    };
    assert_eq!(Value::Size(sv).to_string(), "512M");

    let sv = SizeValue {
        amount: 4,
        unit: SizeUnit::K,
    };
    assert_eq!(Value::Size(sv).to_string(), "4K");

    let sv = SizeValue {
        amount: 1,
        unit: SizeUnit::T,
    };
    assert_eq!(Value::Size(sv).to_string(), "1T");

    let sv = SizeValue {
        amount: 4096,
        unit: SizeUnit::B,
    };
    assert_eq!(Value::Size(sv).to_string(), "4096B");
}

#[test]
fn display_value_percentage() {
    assert_eq!(Value::Percentage(50).to_string(), "50%");
    assert_eq!(Value::Percentage(100).to_string(), "100%");
}

#[test]
fn display_value_remaining() {
    assert_eq!(Value::Remaining.to_string(), "remaining");
}

#[test]
fn display_value_array() {
    let arr = Value::Array(vec![
        Value::Integer(1),
        Value::Integer(2),
        Value::Integer(3),
    ]);
    assert_eq!(arr.to_string(), "[1, 2, 3]");

    let arr = Value::Array(vec![Value::String("a".into()), Value::String("b".into())]);
    assert_eq!(arr.to_string(), r#"["a", "b"]"#);

    assert_eq!(Value::Array(vec![]).to_string(), "[]");
}

#[test]
fn display_value_ident() {
    assert_eq!(Value::Ident("enforcing".into()).to_string(), "enforcing");
}

#[test]
fn display_value_path() {
    assert_eq!(Value::Path("/etc/hosts".into()).to_string(), "/etc/hosts");
    assert_eq!(
        Value::DevicePath("/dev/sda1".into()).to_string(),
        "/dev/sda1"
    );
}

#[test]
fn display_value_url() {
    assert_eq!(
        Value::Url("https://example.com/repo".into()).to_string(),
        "https://example.com/repo"
    );
}

#[test]
fn display_property() {
    let prop = Property {
        key: "size".into(),
        value: Value::Size(SizeValue {
            amount: 20,
            unit: SizeUnit::G,
        }),
        span: dummy_span(),
    };
    assert_eq!(prop.to_string(), "size = 20G");

    let prop = Property {
        key: "label".into(),
        value: Value::Ident("gpt".into()),
        span: dummy_span(),
    };
    assert_eq!(prop.to_string(), "label = gpt");
}

#[test]
fn display_selinux_context() {
    let ctx = SelinuxContext {
        user: "system_u".into(),
        role: "object_r".into(),
        typ: "httpd_sys_content_t".into(),
        range: MlsRange {
            low: Sensitivity { level: 0 },
            high: None,
            categories: None,
        },
        raw: "system_u:object_r:httpd_sys_content_t:s0".into(),
    };
    assert_eq!(ctx.to_string(), "system_u:object_r:httpd_sys_content_t:s0");
}

#[test]
fn display_mount_expr() {
    let mount = MountExpr {
        target: "/boot".into(),
        options: vec![],
        context: None,
    };
    assert_eq!(mount.to_string(), "/boot");

    let mount = MountExpr {
        target: "/boot/efi".into(),
        options: vec!["nodev".into(), "nosuid".into(), "noexec".into()],
        context: None,
    };
    assert_eq!(mount.to_string(), "/boot/efi [nodev, nosuid, noexec]");

    let ctx = SelinuxContext {
        user: "system_u".into(),
        role: "object_r".into(),
        typ: "boot_t".into(),
        range: MlsRange {
            low: Sensitivity { level: 0 },
            high: None,
            categories: None,
        },
        raw: "system_u:object_r:boot_t:s0".into(),
    };
    let mount = MountExpr {
        target: "/boot".into(),
        options: vec!["ro".into()],
        context: Some(ctx),
    };
    assert_eq!(mount.to_string(), "/boot [ro] system_u:object_r:boot_t:s0");
}
