# Ironclad Packages Syntax Specification

**Status:** Draft — syntax development, Phase 1  
**Scope:** Package declarations, version constraints, repository configuration, package groups, excluded packages, and cross-validation against filesystem, init, and bootloader declarations  
**Dependencies:** `LANGUAGE_SYNTAX.md`, `FILESYSTEM_SYNTAX.md`, `INIT_SYNTAX.md`

---

## Design Principles

Packages are the mechanism by which software reaches the filesystem. Ironclad declares which packages are installed so the compiler can validate file-package relationships, emit correct backend artifacts (Containerfile `RUN dnf install`, Kickstart `%packages`), and detect conflicts between package-provided files and operator-declared files.

The key design constraints:

1. **Packages are declared, not discovered.** The compiler does not query a package manager at compile time. The operator declares the package set. The compiler validates internal consistency (filesystem `package = httpd` references a declared package) and emits the package list to the backend.

2. **Version constraints are optional but validated.** Most packages should track the repository's current version. When pinning is required, the constraint syntax matches the native package manager's version comparison semantics (RPM for RHEL-family, dpkg for Debian-family).

3. **Repositories are declared alongside packages.** A system must declare where packages come from. This enables airgapped builds and ensures reproducibility.

4. **Package groups are a convenience, not magic.** A package group (`@core`, `@minimal-install`) expands to its member packages. The compiler does not resolve group membership — it emits the group name to the backend and trusts the package manager. Individual package overrides (includes and excludes) are explicit.

5. **The package list is an input to the build, not an output.** The compiler does not perform dependency resolution. It emits the declared packages. The package manager in the build environment resolves dependencies. The compiler's job is to ensure that packages referenced by other declarations (`package = httpd`, `init systemd` requiring `systemd`) are in the declared set.

---

## System-Level Packages Block

Package declarations live inside a `packages` block at the system level or inside a class.

```
system web01 {
    packages {
        # repositories, package declarations, groups, excludes
    }
}
```

A system may have at most one `packages` block. When classes contribute package declarations, they are merged into the single `packages` block using standard merge rules.

---

## Repository Configuration

Repositories are declared inside the `packages` block. They define where the package manager fetches packages.

```
packages {
    repo baseos {
        name = "AlmaLinux 9 - BaseOS"
        baseurl = "https://repo.almalinux.org/almalinux/9/BaseOS/x86_64/os/"
        gpgcheck = true
        gpgkey = "file:///etc/pki/rpm-gpg/RPM-GPG-KEY-AlmaLinux-9"
        enabled = true
    }

    repo appstream {
        name = "AlmaLinux 9 - AppStream"
        baseurl = "https://repo.almalinux.org/almalinux/9/AppStream/x86_64/os/"
        gpgcheck = true
        gpgkey = "file:///etc/pki/rpm-gpg/RPM-GPG-KEY-AlmaLinux-9"
        enabled = true
    }

    repo epel {
        name = "Extra Packages for Enterprise Linux 9"
        metalink = "https://mirrors.fedoraproject.org/metalink?repo=epel-9&arch=x86_64"
        gpgcheck = true
        gpgkey = "file:///etc/pki/rpm-gpg/RPM-GPG-KEY-EPEL-9"
        enabled = true
    }
}
```

### Repository Properties

| Property       | Type     | Default     | Description                                                        |
|----------------|----------|-------------|--------------------------------------------------------------------|
| `name`         | `string` | required    | Human-readable repository name.                                    |
| `baseurl`      | `string` | (none)      | Base URL for the repository. Mutually exclusive with `metalink` and `mirrorlist`. |
| `metalink`     | `string` | (none)      | Metalink URL for mirror selection. Mutually exclusive with `baseurl` and `mirrorlist`. |
| `mirrorlist`   | `string` | (none)      | Mirror list URL. Mutually exclusive with `baseurl` and `metalink`. |
| `gpgcheck`     | `bool`   | `true`      | Whether to verify package signatures.                              |
| `gpgkey`       | `string` | (none)      | GPG key URL or file path.                                          |
| `enabled`      | `bool`   | `true`      | Whether the repository is active.                                  |
| `priority`     | `int`    | `99`        | Repository priority (lower = preferred). For `dnf` priority plugin.|
| `sslverify`    | `bool`   | `true`      | Whether to verify TLS certificates.                                |
| `sslcacert`    | `path`   | (none)      | Path to CA certificate bundle.                                     |
| `sslclientcert`| `path`   | (none)      | Client certificate for authenticated repos.                        |
| `sslclientkey` | `path`   | (none)      | Client private key for authenticated repos.                        |
| `module_hotfixes` | `bool` | `false`   | Whether this repo provides module hotfixes.                        |
| `cost`         | `int`    | `1000`      | Relative cost of accessing this repository.                        |

