# Ironclad Users and Groups Syntax Specification

**Status:** Draft — syntax development, Phase 1  
**Scope:** User declarations, group declarations, password policy, home directory provisioning, SELinux user mapping, system accounts, and cross-validation against filesystem, init, SELinux, and secrets declarations  
**Dependencies:** `LANGUAGE_SYNTAX.md`, `FILESYSTEM_SYNTAX.md`, `SELINUX_SYNTAX.md`, `INIT_SYNTAX.md`

---

## Design Principles

Users and groups are the identity layer of a Linux system. Every file has an owner and group. Every process runs as a user. Every SELinux context includes a user mapping. Ironclad declares users and groups explicitly so the compiler can validate every `owner`, `group`, `user`, and SELinux user reference across the entire source tree. Undeclared identities are compile errors, not runtime surprises.

The key design constraints:

1. **Every user and group is declared.** The compiler does not assume standard users exist unless they are explicitly declared or inherited from a base class. `root` is the only exception — it is implicitly declared with well-known properties. All other accounts — system or human — must appear in the source tree.

2. **System accounts and human accounts are the same construct.** The `system` flag marks an account as a system account (low UID, no login shell, no home directory by default). The syntax is identical. This avoids a separate concept for service accounts.

3. **SELinux user mapping is a user property.** Every Linux user maps to an SELinux user. The mapping is declared on the user and validated against the SELinux policy declaration. This is where Linux identity meets MAC identity.

4. **Home directories are provisioned from the declaration.** When a user declares a home directory, the compiler ensures the path exists in the filesystem tree with the correct ownership and permissions. The operator can override any generated property.

5. **Password policy is declarative, not procedural.** The system declares password rules — minimum length, maximum age, complexity, lockout. The compiler emits the corresponding `/etc/login.defs`, `/etc/security/pwquality.conf`, and PAM configuration. Individual user password hashes are handled through the secrets system.

---

## System-Level Users Block

User and group declarations live inside a `users` block at the system level or inside a class.

```
system web01 {
    users {
        # password policy, user declarations, group declarations
    }
}
```

A system may have at most one `users` block. When classes contribute user declarations, they are merged into the single `users` block using standard merge rules.

---

## Password Policy

The `policy` sub-block inside `users` configures system-wide password and account policy.

```
users {
    policy {
        min_length = 15
        max_age = 90
        min_age = 1
        warn_age = 14
        inactive_days = 30
        history = 12
        complexity {
            min_classes = 3           # of: uppercase, lowercase, digit, special
            max_repeat = 3
            max_sequence = 3
            reject_username = true
        }
        lockout {
            attempts = 5
            interval = 900            # seconds
            unlock_time = 1800        # seconds, 0 = manual unlock only
        }
        umask = 0077
        uid_min = 1000
        uid_max = 60000
        gid_min = 1000
        gid_max = 60000
        sys_uid_min = 201
        sys_uid_max = 999
        sys_gid_min = 201
        sys_gid_max = 999
    }
}
```

### Policy Properties

| Property          | Type   | Default     | Description                                                    |
|-------------------|--------|-------------|----------------------------------------------------------------|
| `min_length`      | `int`  | `8`         | Minimum password length.                                       |
| `max_age`         | `int`  | `99999`     | Maximum password age in days. `99999` disables expiry.         |
| `min_age`         | `int`  | `0`         | Minimum days between password changes.                         |
| `warn_age`        | `int`  | `7`         | Days before expiry to warn the user.                           |
| `inactive_days`   | `int`  | `-1`        | Days after expiry before account is locked. `-1` disables.     |
| `history`         | `int`  | `0`         | Number of previous passwords remembered. `0` disables.         |
| `umask`           | `mode` | `0022`      | Default file creation mask for new users.                      |
| `uid_min`         | `int`  | `1000`      | Minimum UID for regular users.                                 |
| `uid_max`         | `int`  | `60000`     | Maximum UID for regular users.                                 |
| `gid_min`         | `int`  | `1000`      | Minimum GID for regular groups.                                |
| `gid_max`         | `int`  | `60000`     | Maximum GID for regular groups.                                |
| `sys_uid_min`     | `int`  | `201`       | Minimum UID for system accounts.                               |
| `sys_uid_max`     | `int`  | `999`       | Maximum UID for system accounts.                               |
| `sys_gid_min`     | `int`  | `201`       | Minimum GID for system groups.                                 |
| `sys_gid_max`     | `int`  | `999`       | Maximum GID for system groups.                                 |

