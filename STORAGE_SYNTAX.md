# Ironclad Storage Syntax Specification

**Status:** Draft — syntax development, Phase 1  
**Scope:** Disk, partitioning, encryption, volume management, filesystems, and mounts

---

## Design Principles

Ironclad's storage syntax is a thin, structured wrapper over Linux storage tooling. Every block in the syntax maps to a real tool invocation — `parted`, `cryptsetup`, `pvcreate`/`lvcreate`, `mkfs.*`, `mount` — but the compiler handles ordering, validation, and interdependency resolution rather than the operator.

The key ergonomic properties:

1. **Nesting mirrors the real storage stack.** A filesystem inside an LVM volume inside a LUKS container inside a partition is written as exactly that nesting. The code reads like the actual dependency chain.

2. **Implicit ordering from structure.** The compiler topologically sorts operations from the declared hierarchy. The operator never specifies execution order.

3. **Named references.** Every named block becomes a referenceable identifier throughout the Ironclad source tree. A LUKS container named `system` can be referenced by key management declarations, clevis bindings, or firewall rules elsewhere.

4. **Validation from structure.** The compiler rejects impossible configurations at compile time — overlapping partitions, filesystems without backing devices, mount points referencing undeclared filesystems, LVM logical volumes exceeding volume group capacity, thin pool overcommit beyond configurable thresholds.

5. **Defaults that disappear the obvious.** When a default exists that a competent administrator would choose in nearly all cases, the language assumes it. Explicit declaration overrides any default. Every default is documented.

---

## Top-Level Blocks

Storage declarations begin with a top-level block that represents a physical or virtual block device.

### `disk`

Declares a physical block device and its partition table.

```
disk /dev/sda {
    label = gpt
}
```

**Required properties:**

| Property | Type | Description |
|----------|------|-------------|
| `label` | `gpt` \| `msdos` \| `none` | Partition table type. `none` indicates a whole-disk device with no partition table. |

**Optional properties:**

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `sector_size` | integer | auto-detected | Override logical sector size (bytes). Relevant for 4Kn drives. |

**Children:** Partition blocks (filesystem type keywords, `luks2`, or `raw`) when `label` is `gpt` or `msdos`. A single filesystem block when `label` is `none`.

**Compiler behavior:** Emits `parted mklabel <label>` or skips partitioning entirely when `label = none`.

---

### `mdraid`

Declares a Linux software RAID array. Treated as a virtual block device — its children follow the same rules as `disk` children.

```
mdraid md0 {
    level = 10
    disks = [/dev/sdd, /dev/sde, /dev/sdf, /dev/sdg]
}
```

**Required properties:**

| Property | Type | Description |
|----------|------|-------------|
| `level` | `0` \| `1` \| `5` \| `6` \| `10` | RAID level. |
| `disks` | array of device paths | Member devices. |

**Optional properties:**

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `spare` | array of device paths | none | Hot spare devices. |
| `chunk` | size string | kernel default | Chunk size for striped levels (`64K`, `512K`, etc.). |
| `bitmap` | `internal` \| `none` \| path | `internal` | Write-intent bitmap location. |
| `metadata` | `1.0` \| `1.1` \| `1.2` | `1.2` | Metadata version. `1.0` stores metadata at end of device (required for boot arrays on some bootloaders). |
| `layout` | string | kernel default | RAID layout. Level-specific (e.g., `f2` for RAID10 far layout). |
| `name` | string | array name | Human-readable name written to metadata. |

**Children:** Same as `disk` — filesystem blocks, `luks2`, `lvm`, or `raw`.

**Compiler behavior:** Emits `mdadm --create /dev/md/<name>` with the specified parameters. Member devices must either be raw partitions declared elsewhere in the source tree (compiler validates their existence) or assumed-present paths for pre-existing hardware.

---

## Partition-Level Blocks