**Compiler behavior:** Each `repo` block emits a `.repo` file to `/etc/yum.repos.d/<name>.repo`. The file is marked `generated`.

### At Least One Source Required

The compiler errors if a `packages` block contains package declarations but no repository configuration. The exception is when the system's build backend provides repositories implicitly (e.g., a bootc base image that already has repos configured). This exception is declared with:

```
packages {
    base_image_repos = true    # trust repos from the base image
}
```

---

## Package Declarations

Individual packages are declared with the `pkg` keyword.

```
packages {
    pkg httpd {}
    pkg mod_ssl {}
    pkg php {}
    pkg php-mysqlnd {}
    pkg firewalld { state = absent }
}
```

### Shorthand

When no properties need to be set, the block can be written inline:

```
packages {
    pkg httpd
    pkg mod_ssl
    pkg php
    pkg php-mysqlnd
}
```

### Package Properties

| Property    | Type              | Default   | Description                                                              |
|-------------|-------------------|-----------|--------------------------------------------------------------------------|
| `version`   | `string`          | (none)    | Version constraint. See version constraint syntax below.                 |
| `arch`      | `string`          | (none)    | Architecture constraint (`x86_64`, `noarch`, `i686`). Omit for default. |
| `repo`      | `string`          | (none)    | Pin to a specific declared repository name.                              |
| `state`     | `present\|absent` | `present` | `absent` ensures the package is not installed (exclude).                 |
| `reason`    | `string`          | (none)    | Human-readable note for why this package is included. Informational.     |

### Version Constraint Syntax

Version constraints use RPM version comparison semantics for RHEL-family systems:

```
pkg httpd { version = "2.4.57" }              # exact version
pkg httpd { version = ">= 2.4.57" }           # minimum version
pkg httpd { version = ">= 2.4.57, < 2.5.0" }  # range
pkg kernel { version = "5.14.0-362.el9" }     # exact with release
```

| Operator | Meaning               |
|----------|-----------------------|
| `=`      | Exact version match   |
| `>=`     | Minimum version       |
| `<=`     | Maximum version       |
| `>`      | Greater than          |
| `<`      | Less than             |
| `,`      | AND (both must hold)  |

When `version` is omitted, the package manager installs the latest available version. This is the recommended default — version pinning should be reserved for cases where compatibility is known to break.

---

## Package Groups

Package groups (comps groups in RPM terminology) are declared with the `group` keyword inside the `packages` block.

```
packages {
    group core
    group minimal-install

    group "Development Tools" {
        optional = true     # include optional packages from this group
    }
}
```

### Group Properties

| Property    | Type   | Default | Description                                                         |
|-------------|--------|---------|---------------------------------------------------------------------|
| `optional`  | `bool` | `false` | Whether to include optional packages from the group.                |
| `state`     | `present\|absent` | `present` | `absent` excludes the entire group.                       |

**Compiler behavior:** Group names are passed directly to the package manager (`dnf groupinstall`, `%packages @group`). The compiler does not resolve group membership. Individual packages can be excluded from a group using `pkg <name> { state = absent }`.

---

## Package Modules (DNF Modularity)

For systems using DNF module streams:

```
packages {
    module php {
        stream = "8.2"
        profiles = [common]
    }

    module nodejs {
        stream = "20"
        profiles = [common, development]
    }
}
```

### Module Properties

| Property   | Type           | Default    | Description                                          |
|------------|----------------|------------|------------------------------------------------------|
| `stream`   | `string`       | required   | Module stream version.                               |
| `profiles` | `list[string]` | `[common]` | Module profiles to install.                          |
| `state`    | `enabled\|disabled\|absent` | `enabled` | Module state.                            |

---

## Excluded Packages

Packages can be excluded globally (never installed regardless of group membership or dependency resolution):