### Complexity Sub-Block

| Property           | Type   | Default | Description                                                |
|--------------------|--------|---------|------------------------------------------------------------|
| `min_classes`      | `int`  | `0`     | Minimum number of character classes required (1-4).        |
| `max_repeat`       | `int`  | `0`     | Maximum consecutive identical characters. `0` disables.    |
| `max_sequence`     | `int`  | `0`     | Maximum monotonic character sequence length. `0` disables. |
| `reject_username`  | `bool` | `false` | Reject passwords containing the username.                  |

### Lockout Sub-Block

| Property       | Type   | Default | Description                                                       |
|----------------|--------|---------|-------------------------------------------------------------------|
| `attempts`     | `int`  | `5`     | Failed login attempts before lockout.                              |
| `interval`     | `int`  | `900`   | Window (seconds) in which failed attempts are counted.             |
| `unlock_time`  | `int`  | `600`   | Seconds before a locked account unlocks automatically. `0` = manual only. |

**Compiler behavior:** The `policy` block emits `/etc/login.defs`, `/etc/security/pwquality.conf`, and PAM `pam_faillock` configuration. These files are marked `generated` — the operator can override them with inline file declarations, but the compiler warns on conflict.

---

## User Declarations

Users are declared as named blocks inside the `users` block.

```
users {
    user root {
        uid = 0
        gid = 0
        home = /root
        shell = /bin/bash
        selinux_user = unconfined_u
    }

    user kit {
        uid = 1000
        gid = 1000
        comment = "Kit"
        home = /home/kit
        shell = /bin/bash
        groups = [wheel, developers]
        selinux_user = staff_u
        password = secret.kit_password
    }

    user apache {
        system = true
        uid = 48
        gid = 48
        comment = "Apache HTTP Server"
        home = /usr/share/httpd
        shell = /sbin/nologin
        selinux_user = system_u
    }

    user nobody {
        system = true
        uid = 65534
        gid = 65534
        home = /
        shell = /sbin/nologin
        selinux_user = system_u
    }
}
```

### User Properties

| Property        | Type             | Default                    | Description                                                             |
|-----------------|------------------|----------------------------|-------------------------------------------------------------------------|
| `uid`           | `int`            | auto-assigned              | Numeric user ID. Auto-assigned within the policy range if omitted.      |
| `gid`           | `int`            | auto-assigned              | Primary group ID. Must match a declared group. Auto-assigned if omitted.|
| `system`        | `bool`           | `false`                    | System account flag. Affects UID range and defaults.                    |
| `comment`       | `string`         | `""`                       | GECOS field. Human-readable description.                                |
| `home`          | `path`           | `/home/<name>` or `/`      | Home directory path. System accounts default to `/`.                    |
| `create_home`   | `bool`           | `true` for regular, `false` for system | Whether to create the home directory.                    |
| `shell`         | `string`         | `/bin/bash` or `/sbin/nologin` | Login shell. System accounts default to `/sbin/nologin`.           |
| `groups`        | `list[string]`   | `[]`                       | Supplementary groups. Each must be a declared group name.               |
| `selinux_user`  | `string`         | (from SELinux default)     | SELinux user mapping. Validated against SELinux user declarations.       |
| `password`      | `reference`      | (none)                     | Reference to a secret containing the password hash.                     |
| `locked`        | `bool`           | `false`                    | Whether the account is locked (`!` prefix in shadow).                   |
| `expires`       | `string`         | (none)                     | Account expiration date in `YYYY-MM-DD` format. Omit for no expiry.    |
| `ssh_keys`      | `list[string]`   | `[]`                       | SSH authorized keys. Written to `~/.ssh/authorized_keys`.              |
| `state`         | `present\|absent`| `present`                  | `absent` ensures the user does not exist (useful for class overrides).  |

