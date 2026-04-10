# Ironclad Bootloader Syntax Specification

**Status:** Draft — syntax development, Phase 1  
**Category:** Standard library class API — the bootloader is not a compiler-native construct. This specification defines the API for standard library bootloader classes that emit configuration files. The compiler validates bootloader declarations structurally (correct file paths, ownership, permissions, storage references) but does not have built-in knowledge of bootloader formats.  
**Scope:** Bootloader installation, configuration, kernel entries, UKI bundling, multi-boot, console, and boot environment management  
**Dependencies:** `STORAGE_SYNTAX.md`, `FILESYSTEM_SYNTAX.md`, `LANGUAGE_SYNTAX.md`

---

## Design Principles

The bootloader is files on a filesystem. In Ironclad, the bootloader declaration lives inside the storage topology — typically nested under the EFI System Partition or `/boot` filesystem where bootloader files physically reside. This is consistent with the core Ironclad principle: the storage tree is the system.

The compiler emits a bash script from the bootloader declaration. That script is executed during Kickstart `%post` (or equivalent install-time hook) to install, configure, and populate the bootloader. This means:

1. **Any bootloader the operator can script is a valid target.** GRUB2, systemd-boot, rEFInd, syslinux, Limine, or anything else — if it can be installed and configured from a shell, Ironclad can emit for it.

2. **The compiler does not abstract away the bootloader.** The operator declares which bootloader, which entries, which kernel parameters. The compiler validates, cross-references against storage and kernel declarations, and emits the script. It does not choose for you.

3. **UKI bundling is declared alongside entries.** When a boot entry uses a Unified Kernel Image, the UKI composition (kernel, initrd, cmdline, splash, OS release, PCR policy) is declared in the entry block. The compiler emits the `ukify` or equivalent invocation.

4. **Multi-boot is first-class.** Multiple entries, fallback entries, previous-generation rollback entries, and foreign OS chainload entries are all expressible.

5. **Authoring flexibility.** The bootloader block can be written directly in the storage tree (inline), authored in a class and applied to the system, or built as a parameterized function and embedded. All three resolve to the same compiled form. See `LANGUAGE_SYNTAX.md` for the class, function, and embed mechanisms.

---

## Placement in the Storage Tree

The `bootloader` block is declared inside the filesystem block where the bootloader physically resides — typically the EFI System Partition for UEFI systems or the `/boot` filesystem for BIOS systems.

```
disk /dev/sda {
    label = gpt

    fat32 efi {
        size = 1G
        type = ef00
        mount = /boot/efi

        bootloader grub2 "main" {
            # configuration here
        }

        dir EFI/almalinux {
            file grubx64.efi { package = grub2-efi-x64 }
        }
    }
}
```

When the `bootloader` block is embedded in the tree, the compiler automatically resolves the ESP reference from context — the enclosing filesystem *is* the ESP. The `esp` property is still available for cases where the bootloader writes to a different filesystem than the one it's declared in, but is optional when the enclosing filesystem is the correct target.

For bootloader declarations authored as classes or functions (see `LANGUAGE_SYNTAX.md`), the compiler resolves placement when the class is applied, using the same mount-point matching as all other class projections.

### Alternative: Bootloader at System Scope

The bootloader block can also be declared at the `system` scope (outside the storage tree) with an explicit `esp` reference. This is equivalent — the compiler resolves it into the tree:

```
system web01 {
    disk /dev/sda { ... }

    bootloader grub2 "main" {
        esp = storage.sda.efi       # explicit reference required when outside the tree
        # ...
    }
}
```

Both placements compile identically. Inline-in-tree is preferred when the bootloader is simple. System-scope is useful when the bootloader class is complex or shared across systems.

---

## Bootloader Block

The keyword after `bootloader` is the **backend**: `grub2`, `systemd-boot`, `refind`, `syslinux`, `limine`, or `raw`. The string after the backend is the **name** — a referenceable identifier for cross-referencing elsewhere in the Ironclad source tree.

### Backend Keywords

| Keyword        | Tool                  | Notes                                                        |
|----------------|-----------------------|--------------------------------------------------------------|
| `grub2`        | GRUB 2                | Full-featured, supports BIOS and UEFI, LUKS1 boot encryption |
| `systemd-boot` | systemd-boot (gummiboot) | UEFI only, lightweight, strong UKI integration            |
| `refind`       | rEFInd                | UEFI only, graphical boot manager                            |
| `syslinux`     | SYSLINUX / ISOLINUX   | BIOS or UEFI, lightweight, limited feature set               |
| `limine`       | Limine                | BIOS and UEFI, minimal                                       |
| `raw`          | None                  | Compiler emits no bootloader installation; operator provides full script via `raw_script` |