Direct children of a `disk` block represent partitions. The block keyword is the filesystem type that will be created on the partition, or a structural keyword (`luks2`, `raw`).

### Common Partition Properties

Every direct child of a `disk` block accepts these properties:

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `index` | integer | declaration order | Partition number in the table. Explicit when order matters for bootloader compatibility; otherwise inferred from source order. |
| `size` | size string | required unless `start`/`end` given | Partition size. Accepts `1G`, `500M`, `50%`, `remaining`. |
| `start` | size string | auto-calculated | Explicit start offset from beginning of disk. Not recommended — prefer `size` and let the compiler calculate. |
| `end` | size string | auto-calculated | Explicit end offset. `-1` means end of disk. |
| `align` | size string | optimal for device | Alignment boundary. Override only when you know why. |
| `type` | string | inferred from context | Partition type GUID (GPT) or partition ID (MBR). Common values: `ef00` (EFI System), `ef02` (BIOS boot), `8300` (Linux filesystem), `8200` (Linux swap), `8309` (Linux LUKS), `8e00` (Linux LVM). When omitted, the compiler infers from the block type — `fat32` as a first partition infers `ef00`; `swap` infers `8200`; `luks2` infers `8309`. |

**Size strings:** A number followed by a unit. Valid units: `B`, `K`, `KB`, `M`, `MB`, `G`, `GB`, `T`, `TB`. `%` indicates percentage of the parent container's available space. `remaining` consumes all unallocated space in the parent. Only one `remaining` is permitted per parent scope.

---

### `raw`

A partition with no filesystem. Used for BIOS boot partitions, reserved regions, or partitions managed by external tooling.

```
raw bios_boot {
    index = 1
    size = 1M
    type = ef02
}
```

**Compiler behavior:** Emits `parted mkpart` with the specified boundaries. No `mkfs` or `mount` is generated.

---

## Filesystem Type Keywords

Filesystem type keywords serve as block identifiers that simultaneously declare the partition (when inside `disk`) or logical volume (when inside `lvm`) **and** the filesystem to create on it. The keyword determines which `mkfs` variant the compiler emits.

### `ext4`

```
ext4 boot {
    size = 1G
    mount = /boot [nodev, nosuid, noexec]
}
```

**Optional properties:**

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `label` | string | block name | Filesystem label (`mkfs.ext4 -L`). |
| `block_size` | integer | 4096 | Block size in bytes (`mkfs.ext4 -b`). |
| `reserved_blocks` | percentage | `5%` | Reserved block percentage (`tune2fs -m`). |
| `features` | array of strings | mkfs defaults | Feature flags (`mkfs.ext4 -O`). e.g., `[metadata_csum, 64bit]`. |
| `inode_size` | integer | 256 | Inode size in bytes (`mkfs.ext4 -I`). |
| `inode_ratio` | integer | mkfs default | Bytes-per-inode ratio (`mkfs.ext4 -i`). Controls inode density. |
| `journal` | `true` \| `false` \| size string | `true` | External journal size or disable. |
| `stride` | integer | none | RAID stride in filesystem blocks (`mkfs.ext4 -E stride=`). |
| `stripe_width` | integer | none | RAID stripe width in filesystem blocks (`mkfs.ext4 -E stripe-width=`). |
| `mount` | mount expression | none | Mount target and options. See Mount Expressions. |

**Compiler behavior:** Emits `mkfs.ext4` with mapped flags. When `reserved_blocks` differs from default, emits a follow-up `tune2fs -m` call.

---

### `xfs`

```
xfs data {
    size = 500G
    mount = /srv/data
}
```

