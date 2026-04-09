# Ironclad Filesystem Declaration Syntax Specification

**Status:** Draft — syntax development, Phase 1  
**Scope:** File and directory declarations inside the storage topology, class projection into the tree, merge semantics, and conflict resolution

---

## Design Principles

Every file and directory on an Ironclad-built system exists inside the storage topology. The storage declaration (`STORAGE_SYNTAX.md`) defines block devices, encryption, volume management, and filesystems. This specification defines what goes *inside* those filesystems: directories, files, permissions, ownership, SELinux labels, and content.

The core rule:

> **The compiler's internal representation is the fully resolved filesystem tree.** Every file, every directory, every attribute has a concrete location inside a declared filesystem. There are two ways to author declarations that produce this tree — direct and pointed — but both resolve to the same internal form.

### Direct Authoring

Write file and directory declarations inline inside the storage topology, nested under the filesystem they belong to. What you write is what the compiler sees.

### Pointed Authoring (Classes)

Write file and directory declarations inside a class. The class declares paths. The compiler resolves those paths to the filesystem topology and inserts the declarations into the tree. The class is an ergonomic layer — the compiler reads the result the same as if the author had written everything inline.

### Mixed Authoring

Both modes can be used in the same system declaration. The compiler merges direct and pointed declarations into a single tree. Conflicts are resolved deterministically: **inline wins**, with a soft warning identifying the conflict and the compiler's resolution.

---

## Directory Declarations

Directories are declared with the `dir` keyword inside a filesystem block or inside another `dir` block. Nesting mirrors the real directory hierarchy.

```
xfs root {
    size = 50G
    mount = /

    dir etc {
        owner = root
        group = root
        mode = 0755

        dir httpd {
            dir conf {
                mode = 0755
            }
            dir conf.d {
                mode = 0755
            }
        }

        dir ssh {
            mode = 0755
            selinux = ssh_home_t
        }
    }

    dir var {
        dir log {
            mode = 0755

            dir httpd {
                owner = root
                group = root
                mode = 0700
                selinux = httpd_log_t
            }
        }
    }
}
```

### Directory Properties

| Property   | Default      | Description                                                    |
|------------|--------------|----------------------------------------------------------------|
| `owner`    | `root`       | User owner. Validated against the system's user declarations.  |
| `group`    | `root`       | Group owner. Validated against the system's group declarations.|
| `mode`     | `0755`       | Octal permission mode.                                         |
| `selinux`  | (inherited)  | SELinux type label. Validated against SELinux policy declaration. When omitted, the compiler infers from the SELinux file context specification or inherits from the parent. |
| `recursive`| `false`      | When `true`, declared `owner`, `group`, `mode`, and `selinux` apply to all children that do not override them. |
| `state`    | `present`    | `present` or `absent`. `absent` ensures the directory does not exist (useful for overriding base class defaults). |

### Path Shorthand

Deeply nested paths can be declared with slash-separated shorthand instead of nested blocks:

```
# These are equivalent:
dir etc/httpd/conf {
    mode = 0755
}

dir etc {
    dir httpd {
        dir conf {
            mode = 0755
        }
    }
}
```

When using path shorthand, only the final directory in the path receives the declared properties. Intermediate directories are created with default properties. If an intermediate directory needs explicit properties, it must be declared separately — either by expanding the nesting or by declaring it elsewhere in the tree (the compiler merges).

---

## File Declarations

Files are declared with the `file` keyword inside a `dir` block or directly inside a filesystem block (with a path).

```
dir etc/httpd/conf {
    file httpd.conf {
        owner = root
        group = root
        mode = 0644
        selinux = httpd_config_t

        content = """
            ServerRoot "/etc/httpd"
            Listen 80
            Include conf.modules.d/*.conf
            User apache
            Group apache
        """
    }
}
```

### File Properties

