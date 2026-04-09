# Ironclad Storage Syntax Specification

**Status:** Draft — syntax development, Phase 1  
**Scope:** Disk, partitioning, encryption, volume management, filesystems, mounts, and storage-level SELinux labeling

---

## Design Principles

Ironclad's storage syntax is a thin, structured wrapper over Linux storage tooling. Every block in the syntax maps to a real tool invocation — `parted`, `cryptsetup`, `pvcreate`/`lvcreate`, `mkfs.*`, `mount` — but the compiler handles ordering, validation, and interdependency resolution rather than the operator.

The key ergonomic properties:

1. **Nesting mirrors the real storage stack.** A filesystem inside an LVM volume inside a LUKS container inside a partition is written as exactly that nesting. The code reads like the actual dependency chain.
2. **Implicit ordering from structure.** The compiler topologically sorts operations from the declared hierarchy. The operator never specifies execution order.
3. **Named references.** Every named block becomes a referenceable identifier throughout the Ironclad source tree. A LUKS container named `system` can be referenced by key management declarations, clevis bindings, or firewall rules elsewhere.
4. **Validation from structure.** The compiler rejects impossible configurations at compile time — overlapping partitions, filesystems without backing devices, mount points referencing undeclared filesystems, LVM logical volumes exceeding volume group capacity, thin pool overcommit beyond configurable thresholds.
5. **Defaults that disappear the obvious.** When a default exists that a competent administrator would choose in nearly all cases, the language assumes it. Explicit declaration overrides any default. Every default is documented.
6. **Security labeling is a storage concern.** SELinux contexts on mount points are not decorative metadata — they define the trust boundary of every filesystem object created beneath that mount. The storage syntax carries enough label information for the compiler to emit correctly labeled mounts and validate those labels against the system's declared SELinux policy.

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
| --- | --- | --- |
| `label` | `gpt` \| `msdos` \| `none` | Partition table type. `none` indicates a whole-disk device with no partition table. |

**Optional properties:**

| Property | Type | Default | Description |
| --- | --- | --- | --- |
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
| --- | --- | --- |
| `level` | `0` \| `1` \| `5` \| `6` \| `10` | RAID level. |
| `disks` | array of device paths | Member devices. |

**Optional properties:**

| Property | Type | Default | Description |
| --- | --- | --- | --- |
| `spare` | array of device paths | none | Hot spare devices. |
| `chunk` | size string | kernel default | Chunk size for striped levels (`64K`, `512K`, etc.). |
| `bitmap` | `internal` \| `none` \| path | `internal` | Write-intent bitmap location. |
| `metadata` | `1.0` \| `1.1` \| `1.2` | `1.2` | Metadata version. `1.0` stores metadata at end of device (required for boot arrays on some bootloaders). |
| `layout` | string | kernel default | RAID layout. Level-specific (e.g., `f2` for RAID10 far layout). |
| `name` | string | array name | Human-readable name written to metadata. |

**Children:** Same as `disk` — filesystem blocks, `luks2`, `lvm`, or `raw`.

**Compiler behavior:** Emits `mdadm --create /dev/md/<n>` with the specified parameters. Member devices must either be raw partitions declared elsewhere in the source tree (compiler validates their existence) or assumed-present paths for pre-existing hardware.

---

## Partition-Level Blocks

Direct children of a `disk` block represent partitions. The block keyword is the filesystem type that will be created on the partition, or a structural keyword (`luks2`, `raw`).

### Common Partition Properties

Every direct child of a `disk` block accepts these properties:

| Property | Type | Default | Description |
| --- | --- | --- | --- |
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
| --- | --- | --- | --- |
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
| --- | --- | --- | --- |
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
| --- | --- | --- | --- |
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
| --- | --- | --- | --- |
| `mount` | mount expression | none | Mount target and options. The `subvol=<n>` mount option is automatically appended by the compiler. |
| `quota` | size string | none | Btrfs qgroup limit for this subvolume. Requires `quota` in parent's `features`. |
| `compress` | string | inherited from parent | Override parent compression for this subvolume (applied as mount option). |