```
packages {
    exclude = [
        "PackageKit",
        "abrt*",
        "evolution",
        "gnome-*",
    ]
}
```

The `exclude` list supports glob patterns. Excluded packages are passed to the package manager's exclude mechanism (`exclude=` in `dnf.conf`, `--excludepkgs` in Kickstart).

**Interaction with `pkg { state = absent }`:** Both achieve similar results. `exclude` is a blanket exclusion that prevents even dependency-pulled installation. `pkg { state = absent }` is a targeted declaration that the operator has considered this package and wants it absent.

---

## Classes and Packages

Classes can declare packages they require. This is the mechanism by which role classes ensure their dependencies are present.

```
class httpd_server {
    packages {
        pkg httpd
        pkg mod_ssl
        pkg mod_security
    }

    # ... file and service declarations that depend on these packages
}
```

### Merge Semantics

Package declarations from classes merge additively:

1. **Same package, no conflicts:** Package appears once in the merged list.
2. **Same package, different versions:** Later `apply` wins. Warning emitted.
3. **Package declared `present` by one source and `absent` by another:** Inline wins over class. Later `apply` wins between classes. Warning emitted.
4. **Repository declarations merge by name.** Properties of the same-named repository follow standard merge rules.

---

## Compiler Cross-Validation

| Validation | Description |
|---|---|
| **File `package` references** | Every `package = httpd` in a filesystem declaration must reference a package declared in the `packages` block. |
| **Init backend package** | `init systemd` validates that `systemd` (or equivalent) is in the package list. `init s6` validates that `s6` packages are present. |
| **Repo source exists** | At least one repository or `base_image_repos = true` must be declared. |
| **Version constraint syntax** | Version strings are validated for correct syntax. |
| **Exclude conflicts** | A package that is both explicitly declared (`pkg httpd`) and in the `exclude` list produces a compile error. |
| **Arch consistency** | If a package declares `arch`, the compiler validates the architecture is plausible for the target system. |
| **Repository references** | A package pinned to `repo = epel` must reference a declared repository. |

---

## Security Floor Enforcement

| Level | Enforcement |
|---|---|
| **Baseline** | No package enforcement. |
| **Standard** | Warning if `gpgcheck = false` on any repository. Warning if packages are installed without version constraints in a production-tagged system. |
| **Strict** | Standard warnings become errors. All repositories must have `gpgcheck = true` and `sslverify = true`. |
| **Maximum** | Strict rules plus: all repositories must use `baseurl` (not `mirrorlist` or `metalink`) for deterministic resolution. All packages must have explicit `version` constraints. No glob patterns in `exclude` (each exclusion must be specific). |

---

## Reserved Keywords

The following words are reserved in package context:

`packages`, `pkg`, `repo`, `group`, `module`, `exclude`, `version`, `arch`, `state`, `present`, `absent`, `enabled`, `disabled`, `name`, `baseurl`, `metalink`, `mirrorlist`, `gpgcheck`, `gpgkey`, `priority`, `sslverify`, `sslcacert`, `sslclientcert`, `sslclientkey`, `optional`, `stream`, `profiles`, `reason`, `base_image_repos`, `cost`, `module_hotfixes`

---

## Grammar Summary (Informative)

```
packages_block  = "packages" "{" packages_body "}"
packages_body   = property* (repo_decl | pkg_decl | group_decl | module_decl | exclude_decl)*

repo_decl       = "repo" identifier "{" property* "}"
pkg_decl        = "pkg" identifier ("{" property* "}")? 
group_decl      = "group" (identifier | string) ("{" property* "}")?
module_decl     = "module" identifier "{" property* "}"
exclude_decl    = "exclude" "=" list

property        = identifier "=" value
```

---

## What This Document Does Not Cover

This specification covers package declarations and repository configuration. The following topics are defined in separate specifications:

- **File-package relationships** — How filesystem declarations reference packages (`FILESYSTEM_SYNTAX.md`)
- **Init backend dependencies** — How init system selection implies package requirements (`INIT_SYNTAX.md`)
- **Build backend mapping** — How the package list maps to Containerfile or Kickstart output (future specification)
- **Package signing and verification** — RPM signature validation beyond GPG check (future specification)
- **Dependency resolution** — Performed by the package manager, not the compiler
