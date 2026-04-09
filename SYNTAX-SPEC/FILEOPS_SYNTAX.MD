# Ironclad File Operations and Raw Encoding Syntax Specification

**Status:** Draft — syntax development, Phase 1  
**Scope:** Direct bash execution, file editing operations, structured format manipulation (YAML, TOML, JSON, INI, XML, properties, plain text), find-and-replace, line operations, patch application, and binary file handling  
**Dependencies:** `LANGUAGE_SYNTAX.md`, `FILESYSTEM_SYNTAX.md`

---

## Design Principles

Ironclad's structured DSL covers the common cases: declaring files with known content, setting permissions, applying SELinux labels. But Linux system configuration is a landscape of dozens of file formats, package-provided configs that need surgical edits rather than wholesale replacement, and situations where the right answer is a shell command.

This specification defines the mechanisms for reaching outside the structured model. They are organized from most structured (safest, most validatable) to least structured (most flexible, least validatable):

1. **Format-aware editors** — Modify YAML, TOML, JSON, INI, XML, and other structured files by key path. The compiler understands the format and validates the operation.

2. **Text operations** — Find-and-replace, line insertion, line deletion, append, prepend. The compiler emits the operations but cannot validate the result semantically.

3. **Raw bash** — Arbitrary shell commands. The compiler emits them verbatim. No validation. The operator owns correctness entirely.

The guiding principle: **use the most structured mechanism that solves the problem.** Format-aware editing is preferred over text replacement. Text replacement is preferred over raw bash. Raw bash is the last resort. The compiler does not enforce this — it is a convention. But security floor levels can warn or error on raw operations (see Security Floor Enforcement below).

---

## Format-Aware File Editors

When a file's format is known, Ironclad provides declarative key-path editing. The compiler parses the target format, applies the declared operations, and validates that the result is well-formed. This is the preferred mechanism for modifying package-provided configuration files.

### General Syntax

```
file sshd_config {
    package = openssh-server
    
    edit {
        format = sshd_config            # format identifier
        
        set PermitRootLogin = "no"
        set PasswordAuthentication = "no"
        set X11Forwarding = "no"
        set MaxAuthTries = 3
        set AllowUsers = ["deploy", "admin"]
        
        delete GSSAPIAuthentication
        
        ensure_absent UseDNS
    }
}
```

The `edit` block declares operations against a file's content. The file must have a content source (`package`, `source`, `content`, or `template`) — `edit` modifies that content. `edit` without a content source is a compile error (you can't edit a file that doesn't exist).

`edit` operations are applied in declaration order after the base content is loaded.

### Supported Format Identifiers

| Format ID          | File Types                                     | Path Syntax                |
|--------------------|-------------------------------------------------|----------------------------|
| `yaml`             | `.yaml`, `.yml`                                 | Dotted key path            |
| `toml`             | `.toml`                                         | Dotted key path            |
| `json`             | `.json`                                         | Dotted key path            |
| `ini`              | `.ini`, `.conf` (INI-style)                     | `[section].key`            |
| `xml`              | `.xml`                                          | XPath-like path            |
| `properties`       | `.properties`, Java-style                       | Key name                   |
| `keyvalue`         | Generic `key = value` or `key: value` files     | Key name                   |
| `sshd_config`      | OpenSSH config format                           | Directive name             |
| `systemd_unit`     | systemd `.service`, `.timer`, `.mount`, etc.     | `[Section].Key`            |
| `grub_default`     | `/etc/default/grub`                             | Variable name              |
| `fstab`            | `/etc/fstab`                                    | Device or mount point      |
| `sudoers`          | `/etc/sudoers`, `/etc/sudoers.d/*`              | Specialized (see below)    |
| `pam`              | PAM stack files (`/etc/pam.d/*`)                | Specialized (see below)    |
| `crontab`          | Crontab files                                   | Specialized (see below)    |
| `plain`            | Unstructured text                               | Line-based operations only |
| `csv`              | Comma/delimiter-separated values                | Row/column operations      |

New formats can be added to the compiler without grammar changes — each format is a parser plugin.

---

### YAML