**Compiler behavior:** Emits `btrfs subvolume create <parent_mount>/<n>`. If `quota` is set, emits `btrfs qgroup limit <size> <parent_mount>/<n>`.

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
| --- | --- | --- | --- |
| `label` | string | block name (uppercased, truncated to 11 chars) | Volume label (`mkfs.fat -n`). |
| `fat_size` | `12` \| `16` \| `32` | `32` | FAT variant (`mkfs.fat -F`). |
| `cluster_size` | integer | auto | Cluster size in bytes (`mkfs.fat -s`). |
| `mount` | mount expression | none | Mount target and options. |

**Compiler behavior:** Emits `mkfs.fat -F 32` (or specified variant).

**SELinux note:** `fat32` does not support extended attributes. The compiler enforces that any `fat32` filesystem with a `mount` declaration must have an explicit `context` in its mount expression when the system's SELinux mode is `mls` or `strict` security floor is active. Without `context=`, all files under the mount inherit an unlabeled type, which MLS policy will deny access to. See [SELinux Context on Mount Expressions](#selinux-context-on-mount-expressions).

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
| --- | --- | --- | --- |
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
| --- | --- | --- | --- |
| `label` | string | block name | Volume label (`mkfs.ntfs -L`). |
| `compression` | `true` \| `false` | `false` | Enable NTFS compression. |
| `quick` | `true` \| `false` | `true` | Quick format — skip full surface scan (`mkfs.ntfs -Q`). |
| `mount` | mount expression | none | Mount target and options. |

**Compiler behavior:** Emits `mkfs.ntfs`. Requires `ntfs-3g` in the image package list.