**Optional properties:**

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `label` | string | block name | Filesystem label (`mkfs.xfs -L`). |
| `block_size` | integer | 4096 | Block size (`mkfs.xfs -b size=`). |
| `su` | size string | none | Stripe unit (`mkfs.xfs -d su=`). For hardware or software RAID alignment. |
| `sw` | integer | none | Stripe width — number of data disks (`mkfs.xfs -d sw=`). |
| `log_size` | size string | auto | Internal log size (`mkfs.xfs -l size=`). |
| `log_device` | device path | none | External log device (`mkfs.xfs -l logdev=`). |
| `reflink` | `true` \| `false` | `true` | Enable reflink support (`mkfs.xfs -m reflink=`). |
| `bigtime` | `true` \| `false` | `true` | Timestamps beyond 2038 (`mkfs.xfs -m bigtime=`). |
| `mount` | mount expression | none | Mount target and options. |

**Compiler behavior:** Emits `mkfs.xfs` with mapped flags.

---

### `btrfs`

Btrfs blocks can contain `subvol` children declaring named subvolumes.

```
btrfs root {
    size = remaining
    compress = zstd:1
    
    subvol @ {
        mount = /
    }
    
    subvol @home {
        mount = /home [nodev, nosuid, compress=zstd:3]
        quota = 50G
    }
}
```

**Optional properties:**

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `label` | string | block name | Filesystem label (`mkfs.btrfs -L`). |
| `features` | array of strings | mkfs defaults | Feature flags (`mkfs.btrfs -O`). e.g., `[quota, free-space-tree]`. |
| `compress` | string | none | Default compression algorithm and level. Applied as mount option. Valid: `zstd`, `zstd:<level>`, `lzo`, `zlib`, `zlib:<level>`. |
| `node_size` | size string | 16K | Metadata node size (`mkfs.btrfs -n`). |
| `sector_size` | integer | auto | Sector size (`mkfs.btrfs -s`). |
| `data_profile` | `single` \| `dup` \| `raid0` \| `raid1` \| `raid1c3` \| `raid1c4` \| `raid10` \| `raid5` \| `raid6` | `single` | Data block group profile (`mkfs.btrfs -d`). |
| `metadata_profile` | same as `data_profile` | `dup` | Metadata block group profile (`mkfs.btrfs -m`). |
| `mount` | mount expression | none | Mount target for the filesystem root (rarely used directly — prefer `subvol` mounts). |

**Children:** `subvol` blocks.

**Compiler behavior:** Emits `mkfs.btrfs`, then `btrfs subvolume create` for each declared subvolume. Mounts the filesystem temporarily to a staging path to create subvolumes, then unmounts and remounts each subvolume at its declared mount point.

---

#### `subvol`

A Btrfs subvolume. Only valid inside a `btrfs` block. The name after the keyword is the actual subvolume name passed to `btrfs subvolume create`.

```
subvol @var_log {
    mount = /var/log [nodev, nosuid, noexec]
    quota = 10G
}
```

**Optional properties:**

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `mount` | mount expression | none | Mount target and options. The `subvol=<name>` mount option is automatically appended by the compiler. |
| `quota` | size string | none | Btrfs qgroup limit for this subvolume. Requires `quota` in parent's `features`. |
| `compress` | string | inherited from parent | Override parent compression for this subvolume (applied as mount option). |

**Compiler behavior:** Emits `btrfs subvolume create <parent_mount>/<name>`. If `quota` is set, emits `btrfs qgroup limit <size> <parent_mount>/<name>`.

---

### `fat32`

Used primarily for EFI System Partitions.

```
fat32 efi {
    size = 1G
    type = ef00
    mount = /boot/efi [nodev, nosuid, noexec]
}
```

**Optional properties:**

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `label` | string | block name (uppercased, truncated to 11 chars) | Volume label (`mkfs.fat -n`). |
| `fat_size` | `12` \| `16` \| `32` | `32` | FAT variant (`mkfs.fat -F`). |
| `cluster_size` | integer | auto | Cluster size in bytes (`mkfs.fat -s`). |
| `mount` | mount expression | none | Mount target and options. |

**Compiler behavior:** Emits `mkfs.fat -F 32` (or specified variant).