```
file /etc/kubernetes/kubelet-config.yaml {
    package = kubelet

    edit {
        format = yaml

        set apiVersion = "kubelet.config.k8s.io/v1beta1"
        set kind = "KubeletConfiguration"
        set authentication.webhook.enabled = true
        set authentication.anonymous.enabled = false
        set authorization.mode = "Webhook"
        set readOnlyPort = 0
        set serverTLSBootstrap = true
        
        # Nested object creation — intermediate keys created if missing
        set logging.verbosity = 2
        
        # Array operations
        set clusterDNS = ["10.96.0.10"]
        append clusterDNS = "10.96.0.11"
        
        # Delete a key entirely
        delete featureGates.RotateKubeletServerCertificate

        # Set only if key doesn't already exist (don't override package default)
        set_default maxPods = 110
    }
}
```

**Path syntax:** Dotted paths traverse nested maps. `authentication.webhook.enabled` reaches `authentication: { webhook: { enabled: ... } }`. Array elements are addressed by index: `clusterDNS.0` is the first element.

### TOML

```
file /etc/containers/registries.conf {
    package = containers-common

    edit {
        format = toml

        set unqualified-search-registries = ["registry.access.redhat.com", "docker.io"]
        
        # Table (section) creation
        set registry."registry.example.com".blocked = false
        set registry."registry.example.com".insecure = false
        set registry."registry.example.com".location = "registry.example.com"
        
        # Nested table
        set registry."registry.example.com".mirror.0.location = "mirror.internal.example.com"
    }
}
```

### JSON

```
file /etc/docker/daemon.json {
    source = files/docker/daemon-base.json

    edit {
        format = json

        set storage-driver = "overlay2"
        set log-driver = "json-file"
        set log-opts."max-size" = "10m"
        set log-opts."max-file" = "3"
        set dns = ["10.0.0.1", "10.0.0.2"]
        set default-ulimits.nofile.Name = "nofile"
        set default-ulimits.nofile.Hard = 65536
        set default-ulimits.nofile.Soft = 65536
        
        delete insecure-registries
    }
}
```

### INI

```
file /etc/my.cnf.d/server.cnf {
    package = mariadb-server

    edit {
        format = ini

        set [mysqld].innodb_buffer_pool_size = "4G"
        set [mysqld].max_connections = 500
        set [mysqld].log_error = "/var/log/mariadb/error.log"
        set [mysqld].slow_query_log = 1
        set [mysqld].long_query_time = 2
        
        set [galera].wsrep_on = "ON"
        set [galera].wsrep_cluster_name = "prod_cluster"
        
        # Create a new section
        set [mysqldump].quick = true
        set [mysqldump].max_allowed_packet = "256M"
        
        delete [mysqld].skip_networking
    }
}
```

### XML

```
file /etc/libvirt/qemu.conf {
    package = libvirt

    edit {
        format = xml
        
        # XPath-like addressing
        set /domain/memory = 4194304
        set /domain/memory.@unit = "KiB"
        set /domain/vcpu = 4
        set /domain/os/type = "hvm"
        set /domain/os/type.@arch = "x86_64"
        
        # Add a child element
        add /domain/devices << """
            <interface type='bridge'>
                <source bridge='br0'/>
                <model type='virtio'/>
            </interface>
        """

        delete /domain/devices/graphics
    }
}
```

XML path syntax uses `/` delimiters. Attributes are accessed with `.@attrname`. The `add` keyword with `<<` inserts raw XML as a child of the addressed element.

### systemd Unit Files

```
file /usr/lib/systemd/system/httpd.service {
    package = httpd

    edit {
        format = systemd_unit

        set [Service].LimitNOFILE = 65536
        set [Service].PrivateTmp = "true"
        set [Service].ProtectSystem = "strict"
        set [Service].ReadWritePaths = "/var/log/httpd"
        
        # Drop-in style: append to existing values
        append [Service].ReadWritePaths = "/var/www"
        
        set [Unit].After = ["network.target", "remote-fs.target"]
        
        delete [Install].Also
    }
}
```

Note: For systemd, the compiler can also emit drop-in files instead of modifying the package unit directly. The `dropin` keyword generates a file in the unit's `.d/` directory:

```
file /etc/systemd/system/httpd.service.d/override.conf {
    dropin_for = /usr/lib/systemd/system/httpd.service
    
    edit {
        format = systemd_unit
        
        set [Service].LimitNOFILE = 65536
        set [Service].PrivateTmp = "true"
    }
}
```