The compiler validates that the declared backend is compatible with the declared storage layout. For example, `systemd-boot` requires a GPT disk with an EFI System Partition; the compiler cross-references the storage declaration and errors if no ESP is found.

---

## Properties

Properties inside the `bootloader` block configure installation behavior and global bootloader settings.

### Installation Target

```
bootloader grub2 "main" {
    target = uefi               # uefi | bios | uefi+bios
    esp = storage.sda.efi       # reference to EFI partition declared in storage
    install_device = /dev/sda   # BIOS: MBR install target. Ignored for pure UEFI.
}
```

`target` declares the firmware interface. `uefi+bios` instructs the compiler to emit both UEFI and BIOS installation steps (hybrid boot media).

`esp` is a **storage reference** — a dotted path into the storage declaration tree. The compiler resolves this to the device path and mount point of the referenced partition and validates that it has the correct partition type (ef00 / EFI System Partition). Required when `target` includes `uefi`.

`install_device` is required when `target` includes `bios`. The compiler validates that the referenced device exists in the storage declaration.

### Timeout and Menu Behavior

```
bootloader grub2 "main" {
    timeout = 5                 # seconds. 0 = no menu, boot immediately. -1 = wait indefinitely.
    timeout_style = menu        # menu | countdown | hidden
    default = "alma"            # name of the default boot entry
}
```

All three properties are optional. When omitted, the compiler emits no value and the bootloader's own compiled-in default applies. There is no Ironclad default — the operator gets the bootloader's native behavior unless they override it.

### Console and Serial

```
bootloader grub2 "main" {
    console {
        terminal_input = [console, serial]
        terminal_output = [console, serial]
        
        serial {
            unit = 0
            speed = 115200
            word = 8
            parity = no
            stop = 1
        }
    }
}
```

The `console` block configures bootloader terminal I/O. `terminal_input` and `terminal_output` accept arrays of terminal names. The `serial` sub-block configures serial port parameters. All properties are optional; when the entire `console` block is omitted, the bootloader's compiled-in console defaults apply.

For backends that do not support serial configuration (e.g., `systemd-boot` has no interactive console), the compiler emits a warning and ignores the `console` block.

### Theme and Appearance

```
bootloader grub2 "main" {
    theme {
        path = /boot/grub2/themes/ironclad   # path to theme directory/file
        resolution = 1920x1080                # gfxmode for GRUB2, screen_size for rEFInd
        background = /path/to/background.png
        font = /path/to/font.pf2
        font_size = 16
    }
}
```

All properties are optional. The entire `theme` block is optional. When omitted, the bootloader's native appearance is used unmodified. The compiler copies referenced files into the image during build.

Backend-specific: `resolution` maps to `GRUB_GFXMODE` for grub2, `resolution` for refind, and is ignored for syslinux/limine. The compiler translates the property to the correct backend mechanism.

### Password Protection

```
bootloader grub2 "main" {
    password {
        user = "root"
        hash = "grub.pbkdf2.sha512.10000.XXXX"   # pre-hashed
        unrestricted = false                        # if true, normal entries boot without password; editing requires password
    }
}
```

When `password` is declared, the compiler emits bootloader-level authentication configuration. The `hash` value must be pre-computed; the compiler does not hash passwords. For `grub2`, this generates `set superusers` and `password_pbkdf2` directives. For backends that do not support password protection, the compiler errors.

`unrestricted` defaults to `false` (all entries require authentication). When `true`, only editing entries and accessing the bootloader shell require the password.

---

## Boot Entries

Boot entries are declared as named sub-blocks inside the `bootloader` block.

```
bootloader grub2 "main" {
    target = uefi
    esp = storage.sda.efi
    default = "alma"
    timeout = 5

    entry "alma" {
        kernel = /boot/vmlinuz-6.12.0-0.rc1.alma
        initrd = /boot/initramfs-6.12.0-0.rc1.alma.img
        
        params {
            root = "UUID=xxxx-xxxx"
            ro = true
            rhgb = true
            quiet = true
            
            # Security-relevant params the compiler cross-validates
            selinux = 1
            enforcing = 1
            
            # Raw params for anything not structured
            raw = ["mitigations=auto", "iommu=pt"]
        }
        
        options {
            class = "linux"         # GRUB2 --class, rEFInd icon selector
            unrestricted = true     # override bootloader-level password for this entry
        }
    }
}
```

### Entry Properties