### The `root` Account

`root` is implicitly declared with `uid = 0`, `gid = 0`, `home = /root`, `shell = /bin/bash`. The operator can redeclare `root` to override any property (e.g., to set `shell = /sbin/nologin` for a locked-down server, or to assign an explicit `selinux_user`). The implicit declaration prevents the need to declare `root` in every system just to satisfy `owner = root` validation.

### Auto-Assigned UIDs

When `uid` is omitted, the compiler assigns a UID:
- System accounts: next available in `[sys_uid_min, sys_uid_max]`
- Regular accounts: next available in `[uid_min, uid_max]`

Auto-assigned UIDs are deterministic — the compiler assigns in declaration order. The intermediate manifest records the assigned UIDs. The runtime agent verifies them.

### Home Directory Provisioning

When `create_home = true`, the compiler:
1. Ensures the home directory path exists in the filesystem tree
2. Sets `owner = <username>`, `group = <primary_group>`, `mode = 0700` on the directory
3. Copies skeleton files from `/etc/skel` (declared as a directory in the filesystem tree or from the package defaults)

If the operator declares the home directory explicitly in the filesystem tree, the explicit declaration wins (standard merge rules). The compiler emits a warning if the explicitly declared owner does not match the user.

### SSH Key Provisioning

When `ssh_keys` is non-empty, the compiler:
1. Ensures `<home>/.ssh` exists with `owner = <username>`, `group = <primary_group>`, `mode = 0700`
2. Writes `<home>/.ssh/authorized_keys` with `mode = 0600` containing the declared keys

---

## Group Declarations

Groups are declared as named blocks inside the `users` block.

```
users {
    group root {
        gid = 0
    }

    group wheel {
        gid = 10
    }

    group developers {
        gid = 1001
        members = [kit]
    }

    group apache {
        system = true
        gid = 48
    }

    group docker {
        system = true
    }
}
```

### Group Properties

| Property    | Type            | Default        | Description                                                          |
|-------------|-----------------|----------------|----------------------------------------------------------------------|
| `gid`       | `int`           | auto-assigned  | Numeric group ID. Auto-assigned within the policy range if omitted.  |
| `system`    | `bool`          | `false`        | System group flag. Affects GID range.                                |
| `members`   | `list[string]`  | `[]`           | Explicit group members. Each must be a declared user name.           |
| `state`     | `present\|absent` | `present`    | `absent` ensures the group does not exist.                           |

### Implicit Group Creation

When a user declares a `gid` that does not match any declared group, the compiler creates an implicit primary group with the same name and GID as the user. This matches the behavior of `useradd` with `USERGROUPS_ENAB yes`.

When a user lists a group in `groups` that is not declared, this is a compile error — supplementary groups must be explicitly declared.

### Bidirectional Membership

Group membership can be declared from either direction:
- On the **user**: `groups = [wheel, developers]` — the user is added to these groups
- On the **group**: `members = [kit, jane]` — these users are added to the group

Both are valid and additive. The compiler merges membership from both sources. Conflicts are impossible — membership is a set union.

---

## Classes and Users

Classes can declare users and groups. When a class is applied to a system, its user and group declarations merge into the system's `users` block.

```
class httpd_server {
    users {
        user apache {
            system = true
            uid = 48
            gid = 48
            home = /usr/share/httpd
            shell = /sbin/nologin
            selinux_user = system_u
        }

        group apache {
            system = true
            gid = 48
        }
    }
}
```

### Merge Semantics

User and group declarations follow the standard merge rules:

1. **Inline wins over class.** A user declared in the system's `users` block overrides the same user from a class.
2. **Later apply wins.** If two classes declare the same user, the later `apply` wins.
3. **Property-level merge.** Only conflicting properties are overridden. Non-conflicting properties from both sources are preserved.
4. **Conflict warning.** The compiler emits a soft warning on every property conflict, identifying the winning and losing values.