### OpenSSH Config

```
file /etc/ssh/sshd_config {
    package = openssh-server

    edit {
        format = sshd_config

        set PermitRootLogin = "no"
        set PasswordAuthentication = "no"
        set PubkeyAuthentication = "yes"
        set X11Forwarding = "no"
        set MaxAuthTries = 3
        set ClientAliveInterval = 300
        set ClientAliveCountMax = 2
        set AllowUsers = ["deploy", "admin"]
        set LogLevel = "VERBOSE"
        
        # Match blocks
        match "User deploy" {
            set AllowTcpForwarding = "yes"
            set PermitTunnel = "yes"
        }

        match "User admin" {
            set ForceCommand = "/usr/local/bin/admin-shell"
        }

        ensure_absent PermitEmptyPasswords
        ensure_absent ChallengeResponseAuthentication
    }
}
```

### GRUB Defaults

```
file /etc/default/grub {
    package = grub2-common

    edit {
        format = grub_default

        set GRUB_TIMEOUT = 5
        set GRUB_DISABLE_SUBMENU = "true"
        set GRUB_CMDLINE_LINUX = "selinux=1 enforcing=1 audit=1"
        set GRUB_ENABLE_BLSCFG = "false"
        
        append GRUB_CMDLINE_LINUX = "mitigations=auto"    # appends to existing value
        
        delete GRUB_TERMINAL_OUTPUT
    }
}
```

### Sudoers

Sudoers has a unique format with real security implications. The compiler validates syntax before writing.

```
file /etc/sudoers.d/deploy {
    mode = 0440
    owner = root
    group = root

    edit {
        format = sudoers

        # User privilege specification
        grant deploy {
            hosts = ALL
            runas = root
            commands = [
                "/usr/bin/systemctl restart httpd",
                "/usr/bin/systemctl reload httpd",
                "/usr/bin/journalctl -u httpd"
            ]
            nopasswd = true
        }

        grant "%wheel" {
            hosts = ALL
            runas = ALL
            commands = ALL
            nopasswd = false
        }

        # Defaults
        default "Defaults" {
            env_reset = true
            secure_path = "/sbin:/bin:/usr/sbin:/usr/bin"
            logfile = "/var/log/sudo.log"
        }
    }
}
```

The compiler runs `visudo -c` equivalent validation on the generated output.

### PAM

```
file /etc/pam.d/sshd {
    package = openssh-server

    edit {
        format = pam

        # Insert a rule before the first auth rule
        insert_before "auth" {
            type = auth
            control = required
            module = pam_faillock.so
            args = ["preauth", "silent", "deny=5", "unlock_time=900"]
        }

        # Append after all account rules
        insert_after "account" {
            type = account
            control = required
            module = pam_faillock.so
        }

        # Remove a specific module line
        remove_module pam_gdm.so
        
        # Replace a module's arguments
        update_module pam_unix.so {
            args = ["sha512", "shadow", "nullok", "try_first_pass"]
        }
    }
}
```

### Crontab

```
file /etc/cron.d/maintenance {
    mode = 0644

    edit {
        format = crontab

        job "log_rotation" {
            minute = 0
            hour = 2
            day_of_month = "*"
            month = "*"
            day_of_week = "*"
            user = root
            command = "/usr/sbin/logrotate /etc/logrotate.conf"
        }

        job "backup" {
            schedule = "0 3 * * 0"      # shorthand alternative
            user = root
            command = "/usr/local/bin/backup.sh"
        }

        # Environment variables for the crontab
        env MAILTO = "admin@example.com"
        env PATH = "/usr/local/sbin:/usr/local/bin:/sbin:/bin:/usr/sbin:/usr/bin"
    }
}
```

### CSV / Delimited Files

```
file /etc/ethers {
    edit {
        format = csv
        delimiter = " "         # space, tab, comma, etc.
        
        set_row { match_column = 1; match_value = "00:11:22:33:44:55"; column = 2; value = "10.0.0.100" }
        append_row = ["00:AA:BB:CC:DD:EE", "10.0.0.200"]
        delete_row { match_column = 2; match_value = "10.0.0.99" }
    }
}
```

---