---

### `swap`

Swap is not a filesystem — it uses `mkswap` rather than any `mkfs` variant. It is its own keyword to reflect this.

```
swap swap0 {
    size = 32G
}
```

**Optional properties:**

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `label` | string | block name | Swap label (`mkswap -L`). |
| `priority` | integer | none | Swap priority for `swapon -p` and fstab. Higher values = preferred. |
| `discard` | `true` \| `false` | `false` | Enable discard/TRIM on swap (`swapon -d`). |
| `page_size` | integer | system default | Override page size (`mkswap -p`). Rare. |

**Compiler behavior:** Emits `mkswap`. No `mount` — the compiler generates the fstab swap entry and `swapon` invocation automatically.

---

### `ntfs`

Included for dual-boot and data exchange scenarios.

```
ntfs shared {
    size = 100G
    label = "SHARED"
    mount = /mnt/shared [nodev, nosuid, noexec]
}
```

**Optional properties:**

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `label` | string | block name | Volume label (`mkfs.ntfs -L`). |
| `compression` | `true` \| `false` | `false` | Enable NTFS compression. |
| `quick` | `true` \| `false` | `true` | Quick format — skip full surface scan (`mkfs.ntfs -Q`). |
| `mount` | mount expression | none | Mount target and options. |

**Compiler behavior:** Emits `mkfs.ntfs`. Requires `ntfs-3g` in the image package list.

---

## Encryption Blocks

### `luks2`

Declares a LUKS2 encrypted container. Can wrap filesystems directly, or contain an `lvm` block for volume management inside the encrypted layer.

```
luks2 system {
    index = 2
    size = remaining
    type = 8309
    cipher = aes-xts-plain64
    key_size = 512
    
    lvm vg0 {
        ext4 root { size = 50G; mount = / }
    }
}
```

**Optional properties:**

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `cipher` | string | `aes-xts-plain64` | Encryption cipher (`cryptsetup luksFormat --cipher`). |
| `key_size` | integer | `512` | Key size in bits (`cryptsetup luksFormat --key-size`). |
| `hash` | string | `sha512` | PBKDF hash algorithm (`--hash`). |
| `pbkdf` | string | `argon2id` | Key derivation function (`--pbkdf`). |
| `iter_time` | integer | `5000` | PBKDF iteration time in milliseconds (`--iter-time`). |
| `sector_size` | integer | `4096` | Encryption sector size (`--sector-size`). |
| `integrity` | string | none | dm-integrity algorithm (`--integrity`). e.g., `hmac-sha256`. Enables authenticated encryption. Significant write performance cost. |
| `tpm2` | `true` \| `false` | `false` | Bind key to TPM 2.0 via Clevis (`clevis luks bind tpm2`). |
| `tang` | URL string | none | Bind key to a Tang server via Clevis (`clevis luks bind tang`). Can coexist with `tpm2` for Shamir Secret Sharing (SSS) policy. |
| `tang_thp` | string | none | Tang server thumbprint for offline provisioning without trust-on-first-use. |
| `header` | path string | none | Detached LUKS header location. Stores the LUKS header on a separate device or file, leaving the data partition with no visible encryption metadata. |
| `label` | string | block name | LUKS label (`--label`). |

**Children:** A single filesystem block (direct encryption of one filesystem), or an `lvm` block (encryption wrapping a volume group), or another structural block.

**Compiler behavior:** Emits `cryptsetup luksFormat` with mapped flags, followed by `cryptsetup open`. If `tpm2` or `tang` is set, emits the corresponding `clevis luks bind` commands. The opened device name is derived from the block name (`/dev/mapper/<name>`).

---

### `luks1`

Provided for compatibility with systems that require LUKS1 (e.g., GRUB2 boot partition encryption on older configurations). Properties mirror `luks2` except `integrity` and `sector_size` are not available.