| Property    | Default      | Description                                                          |
|-------------|--------------|----------------------------------------------------------------------|
| `owner`     | `root`       | User owner.                                                          |
| `group`     | `root`       | Group owner.                                                         |
| `mode`      | `0644`       | Octal permission mode.                                               |
| `selinux`   | (inherited)  | SELinux type label.                                                  |
| `state`     | `present`    | `present`, `absent`, or `symlink`.                                   |
| `content`   | (none)       | Inline file content as a multiline string.                           |
| `source`    | (none)       | Path to a source file copied into the image at build time.           |
| `template`  | (none)       | Path to a template file processed with variable substitution.        |
| `package`   | (none)       | Declares that this file is provided by a package — do not overwrite. |
| `symlink`   | (none)       | Symlink target. Sets `state = symlink` implicitly.                   |
| `immutable` | `false`      | Sets the `+i` attribute (chattr). Runtime agent monitors for changes.|
| `append_only` | `false`    | Sets the `+a` attribute. Useful for log files.                       |
| `hash`      | (none)       | Expected content hash (sha256). Compiler validates `content`/`source` against this. Runtime agent uses it for drift detection. Auto-computed if omitted. |

### Content Sources

A file must have exactly one content source: `content`, `source`, `template`, or `package`. Declaring more than one is a compile error.

**`content` — Inline content:**

```
file resolv.conf {
    mode = 0644
    content = """
        nameserver 10.0.0.1
        nameserver 10.0.0.2
        search example.com
    """
}
```

The compiler strips consistent leading whitespace from multiline strings (dedent). Content is written verbatim to the image.

**`source` — Copy from build context:**

```
file httpd.conf {
    mode = 0644
    source = files/httpd/httpd.conf    # path relative to Ironclad source tree
}
```

The compiler copies the source file into the image during build. The source file is included in the intermediate manifest hash.

**`template` — Variable-substituted source:**

```
file httpd.conf {
    mode = 0644
    template = templates/httpd/httpd.conf.tpl
}
```

Template processing is defined in the variables/templating specification (future document). Templates reference Ironclad variables and are resolved at compile time. The resolved content is written to the image.

**`package` — Package-managed file:**

```
file /usr/lib/systemd/system/httpd.service {
    package = httpd
    selinux = systemd_unit_file_t
}
```

`package` declares that the file is provided by the named package and should not be overwritten by the compiler. The compiler validates that the package is in the system's package list. Properties like `selinux`, `owner`, `group`, and `mode` are still applied — they declare expected state for the runtime agent to verify, even though the content comes from the package.

### Symlinks

```
file latest.log {
    symlink = /var/log/httpd/access.log
}

# Shorthand
file latest.log { symlink = /var/log/httpd/access.log }
```

The compiler validates that the symlink target resolves to a declared path (warning if not — the target might be created at runtime).

---

## File Tree Nesting Inside Storage

File and directory declarations nest inside filesystem blocks in the storage topology. The mount point of the filesystem establishes the root of the file tree.

```
disk /dev/sda {
    label = gpt

    fat32 efi {
        size = 1G
        type = ef00
        mount = /boot/efi

        dir EFI {
            dir BOOT {
                file BOOTX64.EFI {
                    source = bootloader/BOOTX64.EFI
                    mode = 0700
                }
            }
            dir almalinux {
                file grubx64.efi {
                    package = grub2-efi-x64
                }
            }
        }
    }

    luks2 system {
        lvm vg0 {
            xfs root {
                size = 50G
                mount = /

                dir etc {
                    file hostname {
                        mode = 0644
                        content = "web01.example.com"
                    }

                    dir httpd/conf {
                        file httpd.conf {
                            mode = 0644
                            selinux = httpd_config_t
                            source = files/httpd.conf
                        }
                    }

                    dir ssh {
                        mode = 0755
                        file sshd_config {
                            mode = 0600
                            selinux = etc_t
                            source = files/sshd_config
                        }
                    }
                }

                dir usr/lib/systemd/system {
                    file httpd.service {
                        package = httpd
                        selinux = systemd_unit_file_t
                    }
                }
            }

            xfs var_data {
                size = 20G
                mount = /var

                dir log/httpd {
                    owner = root
                    group = root
                    mode = 0700
                    selinux = httpd_log_t
                }

                dir www/html {
                    owner = apache
                    group = apache
                    mode = 0755
                    selinux = httpd_sys_content_t

                    file index.html {
                        mode = 0644
                        content = "<h1>Ironclad</h1>"
                    }
                }
            }
        }
    }
}
```