## Edit Operations Reference

All format-aware editors support a common set of operations. Not all operations are applicable to all formats — the compiler errors on inapplicable operations.

| Operation        | Description                                                                  | Formats       |
|------------------|------------------------------------------------------------------------------|---------------|
| `set`            | Set a key/path to a value. Creates intermediate keys if missing.             | All structured|
| `set_default`    | Set only if the key does not already exist. Preserves package defaults.      | All structured|
| `delete`         | Remove a key/path entirely.                                                  | All structured|
| `ensure_absent`  | Same as `delete` but no error if the key doesn't exist.                      | All structured|
| `append`         | Append a value to an existing key (string concatenation or array push).      | All structured|
| `prepend`        | Prepend a value to an existing key.                                          | String-valued |
| `replace_value`  | Find and replace within a key's value (not the key name).                    | String-valued |
| `comment_out`    | Comment out a line or key (prefix with `#` or format-appropriate comment).   | All text-based|
| `uncomment`      | Remove comment prefix from a commented line matching a pattern.              | All text-based|
| `insert_before`  | Insert content before a matched line or key.                                 | Ordered formats|
| `insert_after`   | Insert content after a matched line or key.                                  | Ordered formats|
| `add`            | Add a child element (XML), section (INI), or block.                          | Hierarchical  |

---

## Plain Text Operations

For files with no recognized structured format, or when the edit needs to operate at the text level regardless of format, the `plain` format provides line-based operations.

```
file /etc/security/limits.conf {
    package = pam

    edit {
        format = plain

        # Append lines to end of file
        append_line = "* soft nofile 65536"
        append_line = "* hard nofile 65536"
        append_line = "* soft nproc 32768"
        append_line = "* hard nproc 32768"

        # Insert before a matching line
        insert_before "# End of file" {
            line = "deploy soft nproc unlimited"
        }

        # Insert after a matching line
        insert_after "^# Domain" {
            line = "deploy hard nofile 131072"
        }

        # Delete lines matching a pattern
        delete_line = "^#.*example.*$"

        # Find and replace (first match, all matches, or nth match)
        replace {
            find = "SELINUX=permissive"
            replace = "SELINUX=enforcing"
            count = 1                        # 1 = first match, 0 = all matches
        }

        # Regex find and replace
        replace {
            regex = "^(PASS_MAX_DAYS)\\s+\\d+"
            replace = "\\1\t90"
        }

        # Comment out a line
        comment_out = "^PermitRootLogin"     # pattern match, adds # prefix

        # Uncomment a line
        uncomment = "^#\\s*ListenAddress 0.0.0.0"

        # Ensure a line exists (idempotent — adds only if not present)
        ensure_line = "net.ipv4.ip_forward = 1"

        # Ensure a line exists in a specific section/region
        ensure_line_after "^\\[main\\]" {
            line = "gpgcheck=1"
        }
    }
}
```

### Pattern Matching

Plain text operations use patterns for matching. Patterns support:

| Pattern Type | Syntax                         | Description                    |
|-------------|--------------------------------|--------------------------------|
| Literal     | `"exact text"`                 | Exact string match             |
| Regex       | `"^regex.*pattern$"`           | Regular expression             |
| Line number | `line(5)`                      | Specific line number           |
| Range       | `lines(10, 20)`               | Line range inclusive           |
| First/Last  | `first("pattern")`, `last("pattern")` | First or last match   |

---

## Raw Bash

When no structured mechanism fits, raw bash is available. Raw bash is emitted directly into the generated installation/configuration script.

### Block-Level Raw Bash

```
raw "configure_special_hardware" {
    phase = post_install          # when this runs (see Execution Phases below)
    
    script = """
        #!/bin/bash
        set -euo pipefail
        
        # Configure hardware RAID controller
        /opt/MegaRAID/MegaCli/MegaCli64 -CfgLdAdd -r1[0:1,0:2] -a0
        
        # Load custom firmware
        cp /tmp/firmware/*.bin /lib/firmware/custom/
        dracut --force
    """
    
    # Optional: declare files this script creates so the runtime agent can monitor them
    creates = ["/lib/firmware/custom/device.bin"]
    modifies = ["/boot/initramfs-*.img"]
}
```

### Inline Raw Bash