```
luks1 legacy_boot {
    index = 2
    size = 1G
    cipher = aes-xts-plain64
    key_size = 256
}
```

**Compiler behavior:** Emits `cryptsetup luksFormat --type luks1`.

---

## Volume Management Blocks

### `lvm`

Declares an LVM volume group. Contains logical volume children (filesystem or swap blocks) and optionally `thin` pool blocks.

```
lvm vg0 {
    ext4 root { size = 50G; mount = / }
    swap swap0 { size = 16G }
    
    thin pool0 {
        size = 200G
        
        xfs data { size = 300G; mount = /srv }
    }
    
    xfs scratch { size = remaining; mount = /tmp }
}
```

**Optional properties:**

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `pe_size` | size string | `4M` | Physical extent size (`vgcreate -s`). |
| `max_lv` | integer | unlimited | Maximum logical volumes (`vgcreate -l`). |
| `clustered` | `true` \| `false` | `false` | Clustered volume group flag. |
| `tags` | array of strings | none | LVM tags applied to the VG. |

**Children:** Filesystem blocks (thick logical volumes), `swap` blocks, and `thin` pool blocks. Direct children of `lvm` are standard (thick) logical volumes. Their `size` is physically allocated.

**Compiler behavior:** Emits `pvcreate` on the parent block device, `vgcreate <name>`, then `lvcreate` for each child volume in declaration order. The volume is named `/dev/<vg_name>/<lv_name>` where `lv_name` is the child block's name.

---

#### `thin`

A thin provisioning pool inside an LVM volume group. Only valid inside an `lvm` block. Children are thin logical volumes — their `size` is a virtual size that can exceed the pool's physical allocation (overprovisioning).

```
thin pool0 {
    size = 200G
    chunk_size = 64K
    
    xfs data { size = 300G; mount = /srv }
    ext4 containers { size = 150G; mount = /var/lib/containers }
}
```

**Optional properties:**

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `size` | size string | required | Physical size of the thin pool. |
| `chunk_size` | size string | auto | Thin pool chunk size (`lvcreate --chunksize`). Smaller = finer allocation granularity, larger = better sequential performance. |
| `metadata_size` | size string | auto | Metadata LV size. Rarely needs manual specification. |
| `zero` | `true` \| `false` | `true` | Zero new allocations (`--zero y/n`). |
| `discard` | `true` \| `false` | `true` | Support discard/TRIM passthrough. |
| `overcommit_warn` | percentage | `80%` | Compiler emits a warning when total virtual allocation exceeds this percentage of pool physical size. |
| `overcommit_deny` | percentage | none | Compiler refuses to compile when total virtual allocation exceeds this percentage. |

**Children:** Filesystem blocks. These become thin logical volumes in the pool. Their `size` is virtual.

**Compiler behavior:** Emits `lvcreate --thin --size <pool_size> <vg>/<pool_name>`, then `lvcreate --thin --virtualsize <lv_size> <vg>/<pool_name> --name <lv_name>` for each child.

---

## Mount Expressions

Mount expressions declare where a filesystem is accessible and with what options. They appear as inline property values.

### Syntax

```
mount = <path>
mount = <path> [<option>, <option>, ...]
```

**Examples:**

```
mount = /boot
mount = /boot [nodev, nosuid, noexec]
mount = /home [nodev, nosuid, compress=zstd:3]
mount = /var [nodev, nosuid, noexec, x-systemd.mount-timeout=30]
```

The path is the mount target. Options in brackets are comma-separated and map directly to the `-o` flag of `mount` and the options column of `/etc/fstab`.

**Default mount options:** When no options are specified, the compiler uses `defaults`. The security floor (configurable per-compilation) may inject additional options. For example, a `strict` security floor might automatically add `nodev, nosuid` to all mounts except `/` and `/boot`.

### Fstab Generation