| Property   | Required | Description                                                              |
|------------|----------|--------------------------------------------------------------------------|
| `kernel`   | Yes*     | Path to the kernel image. Not required if `uki` block is present.        |
| `initrd`   | No       | Path to initramfs/initrd image. Can be an array for multiple initrds.    |
| `params`   | No       | Kernel command-line parameters block.                                    |
| `options`  | No       | Backend-specific entry metadata.                                         |
| `uki`      | No       | UKI composition block. Mutually exclusive with `kernel` + `initrd`.      |
| `chainload`| No       | Chainload target for foreign OS entries. Mutually exclusive with `kernel`.|

### Multiple Initrds

```
entry "alma" {
    kernel = /boot/vmlinuz-6.12.0
    initrd = [
        /boot/initramfs-6.12.0.img,
        /boot/microcode.cpio
    ]
}
```

The compiler emits multiple `initrd` lines (GRUB2) or the correct concatenated initrd argument for the backend.

---

## Kernel Parameters Block

The `params` block inside an entry provides hybrid structured/raw kernel command-line declaration.

### Structured Parameters

Structured parameters are typed values the compiler understands and can validate:

```
params {
    # Filesystem / root
    root = "UUID=xxxx-xxxx-xxxx"
    rootfstype = "xfs"
    rootflags = "defaults"
    ro = true
    rw = false
    
    # SELinux
    selinux = 1             # 0 or 1
    enforcing = 1           # 0 or 1
    
    # Console
    console = ["tty0", "ttyS0,115200n8"]   # array emits multiple console= params
    
    # LUKS / encryption
    rd_luks_uuid = "luks-xxxx-xxxx"
    rd_luks_name = "luks-xxxx-xxxx=system"
    
    # Networking
    ip = "dhcp"
    rd_neednet = 1
    
    # Dracut
    rd_break = false        # true = drop to dracut shell (debug only)
    
    # systemd
    systemd_unit = "multi-user.target"
    
    # Memory / CPU
    mem = "4G"
    maxcpus = 8
    nosmt = false
    
    # IOMMU
    iommu = "pt"
    intel_iommu = "on"
    amd_iommu = "on"
    
    # Mitigations
    mitigations = "auto"    # auto | off | auto,nosmt
    
    # Graphics
    nomodeset = false
    
    # Raw — anything not covered above
    raw = ["custom.param=value", "another_param"]
}
```

**Compiler cross-validation:**

- `root` is validated against the storage declaration. If the storage tree declares a root filesystem with a known UUID, the compiler can verify (or auto-populate) the `root` parameter.
- `selinux` and `enforcing` are validated against the SELinux declaration. If the system declares `selinux { mode = enforcing }`, the compiler errors if the bootloader entry sets `selinux = 0` or `enforcing = 0`.
- `rd_luks_uuid` and `rd_luks_name` are validated against LUKS containers in the storage declaration.
- `console` entries are validated against the bootloader `console` block for consistency.
- `ro` / `rw` are validated: only one can be `true`.

### Raw Parameters

The `raw` array accepts arbitrary strings that are appended to the kernel command line verbatim. The compiler performs no validation on raw parameters. This is the escape hatch for parameters not yet covered by the structured syntax.

```
params {
    raw = [
        "hugepagesz=1G",
        "hugepages=4",
        "isolcpus=2-7",
        "nohz_full=2-7",
        "rcu_nocbs=2-7",
        "processor.max_cstate=1",
        "intel_idle.max_cstate=0"
    ]
}
```

### Parameter Removal

When inheriting from a base class that declares kernel parameters, a derived class may need to remove specific parameters rather than just override them.

```
params {
    quiet = remove            # removes quiet from inherited params
    rhgb = remove             # removes rhgb from inherited params
    raw_remove = ["mitigations=auto"]   # removes specific raw params by exact match
}
```

`remove` is a reserved keyword in parameter context. `raw_remove` matches and removes from the inherited `raw` array by exact string match.

---

## UKI (Unified Kernel Image) Block

When a boot entry uses a UKI, the `uki` block declares the composition. The compiler emits a `ukify` invocation (or equivalent) to assemble the UKI during image build.

```
entry "alma-uki" {
    uki {
        kernel = /boot/vmlinuz-6.12.0
        initrd = [
            /boot/initramfs-6.12.0.img,
            /boot/microcode.cpio
        ]
        cmdline = params                    # reference to a params block (see below)
        os_release = /etc/os-release
        splash = /boot/splash.bmp           # optional splash image
        
        output = /boot/efi/EFI/Linux/alma-6.12.0.efi
        
        # PCR policy — which TPM2 PCRs the UKI measures into
        pcr {
            banks = [sha256, sha384]
            sign = true                     # sign PCR measurements
            private_key = /path/to/pcr.key
            certificate = /path/to/pcr.crt
        }
        
        # Signing — sign the UKI PE binary itself
        sign {
            tool = sbsign                   # sbsign | pesign
            key = /path/to/db.key
            certificate = /path/to/db.crt
        }
    }
}
```