Inside any `dir` or `file` block, `raw_commands` inserts shell commands at that point in execution:

```
dir opt/application {
    raw_commands = """
        tar xzf /tmp/app-bundle.tar.gz -C /opt/application
        chown -R appuser:appgroup /opt/application
    """
    
    # Declare the expected result so the runtime agent can verify
    file bin/app {
        mode = 0755
        owner = appuser
        group = appgroup
        hash = "sha256:abcdef..."
    }
}
```

### Execution Phases

Raw bash blocks can declare when they execute in the build/install pipeline:

| Phase           | Description                                                    |
|-----------------|----------------------------------------------------------------|
| `pre_partition` | Before disk partitioning. Useful for hardware RAID setup.      |
| `post_partition`| After partitioning, before formatting.                         |
| `post_format`   | After filesystem creation, before file installation.           |
| `post_install`  | After all files are written. **Default if not specified.**     |
| `post_bootloader` | After bootloader installation.                              |
| `first_boot`    | Runs once on first boot of the installed system, not during image build. |
| `every_boot`    | Runs on every boot (installed as a service/script).            |

```
raw "raid_setup" {
    phase = pre_partition
    script = """
        mdadm --create /dev/md0 --level=1 --raid-devices=2 /dev/sda /dev/sdb
    """
}

raw "first_boot_enrollment" {
    phase = first_boot
    script = """
        # Enroll TPM2 key for LUKS auto-unlock
        clevis luks bind -d /dev/sda2 tpm2 '{"pcr_bank":"sha256","pcr_ids":"7"}'
    """
}
```

---

## Direct File Write

For files where the content comes from outside Ironclad's source tree at build time — fetched from a URL, extracted from an archive, generated by a build tool:

```
file /etc/pki/tls/certs/ca-bundle.crt {
    mode = 0444
    selinux = cert_t
    
    fetch {
        url = "https://curl.se/ca/cacert.pem"
        hash = "sha256:expected_hash_here"
    }
}

file /opt/app/config.json {
    mode = 0640
    owner = appuser
    
    pipe {
        command = "/usr/local/bin/generate-config --env production --format json"
        hash_verify = true     # compiler runs the command at build time and records the hash
    }
}
```

### Fetch Properties

| Property     | Required | Description                                  |
|--------------|----------|----------------------------------------------|
| `url`        | Yes      | URL to fetch at build time.                  |
| `hash`       | Yes      | Expected hash of fetched content.            |
| `headers`    | No       | HTTP headers as key-value map.               |
| `extract`    | No       | Extract from archive: `tar:path/in/archive`. |
| `fallback`   | No       | Alternative URL if primary fails.            |

### Pipe Properties

| Property       | Required | Description                                       |
|----------------|----------|---------------------------------------------------|
| `command`      | Yes      | Shell command whose stdout becomes the file content.|
| `hash_verify`  | No       | Record output hash in manifest for drift detection. |
| `env`          | No       | Environment variables for the command.             |

---

## Patch Application

For complex multi-line modifications to existing files, traditional unified diff patches can be applied:

```
file /etc/httpd/conf/httpd.conf {
    package = httpd

    patch {
        format = unified          # unified | context | normal
        strip = 1                 # -p1 equivalent for patch paths
        
        content = """
            --- a/etc/httpd/conf/httpd.conf
            +++ b/etc/httpd/conf/httpd.conf
            @@ -95,7 +95,7 @@
             # server as combust logs
             #
             
            -ServerAdmin root@localhost
            +ServerAdmin admin@example.com
             
             #
             # ServerName gives the name and port
        """
    }
    
    # Alternative: patch from file in source tree
    patch {
        source = patches/httpd-hardening.patch
        strip = 1
    }
}
```

`patch` is applied after `content`/`source`/`package` content is loaded but before `edit` operations. The application order is: content source → patch → edit → hash computation.

---

## Binary File Operations

Most binary files should be handled with `source` (copy a known binary) or `fetch` (download). For cases where binary files need modification:

```
file /boot/efi/EFI/BOOT/BOOTX64.EFI {
    source = files/BOOTX64.EFI
    
    binary_edit {
        # Sign a PE binary
        command = "sbsign --key /etc/pki/secureboot/db.key --cert /etc/pki/secureboot/db.crt --output {output} {input}"
    }
}
```