The compiler generates a complete `/etc/fstab` from all declared mount expressions. Filesystem identification uses UUID (obtained after `mkfs` execution at install time). The `dump` and `pass` fields are automatically set:

- `pass = 1` for `/`
- `pass = 2` for all other filesystems
- `pass = 0` for swap, `nfs`, and `tmpfs`
- `dump = 0` for all entries (dump is effectively dead tooling)

### `mount` Block (Extended Form)

When mount configuration is complex enough that inline syntax becomes unwieldy, a `mount` block can replace the inline expression. This is optional — the inline form is preferred when it suffices.

```
ext4 data {
    size = 500G
    
    mount {
        target = /srv/data
        options = [nodev, nosuid, noexec]
        automount = false
        timeout = 30
        requires = [network-online.target]
    }
}
```

**Extended mount properties:**

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `target` | path | required | Mount point. |
| `options` | array of strings | `[defaults]` | Mount options. |
| `automount` | `true` \| `false` | `true` | Whether to mount at boot. When `false`, generates `noauto` in fstab. |
| `timeout` | integer | none | Mount timeout in seconds. Emits `x-systemd.mount-timeout=` for systemd or equivalent for s6. |
| `requires` | array of strings | none | Systemd units that must be active before mounting. |
| `before` | array of strings | none | Systemd units this mount must complete before. |

---

## Whole-Disk and Partitionless Layouts

Not every block device needs a partition table.

### Whole-Disk Filesystem

```
disk /dev/sdc {
    label = none
    
    xfs scratch {
        mount = /mnt/scratch
    }
}
```

When `label = none`, the disk has no partition table. Exactly one filesystem child is permitted, and it consumes the entire device. `size`, `index`, `start`, `end`, and `type` are not valid inside this child — the block device *is* the filesystem's backing device.

**Compiler behavior:** Skips `parted` entirely. Emits `mkfs` directly on the raw block device.

### Whole-Disk Encryption

```
disk /dev/sdd {
    label = none
    
    luks2 secure_scratch {
        xfs encrypted_data {
            mount = /mnt/secure
        }
    }
}
```

LUKS2 wrapping the entire device with no partition table. Valid and sometimes desirable for data drives where partition metadata is unnecessary overhead.

---

## Multi-Disk Scenarios

### Separate Boot and Root Drives

```
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
```

### RAID + LVM + Encryption Stack

```
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
```

---

## Explicit Partition Positioning

Ironclad prefers `size` over explicit `start`/`end` boundaries. The compiler calculates optimal alignment and placement automatically. However, for operators who need precise control — unusual hardware, pre-existing partition schemes, mixed-use disks — explicit positioning is available.

```
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
```

`end = -1` means end of disk.

**Mixing `size` and `start`/`end`:** Permitted. The compiler resolves explicit boundaries first, then allocates `size`-based partitions in the remaining gaps. `remaining` consumes whatever space is left after all explicit allocations.

**Validation:** The compiler rejects overlapping boundaries, gaps that result in unreachable space (unless intentional via a `raw` block), and `start`/`end` values that exceed the device's reported capacity (when detectable at compile time).

---

## Compiler Validation Rules

The compiler applies the following validation rules to storage declarations. All violations are compile-time errors unless noted as warnings.

### Structural Validation

- Every `mount` target path must be unique across the entire source tree. Duplicate mount points are an error.
- Only one `remaining` size is permitted per parent scope.
- `start`/`end` partitions must not overlap.
- A `disk` with `label = none` must have exactly one filesystem child.
- A `luks2` block without a child `lvm` may contain at most one filesystem child (LUKS opens to a single block device).
- `subvol` blocks are only valid inside `btrfs`.
- `thin` blocks are only valid inside `lvm`.
- `index` values within a `disk` must be unique and positive.
- `mdraid` member disks must not appear in more than one array.

### Capacity Validation