### UKI Properties

| Property     | Required | Description                                                            |
|--------------|----------|------------------------------------------------------------------------|
| `kernel`     | Yes      | Path to kernel image to bundle.                                        |
| `initrd`     | No       | Path or array of paths to initrd images.                               |
| `cmdline`    | No       | Inline params block, reference to named params block, or raw string.   |
| `os_release` | No       | Path to os-release file to embed. Default: `/etc/os-release`.          |
| `splash`     | No       | Path to BMP splash image.                                              |
| `uname`      | No       | Kernel version string. Auto-detected from `kernel` path if omitted.    |
| `output`     | Yes      | Output path for the assembled UKI `.efi` binary.                       |
| `pcr`        | No       | TPM2 PCR measurement policy.                                           |
| `sign`       | No       | PE binary signing configuration.                                       |

### UKI cmdline: Three Forms

The `cmdline` property accepts three forms:

**1. Reference to a named params block:**

```
# Defined at bootloader scope or system scope
params "hardened_cmdline" {
    root = "UUID=xxxx"
    ro = true
    selinux = 1
    enforcing = 1
    mitigations = "auto"
}

entry "alma-uki" {
    uki {
        cmdline = params.hardened_cmdline
    }
}
```

**2. Inline params block:**

```
entry "alma-uki" {
    uki {
        cmdline {
            root = "UUID=xxxx"
            ro = true
            raw = ["mitigations=auto"]
        }
    }
}
```

**3. Raw string (no validation):**

```
entry "alma-uki" {
    uki {
        cmdline = "root=UUID=xxxx ro quiet rhgb selinux=1 enforcing=1"
    }
}
```

The compiler applies structured cross-validation only for forms 1 and 2. Form 3 is passed through verbatim.

---

## Chainload Entries

For foreign operating systems or secondary bootloaders:

```
entry "windows" {
    chainload {
        path = /EFI/Microsoft/Boot/bootmgfw.efi   # UEFI chainload path
    }
    options {
        class = "windows"
    }
}

entry "legacy_os" {
    chainload {
        device = /dev/sdb           # BIOS: chainload from this device's MBR
        partition = 1               # optional: chainload from specific partition
    }
}

entry "secondary_grub" {
    chainload {
        path = /EFI/fedora/grubx64.efi
    }
}
```

`chainload` is mutually exclusive with `kernel`, `initrd`, and `uki`. The compiler validates that UEFI chainload entries use `path` and BIOS chainload entries use `device`.

---

## Fallback and Rollback Entries

```
entry "alma-current" {
    kernel = /boot/vmlinuz-current
    initrd = /boot/initramfs-current.img
    params { root = "UUID=xxxx"; ro = true }
}

entry "alma-previous" {
    fallback_for = "alma-current"
    kernel = /boot/vmlinuz-previous
    initrd = /boot/initramfs-previous.img
    params { root = "UUID=xxxx"; ro = true }
}

entry "rescue" {
    rescue = true
    kernel = /boot/vmlinuz-current
    initrd = /boot/initramfs-current.img
    params {
        root = "UUID=xxxx"
        ro = true
        single = true
        systemd_unit = "rescue.target"
    }
}
```

`fallback_for` creates a dependency: if the named entry is removed or updated, the fallback entry is preserved. The compiler orders fallback entries after their primary in the boot menu.

`rescue = true` marks the entry as a rescue/recovery entry. The compiler may apply different security floor rules to rescue entries (e.g., allowing `enforcing = 0` when the system-level SELinux declaration is `enforcing`).

---

## bootc Integration

For bootc-managed systems, the bootloader declaration interacts with bootc's own entry management. The `bootc` block declares how Ironclad's bootloader configuration coexists with bootc's automatic entry generation:

```
bootloader systemd-boot "main" {
    target = uefi
    esp = storage.sda.efi

    bootc {
        manage_entries = true           # let bootc generate/update entries for image updates
        pin_default = "alma-custom"     # ironclad entry that remains default despite bootc updates
        rollback_entries = 2            # number of previous-generation entries bootc retains
    }

    # This entry is managed by the operator, not bootc
    entry "alma-custom" {
        uki {
            kernel = /boot/vmlinuz-custom
            cmdline { root = "UUID=xxxx"; ro = true }
            output = /boot/efi/EFI/Linux/alma-custom.efi
        }
    }
}
```

When `bootc.manage_entries = true`, the compiler does not generate entries for the current bootc image — bootc handles that. Operator-declared entries coexist with bootc-managed entries. When `false`, the compiler generates all entries and bootc does not modify the boot menu.