`binary_edit` runs a command with `{input}` and `{output}` placeholders. The compiler provides temporary paths. This is a thin wrapper over raw bash scoped to a specific file, with the benefit that the file's metadata (mode, owner, selinux) is still managed declaratively.

---

## Operation Ordering Within a File

When a file declaration has multiple modification mechanisms, they are applied in a fixed order:

1. **Content source** — `content`, `source`, `template`, `package`, `fetch`, or `pipe` loads the base content
2. **Patch** — `patch` blocks are applied to the base content
3. **Edit** — Format-aware `edit` operations are applied
4. **Binary edit** — `binary_edit` runs on the result
5. **Hash computation** — The compiler computes the final hash for the intermediate manifest

Each stage operates on the output of the previous stage. If any stage fails (patch doesn't apply, edit finds an unexpected format), the build fails with a diagnostic.

---

## Compiler Cross-Validation

| Validation | Description |
|---|---|
| **Format parse** | For format-aware edits, the compiler parses the base content in the declared format and errors if it's malformed. |
| **Key existence** | `delete` on a nonexistent key is a warning. `set` on a new key is fine (creates it). |
| **Patch application** | Patches are applied at compile time. Failure to apply is a compile error with context. |
| **Sudoers validation** | Sudoers output is validated with `visudo -c` equivalent logic. |
| **Hash verification** | `fetch` with `hash` is verified at build time. Mismatch is a compile error. |
| **Conflicting operations** | `set` and `delete` on the same key in the same `edit` block is an error. |
| **Content source required** | `edit`, `patch`, and `binary_edit` require a content source. Operating on nothing is an error. |
| **Raw bash auditing** | Raw bash blocks are logged in the intermediate manifest. The runtime agent knows which files were touched by raw operations. |

---

## Security Floor Enforcement

| Level | Enforcement |
|---|---|
| **Baseline** | No restrictions on any operation type. |
| **Standard** | `raw` blocks emit an info-level message in the build log. `fetch` requires `hash`. |
| **Strict** | `raw` blocks emit a warning. `pipe` commands must be from declared packages. `fetch` requires `hash` and HTTPS. `binary_edit` emits a warning. |
| **Maximum** | `raw` blocks emit a hard warning (promotable to error with `--strict-raw`). All `fetch` URLs must be from an allowlist. `pipe` commands must have declared expected hashes. Every `edit` operation is logged individually in the manifest for audit. |

---

## Integration with Classes and Functions

Edit operations compose with the class system and merge rules from `FILESYSTEM_SYNTAX.md`:

```
class hardened_ssh {
    file /etc/ssh/sshd_config {
        package = openssh-server
        
        edit {
            format = sshd_config
            
            set PermitRootLogin = "no"
            set PasswordAuthentication = "no"
            set MaxAuthTries = 3
        }
    }
}

class compliance_overlay extends hardened_ssh {
    file /etc/ssh/sshd_config {
        edit {
            format = sshd_config
            
            # Adds to parent's edits — both sets of edits are applied
            set MaxAuthTries = 2            # overrides parent's 3
            set LoginGraceTime = 60
            set Banner = "/etc/issue.net"
        }
    }
}
```

**Edit merge rules:**

- When a parent and derived class both have `edit` blocks for the same file, the edits are **concatenated** (parent first, derived second) and applied in order.
- If both declare `set` for the same key, the derived class's value wins (it runs later). A soft warning is emitted.
- Inline tree `edit` blocks run **last** (after all class edits), consistent with "inline wins."
- `delete` in a derived class removes a key set by the parent class's edits.

```
# Compiler edit application order for /etc/ssh/sshd_config:
# 1. Load package content (openssh-server's sshd_config)
# 2. Apply hardened_ssh edits (PermitRootLogin=no, PasswordAuthentication=no, MaxAuthTries=3)
# 3. Apply compliance_overlay edits (MaxAuthTries=2 [overrides], LoginGraceTime=60, Banner=...)
# 4. Apply inline tree edits (if any)
# 5. Compute final hash
#
# WARNING: MaxAuthTries set to 2 (class compliance_overlay) overrides 3 (class hardened_ssh)
```

---

## Parameterized Edits with Functions

```
func harden_sshd(max_auth: int, allowed_users: list[string]) {
    file /etc/ssh/sshd_config {
        package = openssh-server
        
        edit {
            format = sshd_config
            
            set PermitRootLogin = "no"
            set PasswordAuthentication = "no"
            set MaxAuthTries = ${max_auth}
            set AllowUsers = ${allowed_users}
            set X11Forwarding = "no"
            set LogLevel = "VERBOSE"
        }
    }
}

# Usage:
system web01 {
    embed func.harden_sshd(3, ["deploy", "admin"])
}
```

---

## Reserved Keywords (File Operations Context)

`edit`, `format`, `set`, `set_default`, `delete`, `ensure_absent`, `append`, `prepend`, `replace_value`, `comment_out`, `uncomment`, `insert_before`, `insert_after`, `add`, `remove_module`, `update_module`, `match`, `grant`, `default`, `job`, `env`, `append_line`, `delete_line`, `ensure_line`, `ensure_line_after`, `replace`, `find`, `regex`, `count`, `raw`, `script`, `phase`, `creates`, `modifies`, `fetch`, `pipe`, `patch`, `binary_edit`, `dropin_for`, `plain`, `yaml`, `toml`, `json`, `ini`, `xml`, `properties`, `keyvalue`, `sshd_config`, `systemd_unit`, `grub_default`, `fstab`, `sudoers`, `pam`, `crontab`, `csv`, `set_row`, `append_row`, `delete_row`

---

## Grammar Summary (Informative)

```
edit_block          = "edit" "{" format_decl edit_op* "}"
format_decl         = "format" "=" format_identifier

edit_op             = set_op | delete_op | append_op | prepend_op | replace_op
                    | comment_op | uncomment_op | insert_op | ensure_op
                    | match_block | grant_block | job_block | row_op
                    | add_op | remove_module_op | update_module_op

set_op              = "set" key_path "=" value
set_default_op      = "set_default" key_path "=" value
delete_op           = ("delete" | "ensure_absent") key_path
append_op           = "append" key_path "=" value
prepend_op          = "prepend" key_path "=" value
replace_op          = "replace" "{" "find" "=" string "replace" "=" string ("count" "=" int)? "}"
                    | "replace" "{" "regex" "=" string "replace" "=" string "}"
comment_op          = "comment_out" "=" pattern
uncomment_op        = "uncomment" "=" pattern
insert_op           = ("insert_before" | "insert_after") pattern "{" "line" "=" string "}"
ensure_op           = "ensure_line" "=" string
                    | "ensure_line_after" pattern "{" "line" "=" string "}"
add_op              = "add" key_path "<<" multiline_string

append_line_op      = "append_line" "=" string
delete_line_op      = "delete_line" "=" pattern

match_block         = "match" string "{" edit_op* "}"           # sshd_config Match blocks
grant_block         = "grant" string "{" grant_props "}"        # sudoers
job_block           = "job" string "{" job_props "}"            # crontab

raw_block           = "raw" name "{" raw_props "}"
raw_props           = ("phase" "=" phase_id)? "script" "=" multiline_string
                      ("creates" "=" array)? ("modifies" "=" array)?

fetch_block         = "fetch" "{" fetch_props "}"
pipe_block          = "pipe" "{" pipe_props "}"
patch_block         = "patch" "{" patch_props "}"
binary_edit_block   = "binary_edit" "{" "command" "=" string "}"

key_path            = identifier ("." identifier)*
pattern             = string                                    # literal or regex
phase_id            = "pre_partition" | "post_partition" | "post_format"
                    | "post_install" | "post_bootloader" | "first_boot" | "every_boot"
```

---

## What This Document Does Not Cover

- **Language primitives** — Variables, classes, functions, references (`LANGUAGE_SYNTAX.md`)
- **Filesystem declarations** — File/directory creation, merge semantics (`FILESYSTEM_SYNTAX.md`)
- **Storage topology** — Block devices, encryption, volume management (`STORAGE_SYNTAX.md`)
- **Secret management** — Handling credentials, API keys, and certificates in edits (future specification)
- **Remote file operations** — Fetching from authenticated endpoints, S3, etc. (future specification)
- **Package management** — Declaring packages that `package =` references validate against (future specification)