The compiler knows that `dir etc` inside the `xfs root` block (mounted at `/`) produces `/etc`. `dir log/httpd` inside `xfs var_data` (mounted at `/var`) produces `/var/log/httpd`. Absolute paths are always computable from the tree structure.

---

## Class Projection (Pointed Authoring)

Classes declare files and directories with absolute paths. The compiler resolves each path to the storage topology by matching against declared mount points.

### Class Declaration

```
class httpd_server {
    description = "Apache HTTP server with standard hardened configuration"

    dir /etc/httpd/conf {
        mode = 0755
    }

    dir /etc/httpd/conf.d {
        mode = 0755
    }

    file /etc/httpd/conf/httpd.conf {
        owner = root
        group = root
        mode = 0644
        selinux = httpd_config_t
        source = files/httpd/httpd.conf
    }

    file /usr/lib/systemd/system/httpd.service {
        package = httpd
        selinux = systemd_unit_file_t
    }

    dir /var/log/httpd {
        owner = root
        group = root
        mode = 0700
        selinux = httpd_log_t
    }

    dir /var/www/html {
        owner = apache
        group = apache
        mode = 0755
        selinux = httpd_sys_content_t
    }
}
```

### Applying a Class

```
system alma_web {
    apply httpd_server
}
```

When the compiler processes `apply httpd_server`, it takes every `dir` and `file` declaration in the class and resolves its absolute path against the storage topology's mount points:

- `/etc/httpd/conf/httpd.conf` → the filesystem mounted at `/` (longest prefix match) → inserted into that filesystem's tree under `etc/httpd/conf/`
- `/var/log/httpd` → if `/var` has its own filesystem, resolves there; if not, falls through to `/` → inserted accordingly
- If no mount point covers a path → **compile error**: "file /some/path declared in class httpd_server has no backing filesystem in the storage declaration"

The compiler performs longest-prefix mount point matching, identical to how the kernel resolves paths to filesystems.

### Class Inheritance

Classes support single inheritance, consistent with the system-level class model:

```
class base_server {
    file /etc/ssh/sshd_config {
        mode = 0600
        source = files/sshd_config_hardened
    }

    dir /var/log {
        mode = 0755
    }
}

class httpd_server extends base_server {
    # Inherits sshd_config and /var/log from base_server
    # Adds httpd-specific declarations

    file /etc/httpd/conf/httpd.conf {
        mode = 0644
        source = files/httpd.conf
    }
}
```

A derived class can override any property of an inherited file or directory:

```
class locked_httpd extends httpd_server {
    file /etc/httpd/conf/httpd.conf {
        mode = 0400          # tighten permissions
        immutable = true     # prevent runtime modification
    }
}
```

The override replaces only the declared properties. Undeclared properties are inherited from the parent. This is property-level merge, not block-level replacement.

---

## Merge Semantics and Conflict Resolution

When the same file or directory path is declared by multiple sources — inline in the tree, from one class, from multiple classes — the compiler merges them into a single declaration using deterministic rules.

### Merge Order (Highest to Lowest Priority)

1. **Inline tree declarations** — written directly in the storage topology
2. **Applied classes** — in the order they appear in the `apply` list (later overrides earlier)
3. **Inherited class members** — from the class inheritance chain (derived overrides base)

### Property-Level Merge

Merge happens at the property level, not the block level. If two sources declare the same file, their properties are merged. A higher-priority source's properties override a lower-priority source's properties. Properties only declared by the lower-priority source are preserved.