- The sum of `size` values for thick LVM logical volumes must not exceed the parent volume group's available physical extents. **Error.**
- Thin pool virtual allocation exceeding `overcommit_warn` threshold. **Warning.**
- Thin pool virtual allocation exceeding `overcommit_deny` threshold. **Error.**
- `start`/`end` values exceeding device capacity (when detectable). **Error.**

### Security Floor Validation

The compiler enforces a configurable security floor on storage declarations:

- **Baseline:** No enforcement — the operator's declaration is accepted as-is.
- **Standard:** `/boot` must have `nodev, nosuid, noexec`. `/tmp` must have `nodev, nosuid, noexec`. `/home` must have `nodev, nosuid`. Warnings for violations.
- **Strict:** Standard rules as errors, not warnings. Root filesystem must be on an encrypted backing device (`luks2` ancestor). Swap must be on an encrypted backing device.
- **Maximum:** Strict rules plus: all non-root mounts must have `nodev`. All mounts except `/` and `/boot` must have `nosuid`. All data-only mounts must have `noexec`.

The security floor level is declared outside the storage block (system-level configuration).

---

## Semicolon Shorthand

For simple declarations where a block contains only a few properties, the semicolon-separated inline form avoids unnecessary vertical space:

```
ext4 root { size = 50G; mount = / }
swap swap0 { size = 16G }
```

This is syntactically identical to the expanded multi-line form. The compiler makes no distinction. The convention is: use inline form for blocks with three or fewer simple properties; expand to multi-line for anything more complex.

---

## Reserved Keywords

The following words are reserved in storage context and cannot be used as block names:

`disk`, `mdraid`, `luks2`, `luks1`, `lvm`, `thin`, `ext4`, `xfs`, `btrfs`, `fat32`, `swap`, `ntfs`, `raw`, `subvol`, `mount`, `remaining`, `none`, `whole`, `true`, `false`

---

## Grammar Summary (Informative)

This section provides an informal summary of the storage grammar for readability. The canonical grammar is the PEG definition in the compiler source.

```
storage_decl    = (disk_block | mdraid_block)*

disk_block      = "disk" device_path "{" disk_body "}"
disk_body       = property* (partition_block | fs_block | luks_block)*

mdraid_block    = "mdraid" name "{" mdraid_body "}"
mdraid_body     = property* (fs_block | luks_block | lvm_block)*

partition_block = (fs_keyword | "luks2" | "luks1" | "raw") name "{" partition_body "}"
partition_body  = property* child_block*

luks_block      = ("luks2" | "luks1") name "{" property* (fs_block | lvm_block) "}"

lvm_block       = "lvm" name "{" property* (fs_block | swap_block | thin_block)* "}"
thin_block      = "thin" name "{" property* (fs_block | swap_block)* "}"

fs_block        = fs_keyword name "{" property* subvol_block* "}"
fs_keyword      = "ext4" | "xfs" | "btrfs" | "fat32" | "ntfs"

swap_block      = "swap" name "{" property* "}"
subvol_block    = "subvol" name "{" property* "}"

raw_block       = "raw" name "{" property* "}"

property        = identifier "=" value
value           = string | number | size | boolean | array | identifier
mount_expr      = path ( "[" option ("," option)* "]" )?

size            = number unit | percentage | "remaining"
unit            = "B" | "K" | "KB" | "M" | "MB" | "G" | "GB" | "T" | "TB"
```

---

## What This Document Does Not Cover

This specification covers storage declaration syntax only. The following topics are defined in separate specifications:

- **Class system and inheritance** — How storage declarations compose with classes
- **Variables, loops, and conditionals** — Parameterizing storage layouts across fleet roles
- **Kernel, init, services, users, SELinux, firewall** — Other system declaration domains
- **Compiler output mapping** — How declarations map to Kickstart, Ansible, and other backends
- **Runtime agent storage monitoring** — How drift detection applies to storage state
- **Secret management** — LUKS passphrase generation, distribution, and escrow