```
# Class declares:
user apache { uid = 48; shell = /sbin/nologin }

# System inline declares:
user apache { uid = 80 }

# Compiler resolves:
# user apache { uid = 80; shell = /sbin/nologin }
# WARNING: uid conflict, inline 80 wins over class 48
```

---

## Compiler Cross-Validation

| Validation | Description |
|---|---|
| **Owner exists** | Every `owner` property in filesystem declarations must reference a declared user name. |
| **Group exists** | Every `group` property in filesystem declarations must reference a declared group name. |
| **Service user exists** | Every `user` property in service declarations must reference a declared user name. |
| **Service group exists** | Every `group` property in service declarations must reference a declared group name. |
| **Supplementary groups exist** | Every group name in a user's `groups` list must be a declared group. |
| **Group members exist** | Every user name in a group's `members` list must be a declared user. |
| **SELinux user exists** | Every `selinux_user` value must reference a declared SELinux user in the SELinux policy. |
| **SELinux role authorized** | The SELinux user's authorized roles are validated against the SELinux policy. |
| **UID uniqueness** | No two users may share a UID. |
| **GID uniqueness** | No two groups may share a GID (unless explicitly overridden with `allow_gid_sharing = true`). |
| **UID/GID range** | Assigned UIDs and GIDs fall within the declared policy ranges. |
| **Home directory backing** | If `create_home = true`, the home directory path must resolve to a declared filesystem via mount-point matching. |
| **Shell exists** | The declared shell path must exist in the filesystem tree (warning, not error — shell may come from a package). |
| **Password reference valid** | If `password` references a secret, the secret must be declared and have `scope = build` or `scope = both`. |

---

## Security Floor Enforcement

| Level | Enforcement |
|---|---|
| **Baseline** | No user/group enforcement. |
| **Standard** | Warning if any regular user has an empty password. Warning if `root` has `shell` set to a login shell and no `password` or `locked = true`. |
| **Strict** | Standard warnings become errors. All users must have an explicit `selinux_user`. All regular users must reference a password secret or be `locked = true`. UID 0 must be `root` only. |
| **Maximum** | Strict rules plus: no shared UIDs or GIDs. All system accounts must have `shell = /sbin/nologin`. All home directories must have `mode = 0700` or stricter. SSH keys must reference secrets, not inline strings. |

---

## Reserved Keywords

The following words are reserved in user/group context:

`users`, `user`, `group`, `policy`, `complexity`, `lockout`, `uid`, `gid`, `system`, `comment`, `home`, `create_home`, `shell`, `groups`, `members`, `selinux_user`, `password`, `locked`, `expires`, `ssh_keys`, `state`, `present`, `absent`, `min_length`, `max_age`, `min_age`, `warn_age`, `inactive_days`, `history`, `umask`

---

## Grammar Summary (Informative)

```
users_block      = "users" "{" users_body "}"
users_body       = policy_block? (user_decl | group_decl)*

policy_block     = "policy" "{" policy_body "}"
policy_body      = property* complexity_block? lockout_block?
complexity_block = "complexity" "{" property* "}"
lockout_block    = "lockout" "{" property* "}"

user_decl        = "user" identifier "{" user_body "}"
user_body        = property*

group_decl       = "group" identifier "{" group_body "}"
group_body       = property*

property         = identifier "=" value
```

---

## What This Document Does Not Cover

This specification covers user and group declarations. The following topics are defined in separate specifications:

- **SELinux user and role declarations** — SELinux user definitions, role assignments, MLS ranges (`SELINUX_SYNTAX.md`)
- **Service identity** — How services reference users for process execution (`INIT_SYNTAX.md`)
- **File ownership** — How filesystem declarations reference users and groups (`FILESYSTEM_SYNTAX.md`)
- **Secret management** — How password hashes and SSH keys are stored and delivered (`SECRETS_SYNTAX.md`)
- **PAM configuration** — Pluggable authentication module stack beyond password policy (future specification or stdlib class)