---

## Named Params Blocks

Kernel parameter sets can be declared at the bootloader scope (outside any entry) and referenced by multiple entries. This avoids duplication across entries sharing the same base parameters.

```
bootloader grub2 "main" {
    target = uefi
    esp = storage.sda.efi
    
    params "base" {
        root = "UUID=xxxx"
        ro = true
        selinux = 1
        enforcing = 1
        quiet = true
        rhgb = true
    }
    
    params "debug" {
        inherit = "base"
        quiet = remove
        rhgb = remove
        rd_break = false
        raw = ["systemd.log_level=debug", "loglevel=7"]
    }
    
    entry "alma" {
        kernel = /boot/vmlinuz-6.12.0
        initrd = /boot/initramfs-6.12.0.img
        params = params.base
    }
    
    entry "alma-debug" {
        kernel = /boot/vmlinuz-6.12.0
        initrd = /boot/initramfs-6.12.0.img
        params = params.debug
    }
}
```

Named params blocks support `inherit` to build parameter sets incrementally. This is separate from the class-level inheritance system — it is local to the bootloader declaration scope.

---

## Backend-Specific Configuration

For bootloader options that are specific to one backend and have no cross-backend equivalent, the `backend_config` block provides typed access:

```
bootloader grub2 "main" {
    backend_config {
        # GRUB2-specific /etc/default/grub values
        GRUB_DISABLE_SUBMENU = true
        GRUB_ENABLE_BLSCFG = false
        GRUB_SAVEDEFAULT = true
        GRUB_DISABLE_RECOVERY = false
        GRUB_PRELOAD_MODULES = ["part_gpt", "part_msdos", "luks2", "lvm"]
        GRUB_TERMINAL = "console"
    }
}

bootloader systemd-boot "main" {
    backend_config {
        # loader.conf values
        auto-entries = true
        auto-firmware = true
        editor = false          # disable kernel cmdline editing at boot
        console-mode = "max"
    }
}

bootloader refind "main" {
    backend_config {
        # refind.conf values
        scanfor = ["internal", "external", "optical"]
        also_scan_dirs = ["/boot", "/EFI/custom"]
        dont_scan_volumes = ["Recovery HD"]
        use_graphics_for = ["linux", "windows"]
        showtools = ["shell", "memtest", "firmware"]
    }
}
```

`backend_config` is a typed key-value block. The compiler validates that keys are real configuration options for the declared backend. Unknown keys are errors. For keys the compiler does not yet know about, the `raw_config` escape hatch is available (see below).

---

## Raw Escape Hatches

### raw_script

Arbitrary shell commands appended to the generated bootloader installation script. Use for anything the structured syntax cannot express.

```
bootloader grub2 "main" {
    raw_script = """
        # Custom GRUB module installation
        grub2-install --modules="part_gpt luks2 lvm" --target=x86_64-efi /dev/sda
        
        # Copy custom theme files
        cp -r /opt/custom-theme /boot/grub2/themes/custom
    """
}
```

The compiler appends `raw_script` content **after** all generated commands. No validation is performed on the content. The operator is responsible for correctness.

### raw_config

Arbitrary lines appended to the bootloader's primary configuration file.

```
bootloader grub2 "main" {
    raw_config = """
        # Custom GRUB config lines appended to grub.cfg
        set check_signatures=enforce
        trust (hd0,gpt1)/EFI/keys/gpg.key
    """
}
```

For `grub2`, this appends to `/etc/default/grub` or directly to `grub.cfg` depending on context. For `systemd-boot`, to `loader.conf`. For `refind`, to `refind.conf`.

### raw_entry

A fully hand-written boot entry in the backend's native format. The compiler includes it verbatim.

```
bootloader grub2 "main" {
    raw_entry "memtest" = """
        menuentry "Memtest86+" {
            linux /boot/memtest86+/memtest.bin
        }
    """
}
```

`raw_entry` entries are included in the boot menu alongside structured entries. The name (`"memtest"`) is used for ordering relative to structured entries but is not validated.

---

## Compiler Cross-Validation

The compiler performs the following validations across the bootloader declaration and other system declarations:

| Validation | Description |
|---|---|
| **ESP exists** | When `target` includes `uefi`, the `esp` reference must resolve to a valid FAT32 partition with type ef00 in the storage declaration. |
| **BIOS boot partition** | When `target` includes `bios` with a GPT disk, the compiler warns if no BIOS Boot Partition (type ef02) exists in the storage declaration. |
| **Root param matches storage** | If `params.root` is declared and the storage tree has a filesystem mounted at `/`, the compiler validates UUID/device consistency. |
| **SELinux param consistency** | `selinux` and `enforcing` params are validated against the system-level SELinux declaration. |
| **LUKS params match storage** | `rd_luks_uuid` and `rd_luks_name` are validated against LUKS containers in the storage tree. |
| **UKI output path writable** | `uki.output` must be on a filesystem the image build process can write to (typically the ESP). |
| **Backend supports target** | `systemd-boot` + `bios` is an error. `syslinux` + UEFI is a warning. |
| **Chainload path exists** | UEFI chainload `path` is validated as a plausible EFI binary path (warning, not error, since the target OS may not be installed yet). |
| **LUKS1 for GRUB boot encryption** | If the `/boot` filesystem is inside a LUKS container and the backend is `grub2`, the compiler warns if the container is `luks2` (GRUB2's LUKS2 support is limited) and suggests `luks1`. |
| **Entry name uniqueness** | All entry names (structured and raw) must be unique within the bootloader block. |
| **Default entry exists** | If `default` is declared, the named entry must exist. |
| **Fallback target exists** | `fallback_for` must reference an existing entry name. |

---

## Security Floor Enforcement

The system-level security floor applies to bootloader declarations:

- **Baseline:** No bootloader-specific enforcement.
- **Standard:** Warning if no `password` block is declared. Warning if `timeout = 0` with no default entry (unbootable system).
- **Strict:** Standard warnings become errors. `rescue` entries must still declare `selinux = 1`. `backend_config.editor` must be `false` for systemd-boot (prevents cmdline editing).
- **Maximum:** Strict rules plus: `password` block required. All entries must have `selinux = 1` and `enforcing = 1` (no override, even for rescue). UKI signing (`uki.sign`) required for all UKI entries. `raw_script`, `raw_config`, and `raw_entry` emit warnings (audit trail — the operator is bypassing validated configuration).

---

## Semicolon Shorthand

As with storage, simple entry blocks can use the inline form:

```
entry "rescue" { rescue = true; kernel = /boot/vmlinuz; initrd = /boot/initramfs.img; params = params.base }
```

---

## Reserved Keywords

The following words are reserved in bootloader context and cannot be used as block or entry names:

`bootloader`, `grub2`, `systemd-boot`, `refind`, `syslinux`, `limine`, `raw`, `entry`, `params`, `uki`, `chainload`, `console`, `serial`, `theme`, `password`, `bootc`, `backend_config`, `raw_script`, `raw_config`, `raw_entry`, `pcr`, `sign`, `remove`, `true`, `false`, `remaining`, `inherit`

---

## Grammar Summary (Informative)

This section provides an informal summary of the bootloader grammar. The canonical grammar is the PEG definition in the compiler source.

```
bootloader_decl     = "bootloader" backend name "{" bootloader_body "}"
backend             = "grub2" | "systemd-boot" | "refind" | "syslinux" | "limine" | "raw"
name                = quoted_string

bootloader_body     = property* block*
block               = entry_block | params_block | console_block | theme_block
                    | password_block | bootc_block | backend_config_block
                    | raw_script_decl | raw_config_decl | raw_entry_decl

entry_block         = "entry" name "{" entry_body "}"
entry_body          = property* (params_block | params_ref)? (uki_block | chainload_block)? options_block?

params_block        = "params" name? "{" param_property* "}"
param_property      = identifier "=" (value | "remove") | "raw" "=" array | "raw_remove" "=" array
params_ref          = "params" "=" dotted_reference

uki_block           = "uki" "{" uki_property* pcr_block? sign_block? "}"
pcr_block           = "pcr" "{" property* "}"
sign_block          = "sign" "{" property* "}"

chainload_block     = "chainload" "{" property* "}"

console_block       = "console" "{" property* serial_block? "}"
serial_block        = "serial" "{" property* "}"

theme_block         = "theme" "{" property* "}"
password_block      = "password" "{" property* "}"
bootc_block         = "bootc" "{" property* "}"
backend_config_block = "backend_config" "{" property* "}"

raw_script_decl     = "raw_script" "=" multiline_string
raw_config_decl     = "raw_config" "=" multiline_string
raw_entry_decl      = "raw_entry" name "=" multiline_string

property            = identifier "=" value
value               = string | number | boolean | array | dotted_reference
dotted_reference    = identifier ("." identifier)*
array               = "[" value ("," value)* "]"
multiline_string    = '"""' .* '"""'
```

---

## Full Example: Hardened AlmaLinux Server — Inline in Tree

```
system web01 {
    var root_uuid = "aaaa-bbbb-cccc-dddd"
    var luks_uuid = "1111-2222-3333-4444"
    var grub_hash = "grub.pbkdf2.sha512.10000.ABC123..."

    disk /dev/sda {
        label = gpt

        fat32 efi {
            size = 1G
            type = ef00
            mount = /boot/efi

            bootloader grub2 "main" {
                target = uefi
                install_device = /dev/sda
                timeout = 5
                timeout_style = menu
                default = "alma"

                password {
                    user = "root"
                    hash = ${grub_hash}
                    unrestricted = false
                }

                console {
                    terminal_input = [console, serial]
                    terminal_output = [console, serial]
                    serial { unit = 0; speed = 115200; word = 8; parity = no; stop = 1 }
                }

                backend_config {
                    GRUB_DISABLE_SUBMENU = true
                    GRUB_ENABLE_BLSCFG = false
                    GRUB_PRELOAD_MODULES = ["part_gpt", "luks2", "lvm"]
                }

                params "base" {
                    root = "UUID=${root_uuid}"
                    ro = true
                    selinux = 1
                    enforcing = 1
                    quiet = true
                    rhgb = true
                    rd_luks_uuid = "luks-${luks_uuid}"
                    rd_luks_name = "luks-${luks_uuid}=system"
                    raw = ["mitigations=auto", "iommu=pt"]
                }

                params "debug" {
                    inherit = "base"
                    quiet = remove
                    rhgb = remove
                    raw = ["systemd.log_level=debug", "loglevel=7"]
                    raw_remove = ["mitigations=auto"]
                }

                entry "alma" {
                    kernel = /boot/vmlinuz-6.12.0-0.rc1.alma
                    initrd = [
                        /boot/microcode.cpio,
                        /boot/initramfs-6.12.0-0.rc1.alma.img
                    ]
                    params = params.base
                    options { class = "linux" }
                }

                entry "alma-debug" {
                    kernel = /boot/vmlinuz-6.12.0-0.rc1.alma
                    initrd = [
                        /boot/microcode.cpio,
                        /boot/initramfs-6.12.0-0.rc1.alma.img
                    ]
                    params = params.debug
                    options { class = "linux" }
                }

                entry "alma-previous" {
                    fallback_for = "alma"
                    kernel = /boot/vmlinuz-6.11.0.alma
                    initrd = /boot/initramfs-6.11.0.alma.img
                    params = params.base
                    options { class = "linux" }
                }

                entry "rescue" {
                    rescue = true
                    kernel = /boot/vmlinuz-6.12.0-0.rc1.alma
                    initrd = /boot/initramfs-6.12.0-0.rc1.alma.img
                    params {
                        inherit = "base"
                        single = true
                        systemd_unit = "rescue.target"
                    }
                    options { class = "linux" }
                }
            }

            dir EFI/almalinux {
                file grubx64.efi { package = grub2-efi-x64 }
                file grub.cfg { mode = 0600; content = generated }
            }
        }

        luks2 system {
            lvm vg0 {
                xfs root {
                    size = 50G
                    mount = /
                }
                xfs var_data {
                    size = 20G
                    mount = /var
                }
                swap swap0 { size = 16G }
            }
        }
    }
}
```

## Full Example: Class-Based Bootloader with Function

```
# --- lib/bootloader.ic ---

func grub2_entry(name: string, kernel: path, initrd: path, params_ref: reference) {
    entry ${name} {
        kernel = ${kernel}
        initrd = ${initrd}
        params = ${params_ref}
        options { class = "linux" }
    }
}

class hardened_grub2 {
    description = "GRUB2 UEFI with password, serial console, hardened defaults"

    var grub_hash = ""      # must be overridden

    bootloader grub2 "main" {
        target = uefi
        timeout = 5
        timeout_style = menu

        password {
            user = "root"
            hash = ${grub_hash}
            unrestricted = false
        }

        console {
            terminal_input = [console, serial]
            terminal_output = [console, serial]
            serial { unit = 0; speed = 115200 }
        }

        backend_config {
            GRUB_DISABLE_SUBMENU = true
            GRUB_ENABLE_BLSCFG = false
            GRUB_PRELOAD_MODULES = ["part_gpt", "luks2", "lvm"]
        }
    }

    dir /boot/efi/EFI/almalinux {
        file grubx64.efi { package = grub2-efi-x64 }
        file grub.cfg { mode = 0600; content = generated }
    }
}

# --- systems/web01.ic ---

import "lib/bootloader.ic" { hardened_grub2, grub2_entry }

system web01 {
    var root_uuid = "aaaa-bbbb-cccc-dddd"
    var luks_uuid = "1111-2222-3333-4444"
    var grub_hash = "grub.pbkdf2.sha512.10000.ABC123..."

    apply hardened_grub2

    # Extend the bootloader from the class with system-specific entries
    bootloader grub2 "main" {
        default = "alma"

        params "base" {
            root = "UUID=${root_uuid}"
            ro = true
            selinux = 1
            enforcing = 1
            quiet = true
            rd_luks_uuid = "luks-${luks_uuid}"
            raw = ["mitigations=auto", "iommu=pt"]
        }

        embed func.grub2_entry("alma", /boot/vmlinuz-current, /boot/initramfs-current.img, params.base)
        embed func.grub2_entry("alma-previous", /boot/vmlinuz-previous, /boot/initramfs-previous.img, params.base)

        entry "rescue" {
            rescue = true
            kernel = /boot/vmlinuz-current
            initrd = /boot/initramfs-current.img
            params {
                inherit = "base"
                single = true
                systemd_unit = "rescue.target"
            }
        }
    }

    disk /dev/sda {
        label = gpt

        fat32 efi {
            size = 1G
            type = ef00
            mount = /boot/efi
            # hardened_grub2 class files and bootloader block
            # merge here via mount-point resolution
        }

        luks2 system {
            lvm vg0 {
                xfs root { size = 50G; mount = / }
                xfs var_data { size = 20G; mount = /var }
                swap swap0 { size = 16G }
            }
        }
    }
}
```

## Full Example: systemd-boot with UKI — Inline in Tree

```
system fedora_workstation {
    var root_uuid = "eeee-ffff-0000-1111"

    disk /dev/nvme0n1 {
        label = gpt

        fat32 efi {
            size = 1G
            type = ef00
            mount = /boot/efi

            bootloader systemd-boot "main" {
                target = uefi
                timeout = 3
                default = "fedora-uki"

                backend_config {
                    editor = false
                    auto-entries = false
                    auto-firmware = true
                    console-mode = "max"
                }

                entry "fedora-uki" {
                    uki {
                        kernel = /boot/vmlinuz-6.13.0-0.rc2.fc42
                        initrd = [
                            /boot/microcode.cpio,
                            /boot/initramfs-6.13.0-0.rc2.fc42.img
                        ]
                        cmdline {
                            root = "UUID=${root_uuid}"
                            ro = true
                            selinux = 1
                            enforcing = 1
                            quiet = true
                        }
                        os_release = /etc/os-release
                        output = /boot/efi/EFI/Linux/fedora-6.13.0.efi

                        pcr {
                            banks = [sha256]
                            sign = true
                            private_key = /etc/pki/uki/pcr.key
                            certificate = /etc/pki/uki/pcr.crt
                        }

                        sign {
                            tool = sbsign
                            key = /etc/pki/secureboot/db.key
                            certificate = /etc/pki/secureboot/db.crt
                        }
                    }
                }

                entry "fedora-previous" {
                    fallback_for = "fedora-uki"
                    uki {
                        kernel = /boot/vmlinuz-6.12.0-fc42
                        initrd = /boot/initramfs-6.12.0-fc42.img
                        cmdline {
                            root = "UUID=${root_uuid}"
                            ro = true
                            selinux = 1
                            enforcing = 1
                        }
                        output = /boot/efi/EFI/Linux/fedora-6.12.0.efi
                    }
                }

                entry "windows" {
                    chainload { path = /EFI/Microsoft/Boot/bootmgfw.efi }
                    options { class = "windows" }
                }
            }

            dir EFI/Linux {
                # UKI .efi files placed here by compiler from uki.output declarations
            }
        }

        luks2 system {
            lvm vg0 {
                btrfs root {
                    size = remaining
                    mount = /
                    subvol home { mount = /home }
                    subvol snapshots { mount = /.snapshots }
                }
            }
        }
    }
}
```

---

## What This Document Does Not Cover

This specification covers bootloader declaration syntax only. The following topics are defined in separate specifications:

- **Storage topology** — Block devices, encryption, volume management, filesystems (`STORAGE_SYNTAX.md`)
- **Filesystem declarations** — Files, directories, merge semantics (`FILESYSTEM_SYNTAX.md`)
- **Language primitives** — Variables, classes, functions, references, composition (`LANGUAGE_SYNTAX.md`)
- **SELinux policy** — SELinux mode, user/role/type, and MLS configuration (`SELINUX_SYNTAX.md`)
- **Secure Boot key management** — Key generation, enrollment, MOK management, and trust chains (future specification)
- **Kernel configuration** — Kernel build options, module loading, sysctl (future specification)
- **Compiler output mapping** — How declarations map to Kickstart `%post`, bash scripts, and backend configs
- **Runtime agent boot monitoring** — How drift detection applies to bootloader state