```
# Class declares:
file /etc/httpd/conf/httpd.conf {
    owner = root
    group = root
    mode = 0644
    selinux = httpd_config_t
    source = files/httpd.conf
}

# Inline tree declares:
file httpd.conf {
    mode = 0600
}

# Compiler resolves to:
# /etc/httpd/conf/httpd.conf {
#     owner = root           ← from class (no inline override)
#     group = root           ← from class (no inline override)
#     mode = 0600            ← from inline (overrides class)
#     selinux = httpd_config_t ← from class (no inline override)
#     source = files/httpd.conf ← from class (no inline override)
# }
```

### Conflict Warnings

When a property conflict occurs (same property declared by multiple sources with different values), the compiler:

1. **Resolves deterministically** — inline wins over class, later class wins over earlier class
2. **Emits a soft warning** identifying:
   - The file path
   - The property name
   - The winning value and its source
   - The losing value and its source
   - The resolution rule applied

Example compiler output:

```
WARNING [merge-conflict] /etc/httpd/conf/httpd.conf
  property: mode
  resolved: 0600 (inline, storage.sda.system.vg0.root:/etc/httpd/conf/httpd.conf)
  overridden: 0644 (class httpd_server, line 12)
  rule: inline declaration wins over class declaration
```

This warning is informational. The build succeeds. The operator can:
- Accept the override (the warning is expected)
- Remove the inline declaration (defer to the class)
- Remove the class declaration (the inline is the intended value)
- Suppress the warning for a specific path with `suppress_merge_warning = true` on the winning declaration

### Conflict Between Multiple Classes

When two classes both declare the same path:

```
system alma_web {
    apply base_hardening     # declares /etc/ssh/sshd_config { mode = 0600 }
    apply compliance_overlay # declares /etc/ssh/sshd_config { mode = 0400 }
}
```

The later `apply` wins. The compiler emits:

```
WARNING [merge-conflict] /etc/ssh/sshd_config
  property: mode
  resolved: 0400 (class compliance_overlay, line 8)
  overridden: 0600 (class base_hardening, line 15)
  rule: later apply declaration wins (apply order: base_hardening < compliance_overlay)
```

### Content Source Conflicts

If two sources declare different content sources (`content` vs `source`, or two different `source` paths), this is a **hard warning** (elevated severity) because it means the file's actual content is ambiguous, not just metadata:

```
HARD WARNING [merge-conflict-content] /etc/httpd/conf/httpd.conf
  property: source
  resolved: files/httpd_locked.conf (inline)
  overridden: files/httpd.conf (class httpd_server, line 12)
  rule: inline declaration wins over class declaration
  note: file content source conflict — verify intended content
```

Hard warnings are still not errors. The build succeeds. But they can be promoted to errors with a compiler flag (`--strict-merge` or equivalent).

---

## The `absent` State

Files and directories can be declared `absent` to ensure they do not exist. This is primarily useful in derived classes or inline overrides that need to remove something a base class creates.

```
class base_server {
    file /etc/motd {
        mode = 0644
        content = "Authorized users only."
    }
}

class silent_server extends base_server {
    file /etc/motd {
        state = absent     # removes the file declared by base_server
    }
}
```

In the inline tree:

```
xfs root {
    mount = /
    dir etc {
        file motd { state = absent }
    }
}
```

The compiler emits a removal command in the generated script. The runtime agent verifies the file remains absent.

---

## Executable Files and Scripts

Files that need to be executable follow the same declaration model with appropriate mode:

```
dir usr/local/bin {
    file health_check.sh {
        mode = 0755
        owner = root
        group = root
        selinux = bin_t
        content = """
            #!/bin/bash
            set -euo pipefail
            curl -sf http://localhost/health || exit 1
        """
    }
}
```

For scripts referenced by services, init systems, or cron, the file declaration is the canonical location. Service declarations (future specification) reference the file by its tree path.

---

## Directory-Level Defaults

A `dir` block can declare default properties that apply to all child files and directories unless overridden:

```
dir etc/httpd {
    owner = root
    group = apache
    selinux = httpd_config_t
    defaults = true              # children inherit these properties

    dir conf {
        # inherits owner=root, group=apache, selinux=httpd_config_t
        file httpd.conf {
            mode = 0644
            # inherits owner, group, selinux from parent dir
            source = files/httpd.conf
        }
    }

    dir conf.d {
        file ssl.conf {
            mode = 0644
            source = files/ssl.conf
        }
    }
}
```

When `defaults = true` is set on a directory, its `owner`, `group`, `selinux`, and `mode` properties become defaults for all descendants. Any child can override any property. The `defaults` property does not propagate — a child directory must set `defaults = true` independently if it wants to establish a new default scope.

---

## Raw Escape Hatch

For file operations that the structured syntax cannot express, the `raw_commands` block injects shell commands at a specific point in the tree:

```
dir opt/custom {
    raw_commands = """
        # Extract a tarball that creates its own directory structure
        tar xzf /tmp/custom-app.tar.gz -C /opt/custom
        chown -R appuser:appgroup /opt/custom/app
    """
}
```

The compiler inserts `raw_commands` into the generated script at the point in execution order corresponding to the block's position in the tree. No validation is performed on the content. The runtime agent cannot verify files created by `raw_commands` unless they are separately declared with expected properties.

---

## Compiler Cross-Validation

| Validation | Description |
|---|---|
| **Path resolves to filesystem** | Every absolute path (from classes) must resolve to a declared mount point via longest-prefix match. |
| **Owner/group exist** | `owner` and `group` values are validated against the system's user and group declarations. |
| **SELinux type exists** | `selinux` values are validated against the SELinux policy declaration. |
| **Single content source** | A file must have at most one of `content`, `source`, `template`, `package`. |
| **Source file exists** | `source` paths are validated against the Ironclad source tree at compile time. |
| **Package exists** | `package` values are validated against the system's package list. |
| **Symlink target plausible** | Symlink targets are checked against the declared tree (warning if not found — may be runtime-created). |
| **Mode sanity** | World-writable files (`xx7`) and world-writable directories without sticky bit emit warnings. |
| **Immutable + content** | Files marked `immutable = true` must have a content source — an immutable file with no declared content is likely a mistake. |
| **Parent directory exists** | A file declaration implies its parent directory exists. The compiler auto-creates intermediate directories with default properties (and emits an info-level message) or errors if the parent is declared `absent`. |

---

## Security Floor Enforcement

| Level | Enforcement |
|---|---|
| **Baseline** | No file-level enforcement. |
| **Standard** | Warning for world-writable files outside `/tmp` and `/var/tmp`. Warning for SUID/SGID files not in the standard system set. |
| **Strict** | Standard warnings become errors. All files in `/etc` must have explicit `owner` and `mode` (no reliance on defaults). |
| **Maximum** | Strict rules plus: every file must have an explicit `selinux` label (no compiler inference). `raw_commands` emits a warning. No world-readable files in `/etc/ssl`, `/etc/ssh`, or `/etc/pki`. |

---

## Reserved Keywords

The following words are reserved in filesystem context:

`dir`, `file`, `content`, `source`, `template`, `package`, `symlink`, `owner`, `group`, `mode`, `selinux`, `state`, `present`, `absent`, `immutable`, `append_only`, `hash`, `defaults`, `recursive`, `raw_commands`, `suppress_merge_warning`, `remove`, `true`, `false`

---

## Grammar Summary (Informative)

```
dir_block       = "dir" path "{" dir_body "}"
dir_body        = property* (dir_block | file_block | raw_commands_decl)*

file_block      = "file" filename "{" file_body "}"
file_body       = property*

path            = identifier ("/" identifier)*
filename        = identifier ("." identifier)*

class_decl      = "class" name ("extends" name)? "{" class_body "}"
class_body      = property* (dir_block | file_block)*

apply_stmt      = "apply" dotted_reference

property        = identifier "=" value
value           = string | number | boolean | multiline_string | array | dotted_reference
multiline_string = '"""' .* '"""'

raw_commands_decl = "raw_commands" "=" multiline_string
```