**SELinux note:** `ntfs` does not support extended attributes. Same enforcement rules as `fat32` — explicit `context` required under MLS. See [SELinux Context on Mount Expressions](#selinux-context-on-mount-expressions).

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
| --- | --- | --- | --- |
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

**Compiler behavior:** Emits `cryptsetup luksFormat` with mapped flags, followed by `cryptsetup open`. If `tpm2` or `tang` is set, emits the corresponding `clevis luks bind` commands. The opened device name is derived from the block name (`/dev/mapper/<n>`).

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
| --- | --- | --- | --- |
| `pe_size` | size string | `4M` | Physical extent size (`vgcreate -s`). |
| `max_lv` | integer | unlimited | Maximum logical volumes (`vgcreate -l`). |
| `clustered` | `true` \| `false` | `false` | Clustered volume group flag. |
| `tags` | array of strings | none | LVM tags applied to the VG. |

**Children:** Filesystem blocks (thick logical volumes), `swap` blocks, and `thin` pool blocks. Direct children of `lvm` are standard (thick) logical volumes. Their `size` is physically allocated.

**Compiler behavior:** Emits `pvcreate` on the parent block device, `vgcreate <n>`, then `lvcreate` for each child volume in declaration order. The volume is named `/dev/<vg_name>/<lv_name>` where `lv_name` is the child block's name.

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
| --- | --- | --- | --- |
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

* `pass = 1` for `/`
* `pass = 2` for all other filesystems
* `pass = 0` for swap, `nfs`, and `tmpfs`
* `dump = 0` for all entries (dump is effectively dead tooling)

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
| --- | --- | --- | --- |
| `target` | path | required | Mount point. |
| `options` | array of strings | `[defaults]` | Mount options. |
| `automount` | `true` \| `false` | `true` | Whether to mount at boot. When `false`, generates `noauto` in fstab. |
| `timeout` | integer | none | Mount timeout in seconds. Emits `x-systemd.mount-timeout=` for systemd or equivalent for s6. |
| `requires` | array of strings | none | Systemd units that must be active before mounting. |
| `before` | array of strings | none | Systemd units this mount must complete before. |
| `context` | SELinux context expression | none | SELinux security context for the mount. See [SELinux Context on Mount Expressions](#selinux-context-on-mount-expressions). |
| `fscontext` | SELinux context expression | none | SELinux context for the filesystem superblock object. |
| `defcontext` | SELinux context expression | none | SELinux default context for unlabeled files. |
| `rootcontext` | SELinux context expression | none | SELinux context for the root inode before the filesystem is visible. |

---

## SELinux Context on Mount Expressions

SELinux labels on mount points are the trust boundary between the storage layer and the policy layer. A mislabeled mount under MLS is not a cosmetic defect — it is a policy breach that either denies all access or silently grants access at the wrong sensitivity level. The storage syntax carries label information so the compiler can emit correctly labeled mounts and validate them at compile time.

### Context Expression Syntax

An SELinux context expression is a structured tuple of four (targeted/strict) or five (MLS) colon-separated fields:

```
<user>:<role>:<type>:<range>
```

Where `<range>` is a sensitivity-category expression:

```
s0                          # single sensitivity, no categories
s0:c0.c1023                 # single sensitivity, category range
s0-s15:c0.c1023             # sensitivity range with category range
s0-s3:c0,c5,c12             # sensitivity range with discrete categories
```

The four mount-level context properties map to the four SELinux mount options:

| Ironclad property | Linux mount option | Behavior |
| --- | --- | --- |
| `context` | `context=` | Labels all files and the filesystem itself. Overrides all on-disk xattr labels. Mutually exclusive with the other three. |
| `fscontext` | `fscontext=` | Labels the filesystem superblock only. Used with `defcontext` for xattr-capable filesystems that need a non-default superblock label. |
| `defcontext` | `defcontext=` | Default label for files that have no xattr label. Only affects unlabeled files. |
| `rootcontext` | `rootcontext=` | Labels the root inode before the filesystem is visible to userspace. Used when the root inode must have a specific label for policy transitions during boot. |

### Inline Form

For the common case where only `context` is needed, the inline mount syntax supports a trailing context clause:

```
mount = /boot/efi [nodev, nosuid, noexec] context system_u:object_r:boot_t:s0
```

This is syntactic sugar for the equivalent extended form. Only `context` is available inline — the other three properties (`fscontext`, `defcontext`, `rootcontext`) require the extended `mount` block.

### Extended Form

```
fat32 efi {
    size = 1G
    type = ef00
    
    mount {
        target = /boot/efi
        options = [nodev, nosuid, noexec]
        context = system_u:object_r:boot_t:s0
    }
}
```

For xattr-capable filesystems that need fine-grained control:

```
ext4 containers {
    size = 200G
    
    mount {
        target = /var/lib/containers
        options = [nodev, nosuid]
        defcontext = system_u:object_r:container_var_lib_t:s0
        rootcontext = system_u:object_r:container_var_lib_t:s0
    }
}
```

### MLS-Specific Examples

Under MLS, sensitivity ranges become critical. A multi-level mount point serving data across sensitivity levels:

```
ext4 shared_data {
    size = 100G
    
    mount {
        target = /srv/shared
        options = [nodev, nosuid, noexec]
        defcontext = system_u:object_r:shared_content_t:s0-s9:c0.c255
    }
}
```

A single-level mount pinned to a specific classification:

```
xfs classified {
    size = 500G
    
    mount {
        target = /srv/classified
        options = [nodev, nosuid, noexec]
        context = system_u:object_r:classified_content_t:s5:c0.c127
    }
}
```

Subvolumes with per-mount sensitivity isolation:

```
btrfs data {
    size = remaining
    compress = zstd:1
    
    subvol @public {
        mount {
            target = /srv/public
            options = [nodev, nosuid, noexec]
            defcontext = system_u:object_r:public_content_t:s0
        }
    }
    
    subvol @restricted {
        mount {
            target = /srv/restricted
            options = [nodev, nosuid, noexec]
            defcontext = system_u:object_r:restricted_content_t:s3:c0.c127
        }
    }
    
    subvol @toplevel {
        mount {
            target = /srv/toplevel
            options = [nodev, nosuid, noexec]
            defcontext = system_u:object_r:toplevel_content_t:s7:c0.c255
        }
    }
}
```

### Context and `context=` Mutual Exclusivity

When `context` is set, `fscontext`, `defcontext`, and `rootcontext` are invalid — this is how the Linux mount system works. The compiler rejects declarations that set `context` alongside any of the other three. The logic:

- **`context`** = "I want every object under this mount to carry this label, period. Ignore xattrs." This is the correct choice for xattr-incapable filesystems (`fat32`, `ntfs`, `tmpfs`) and for mounts where a blanket label is operationally appropriate.
- **`fscontext` + `defcontext` + `rootcontext`** = "The filesystem supports xattrs and I want labeled files, but I need to override the defaults for unlabeled objects, the superblock, or the root inode." Use these for xattr-capable filesystems (`ext4`, `xfs`, `btrfs`) where `restorecon` or policy file contexts will handle per-file labeling, but the mount-level defaults need to be set for MLS range correctness.

### Filesystem xattr Capability and Enforcement

The compiler knows which filesystem types support SELinux xattr labeling and which do not:

| Filesystem | xattr support | Label strategy |
| --- | --- | --- |
| `ext4` | yes | File contexts via `restorecon`; mount-level `defcontext`/`rootcontext` optional |
| `xfs` | yes | Same as ext4 |
| `btrfs` | yes | Same as ext4 |
| `fat32` | no | **Must** use `context=` for all labeling |
| `ntfs` | no | **Must** use `context=` for all labeling |
| `swap` | n/a | Labeled via policy, not mount context |
| `tmpfs` (future) | no | **Must** use `context=` for all labeling |

When no xattr support exists and the SELinux mode is `mls`, the compiler enforces that `context` is declared. Without it, files under the mount inherit `unlabeled_t` which MLS policy denies access to — a guaranteed boot failure or data access failure.

---

## SELinux Sensitivity and Category Validation

The compiler validates SELinux context expressions against the system's declared policy parameters. These parameters are defined outside the storage syntax in the system-level SELinux configuration block (see separate specification), but the storage compiler consumes them for validation.

### What the Storage Compiler Validates

1. **Context field count.** A context must have exactly four colon-separated fields: `user:role:type:range`. Three-field contexts (targeted shorthand) are not valid under MLS — the compiler rejects them.

2. **Range syntax.** The `range` field must be a valid MLS range expression:
   - Sensitivity: `s0` through `s<N>` where `N` is the system's declared `max_sensitivity`.
   - Sensitivity range: `s<low>-s<high>` where `low <= high` and both are within bounds.
   - Categories: `c<N>` discrete, `c<low>.c<high>` range, comma-separated combinations.
   - Category values must be within the system's declared `max_category`.

3. **User existence.** The `user` field must reference an SELinux user declared in the policy module list. The compiler cross-references against the system-level SELinux user declarations.

4. **Type existence.** The `type` field must reference a type declared in one of the loaded policy modules. The compiler cross-references against the system-level module manifest.

5. **Role-user validity.** The `role` field must be a role that the declared user is authorized to assume.

6. **Dominance in ranges.** When a mount declares a sensitivity range (`s0-s5`), the compiler verifies that the low sensitivity dominates (is less than or equal to) the high sensitivity. The compiler also verifies that the declared range does not exceed the user's authorized range.

### What the Storage Compiler Does Not Validate

- Per-file type enforcement rules (that is TE policy, not a storage concern).
- Whether the declared type is appropriate for the mount path (that requires policy-level semantic understanding beyond the storage compiler's scope — the SELinux policy domain specification covers this).
- MLS constraint satisfaction beyond range validity (e.g., whether a process at `s3` can write to a filesystem labeled `s5` — that is a runtime policy enforcement question).

### Validation Failure Behavior

All SELinux context validation failures are compile-time **errors**, not warnings. A malformed context will cause a mount failure at boot time — there is no reasonable "warn and continue" behavior. The compiler halts and reports:

- The offending storage block name and mount target
- The invalid context expression
- Which validation rule was violated
- The valid range/set for the violated field (when applicable)

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

### MLS Multi-Level Storage Server

A storage layout for a system handling data at multiple sensitivity levels, with per-volume SELinux labeling:

```
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
        mount {
            target = /boot
            options = [nodev, nosuid, noexec]
            defcontext = system_u:object_r:boot_t:s0
        }
    }
}

disk /dev/nvme0n1 {
    label = gpt
    
    luks2 system {
        index = 1
        size = remaining
        tpm2 = true
        integrity = hmac-sha256
        
        lvm vg_system {
            ext4 root {
                size = 50G
                mount {
                    target = /
                    defcontext = system_u:object_r:default_t:s0-s15:c0.c1023
                }
            }
            
            ext4 var {
                size = 30G
                mount {
                    target = /var
                    options = [nodev, nosuid, noexec]
                    defcontext = system_u:object_r:var_t:s0-s15:c0.c1023
                }
            }
            
            xfs unclassified {
                size = 200G
                mount {
                    target = /srv/unclassified
                    options = [nodev, nosuid, noexec]
                    defcontext = system_u:object_r:public_content_t:s0
                }
            }
            
            xfs confidential {
                size = 500G
                mount {
                    target = /srv/confidential
                    options = [nodev, nosuid, noexec]
                    defcontext = system_u:object_r:confidential_content_t:s4:c0.c255
                }
            }
            
            xfs secret {
                size = 500G
                mount {
                    target = /srv/secret
                    options = [nodev, nosuid, noexec]
                    defcontext = system_u:object_r:secret_content_t:s8:c0.c255
                }
            }
            
            xfs topsecret {
                size = 500G
                mount {
                    target = /srv/topsecret
                    options = [nodev, nosuid, noexec]
                    defcontext = system_u:object_r:topsecret_content_t:s12:c0.c255
                }
            }
            
            swap swap0 { size = 32G }
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

* Every `mount` target path must be unique across the entire source tree. Duplicate mount points are an error.
* Only one `remaining` size is permitted per parent scope.
* `start`/`end` partitions must not overlap.
* A `disk` with `label = none` must have exactly one filesystem child.
* A `luks2` block without a child `lvm` may contain at most one filesystem child (LUKS opens to a single block device).
* `subvol` blocks are only valid inside `btrfs`.
* `thin` blocks are only valid inside `lvm`.
* `index` values within a `disk` must be unique and positive.
* `mdraid` member disks must not appear in more than one array.

### Capacity Validation

* The sum of `size` values for thick LVM logical volumes must not exceed the parent volume group's available physical extents. **Error.**
* Thin pool virtual allocation exceeding `overcommit_warn` threshold. **Warning.**
* Thin pool virtual allocation exceeding `overcommit_deny` threshold. **Error.**
* `start`/`end` values exceeding device capacity (when detectable). **Error.**

### SELinux Context Validation

* `context` is mutually exclusive with `fscontext`, `defcontext`, and `rootcontext`. Declaring both is an **error**.
* Every context expression must have exactly four colon-separated fields (`user:role:type:range`). Three-field contexts are an **error** under MLS.
* Sensitivity values must be within the system's declared `max_sensitivity`. Out-of-range sensitivities are an **error**.
* Category values must be within the system's declared `max_category`. Out-of-range categories are an **error**.
* In a sensitivity range `s<low>-s<high>`, `low` must be less than or equal to `high`. Inverted ranges are an **error**.
* The `user` field must reference a declared SELinux user. Unknown users are an **error**.
* The `type` field must reference a type declared in the loaded policy module manifest. Unknown types are an **error**.
* The `role` field must be authorized for the declared user. Unauthorized roles are an **error**.
* A `fat32` or `ntfs` filesystem with a `mount` declaration under MLS or `strict`+ security floor must have an explicit `context`. Missing context on xattr-incapable filesystems is an **error**.
* An xattr-capable filesystem (`ext4`, `xfs`, `btrfs`) must not use `context` when per-file labeling is expected — this silently overrides all xattr labels. When the security floor is `maximum`, `context` on xattr-capable filesystems is an **error** (use `defcontext`/`rootcontext` instead). At lower security floors, this is a **warning**.

### Security Floor Validation

The compiler enforces a configurable security floor on storage declarations:

* **Baseline:** No enforcement — the operator's declaration is accepted as-is.
* **Standard:** `/boot` must have `nodev, nosuid, noexec`. `/tmp` must have `nodev, nosuid, noexec`. `/home` must have `nodev, nosuid`. Warnings for violations. Under MLS: warnings for mounts without SELinux context declarations.
* **Strict:** Standard rules as errors, not warnings. Root filesystem must be on an encrypted backing device (`luks2` ancestor). Swap must be on an encrypted backing device. Under MLS: xattr-incapable filesystems must have explicit `context`. All mounts should have either `context` or `defcontext` — missing labels are **warnings**.
* **Maximum:** Strict rules plus: all non-root mounts must have `nodev`. All mounts except `/` and `/boot` must have `nosuid`. All data-only mounts must have `noexec`. Under MLS: **every** mount must have an explicit SELinux context declaration (`context` for non-xattr, `defcontext` or `rootcontext` for xattr-capable). Missing labels are **errors**. `context` on xattr-capable filesystems is an **error** (must use `defcontext`/`rootcontext` to preserve per-file labeling).

The security floor level is declared outside the storage block (system-level configuration).

---

## Semicolon Shorthand

For simple declarations where a block contains only a few properties, the semicolon-separated inline form avoids unnecessary vertical space:

```
ext4 root { size = 50G; mount = / }
swap swap0 { size = 16G }
```

This is syntactically identical to the expanded multi-line form. The compiler makes no distinction. The convention is: use inline form for blocks with three or fewer simple properties; expand to multi-line for anything more complex.

**Note:** Inline mount expressions with a trailing `context` clause remain valid in shorthand:

```
fat32 efi { size = 1G; type = ef00; mount = /boot/efi [nodev, nosuid, noexec] context system_u:object_r:boot_t:s0 }
```

However, this approaches the complexity threshold where the extended form is more readable.

---

## Reserved Keywords

The following words are reserved in storage context and cannot be used as block names:

`disk`, `mdraid`, `luks2`, `luks1`, `lvm`, `thin`, `ext4`, `xfs`, `btrfs`, `fat32`, `swap`, `ntfs`, `raw`, `subvol`, `mount`, `remaining`, `none`, `whole`, `true`, `false`, `context`, `fscontext`, `defcontext`, `rootcontext`

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
                | selinux_context

mount_expr      = path ( "[" option ("," option)* "]" )? ( "context" selinux_context )?
mount_block     = "mount" "{" mount_property* "}"
mount_property  = "target" "=" path
                | "options" "=" "[" option ("," option)* "]"
                | "automount" "=" boolean
                | "timeout" "=" integer
                | "requires" "=" "[" string ("," string)* "]"
                | "before" "=" "[" string ("," string)* "]"
                | "context" "=" selinux_context
                | "fscontext" "=" selinux_context
                | "defcontext" "=" selinux_context
                | "rootcontext" "=" selinux_context

selinux_context = selinux_user ":" selinux_role ":" selinux_type ":" mls_range
mls_range       = sensitivity ( "-" sensitivity )? ( ":" category_set )?
sensitivity     = "s" digit+
category_set    = category_expr ( "," category_expr )*
category_expr   = "c" digit+ ( "." "c" digit+ )?

size            = number unit | percentage | "remaining"
unit            = "B" | "K" | "KB" | "M" | "MB" | "G" | "GB" | "T" | "TB"
```

---

## What This Document Does Not Cover

This specification covers storage declaration syntax only. The following topics are defined in separate specifications:

* **Class system and inheritance** — How storage declarations compose with classes
* **Variables, loops, and conditionals** — Parameterizing storage layouts across fleet roles
* **Kernel, init, services, users, SELinux policy, firewall** — Other system declaration domains
* **SELinux policy modules and type enforcement** — Declaring SELinux users, roles, types, modules, and TE rules (the storage syntax only consumes these declarations for validation — it does not define them)
* **SELinux file contexts** — Per-path labeling rules applied by `restorecon` (separate from the mount-level context declarations in this specification)
* **Compiler output mapping** — How declarations map to Kickstart, Ansible, and other backends
* **Runtime agent storage monitoring** — How drift detection applies to storage state, including SELinux label drift on mount points
* **Secret management** — LUKS passphrase generation, distribution, and escrow