---

## Full Example: Merged Inline + Class

```
# --- Class definition (separate file or same source tree) ---

class hardened_base {
    file /etc/ssh/sshd_config {
        owner = root
        group = root
        mode = 0600
        selinux = etc_t
        source = files/sshd_config_hardened
    }

    file /etc/issue {
        mode = 0644
        content = "Authorized access only. All activity is monitored."
    }

    file /etc/issue.net {
        mode = 0644
        content = "Authorized access only. All activity is monitored."
    }

    dir /var/log/audit {
        owner = root
        group = root
        mode = 0700
        selinux = auditd_log_t
    }
}

class httpd_server extends hardened_base {
    file /etc/httpd/conf/httpd.conf {
        owner = root
        group = root
        mode = 0644
        selinux = httpd_config_t
        source = files/httpd/httpd.conf
    }

    dir /var/log/httpd {
        owner = root
        group = root
        mode = 0700
        selinux = httpd_log_t
    }

    dir /var/www/html {
        owner = apache
        group = apache
        mode = 0755
        selinux = httpd_sys_content_t
    }
}

# --- Storage + inline file tree ---

disk /dev/sda {
    label = gpt

    fat32 efi {
        size = 1G
        type = ef00
        mount = /boot/efi
    }

    luks2 system {
        lvm vg0 {
            xfs root {
                size = 50G
                mount = /

                # Inline override: tighten sshd_config beyond class default
                dir etc/ssh {
                    file sshd_config {
                        mode = 0400          # stricter than class's 0600
                        immutable = true     # prevent runtime changes
                    }
                    # WARNING emitted: mode conflict, inline 0400 wins over class 0600
                }

                # Inline-only file: not in any class
                dir etc {
                    file hostname {
                        mode = 0644
                        content = "web01.example.com"
                    }
                }
            }

            xfs var_data {
                size = 20G
                mount = /var

                # Inline addition alongside class-provided /var/www/html
                dir www/html {
                    file index.html {
                        mode = 0644
                        owner = apache
                        group = apache
                        content = "<h1>Ironclad</h1>"
                    }
                }
            }
        }
    }
}

# --- System composition ---

system web01 {
    apply httpd_server
}
```

**Compiler resolution for `/etc/ssh/sshd_config`:**

```
# Final resolved state:
# /etc/ssh/sshd_config {
#     owner = root           ← class hardened_base
#     group = root           ← class hardened_base
#     mode = 0400            ← inline (wins over class 0600, WARNING emitted)
#     selinux = etc_t        ← class hardened_base
#     source = files/sshd_config_hardened  ← class hardened_base
#     immutable = true       ← inline (no conflict, only declared here)
# }
```

---

## What This Document Does Not Cover

This specification covers file and directory declaration syntax and merge semantics. The following topics are defined in separate specifications:

- **Storage topology** — Block devices, encryption, volume management, filesystems (`STORAGE_SYNTAX.md`)
- **Bootloader configuration** — Boot entries, kernel parameters, UKI bundling (`BOOTLOADER_SYNTAX.md`)
- **SELinux policy** — Policy mode, types, roles, MLS parameters (`SELINUX_SYNTAX.md`)
- **Variables and templating** — Variable declarations, template processing, parameterized classes (future specification)
- **Loops and conditionals** — Control flow for generating repetitive declarations (future specification)
- **Services and init systems** — Service declarations that reference files in the tree (future specification)
- **Users and groups** — User/group declarations that `owner`/`group` properties validate against (future specification)
- **Packages** — Package list declarations that `package` properties validate against (future specification)
- **Class system details** — Full class inheritance rules, multi-file class organization (future specification)
- **Compiler output mapping** — How file declarations map to Containerfile, Kickstart, and Ansible (future specification)
- **Runtime agent file monitoring** — Drift detection for declared files (future specification)
