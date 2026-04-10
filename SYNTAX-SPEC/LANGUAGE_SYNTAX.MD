# Ironclad Language Syntax Specification

**Status:** Draft — syntax development, Phase 1  
**Scope:** Variables, types, classes, functions, references (pointers), imports, and composition — the core language primitives that all domain-specific blocks build on

---

## Design Principles

Ironclad is a declarative language with imperative escape hatches, not the reverse. The language exists to declare systems. General-purpose constructs — variables, conditionals, loops, functions — exist to make declarations reusable and parameterizable, not to express arbitrary computation.

The key design constraints:

1. **Everything resolves to the tree.** Variables, functions, and classes are authoring tools. The compiler resolves all of them into a fully expanded storage-rooted filesystem tree with concrete values. There is no runtime interpretation of Ironclad source.

2. **References are pointers into the tree.** A reference like `storage.sda.system.vg0.root` is a path through the declared structure. References are resolved at compile time. Dangling references are compile errors.

3. **Classes are reusable tree fragments.** A class declares a partial tree (files, directories, configurations, subsystem blocks) that can be applied to a system. Classes compose via single inheritance and property-level merge.

4. **Functions are parameterized tree fragments.** A function takes arguments and returns a tree fragment. Functions are pure — they produce declarations, not side effects.

5. **The scope hierarchy is: system > storage topology > filesystem tree > file properties.** Variables, classes, and functions can be declared at any scope and are visible to that scope and its children.

---    

## Variables

Variables are named, typed values. They are resolved at compile time. There are no runtime variables.

### Declaration

```
var hostname = "web01.example.com"
var domain = "example.com"
var http_port = 80
var enable_ssl = true
var dns_servers = ["10.0.0.1", "10.0.0.2"]
var luks_uuid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"
```

### Types

Types are inferred from the value. Explicit type annotation is optional but available:

```
var http_port: int = 80
var hostname: string = "web01.example.com"
var enable_ssl: bool = true
var dns_servers: list[string] = ["10.0.0.1", "10.0.0.2"]
var disk_size: size = 50G
var permissions: mode = 0644
```

| Type            | Description                                    | Examples                           |
|-----------------|------------------------------------------------|------------------------------------|
| `string`        | Text value. Quoted.                            | `"hello"`, `"/dev/sda"`           |
| `int`           | Integer.                                       | `80`, `0`, `-1`                    |
| `bool`          | Boolean.                                       | `true`, `false`                    |
| `size`          | Storage size with unit.                        | `1G`, `500M`, `2T`, `remaining`    |
| `mode`          | Octal file permission.                         | `0644`, `0755`, `0400`             |
| `list[T]`       | Ordered list of type T.                        | `["a", "b"]`, `[80, 443]`         |
| `map[K, V]`     | Key-value map.                                 | `{ a = 1, b = 2 }`                |
| `path`          | Filesystem path.                               | `/etc/httpd/conf`                  |
| `reference`     | Pointer to another declaration.                | `storage.sda.efi`                  |

### String Interpolation

Variables can be embedded in strings with `${}`:

```
var hostname = "web01"
var domain = "example.com"
var fqdn = "${hostname}.${domain}"    # "web01.example.com"

file hostname {
    content = "${fqdn}"
}
```

Interpolation is resolved at compile time. References to undefined variables are compile errors.

### Scope

Variables are visible to the scope they are declared in and all child scopes. A child scope can shadow a parent's variable.

```
var owner = "root"

dir etc {
    # owner is "root" here

    dir httpd {
        var owner = "apache"
        # owner is "apache" here and in children

        file httpd.conf {
            owner = ${owner}    # "apache"
        }
    }

    dir ssh {
        file sshd_config {
            owner = ${owner}    # "root" — httpd's shadow doesn't leak here
        }
    }
}
```

### System-Level Variables

Variables declared at the top level of a system declaration are global to that system:

```
system web01 {
    var env = "production"
    var region = "us-east-1"
    var base_domain = "example.com"

    # All declarations in this system can reference ${env}, ${region}, etc.
}
```

---

## References (Pointers)

References are dotted paths that point to named blocks elsewhere in the declaration tree. They are the mechanism for connecting declarations across scopes without duplication.

### Syntax

```
reference = identifier ("." identifier)*
```

### Pointing to Storage

```
storage.sda                         # the disk /dev/sda declaration
storage.sda.efi                     # the partition/filesystem named "efi" on sda
storage.sda.system.vg0              # the LVM VG named "vg0" inside LUKS "system" on sda
storage.sda.system.vg0.root         # the filesystem named "root" in vg0
```

The `storage` prefix is implicit when referencing from within the storage tree. From outside the storage tree (e.g., from a class or system block), the full dotted path is required.

### Pointing to Classes and Functions

```
class.httpd_server                  # the class named httpd_server
class.httpd_server.httpd_conf       # a named file inside the class
func.make_vhost                     # a function named make_vhost
```

### Pointing to Variables

```
var.hostname                        # top-level variable
var.http_port
```

### Using References

References are used anywhere a value or a block is expected:

```
# In a property — reference resolves to a value
bootloader grub2 "main" {
    esp = storage.sda.efi           # resolves to the efi partition block
}

# In an apply — reference resolves to a class
system web01 {
    apply class.httpd_server
}

# In a function call — reference resolves to a function
dir etc/httpd/conf.d {
    embed func.make_vhost("api", 8080)
}
```

---

## Functions

Functions are parameterized tree fragments. They accept arguments and return a set of declarations that the compiler inserts at the call site.

### Declaration

```
func make_vhost(name: string, port: int) {
    file ${name}.conf {
        mode = 0644
        owner = root
        group = apache
        selinux = httpd_config_t
        content = """
            <VirtualHost *:${port}>
                ServerName ${name}.${var.base_domain}
                DocumentRoot /var/www/${name}
            </VirtualHost>
        """
    }

    dir /var/www/${name} {
        owner = apache
        group = apache
        mode = 0755
        selinux = httpd_sys_content_t
    }
}
```

### Invocation

Functions are invoked with `embed` inside any block that accepts children:

```
dir etc/httpd/conf.d {
    embed func.make_vhost("api", 8080)
    embed func.make_vhost("admin", 8443)
    embed func.make_vhost("public", 80)
}
```

The compiler expands each `embed` call inline, producing concrete `file` and `dir` blocks with the arguments substituted. After expansion, the result is indistinguishable from hand-written declarations.

Functions declared with absolute paths (like the `/var/www/${name}` directory above) project into the storage tree via the same mount-point resolution as classes. Functions declared with relative paths insert relative to the call site.

### Pure Functions

Functions are pure — they produce declarations, not side effects. A function cannot:
- Modify variables outside its scope
- Perform I/O
- Reference mutable state

A function can:
- Declare files, directories, and sub-blocks
- Reference variables from its enclosing scope (read-only)
- Call other functions
- Use conditionals and loops over its arguments

### Return Type

Functions don't have an explicit return. The body *is* the return — every declaration in the function body is inserted at the call site. This is template expansion, not function evaluation.

---

## Conditionals

Conditionals control which declarations are included in the compiled tree.

```
var enable_ssl = true

dir etc/httpd/conf.d {
    file http.conf {
        mode = 0644
        content = """
            <VirtualHost *:80>
                ServerName ${var.hostname}
            </VirtualHost>
        """
    }

    if ${enable_ssl} {
        file ssl.conf {
            mode = 0644
            source = files/ssl.conf
        }

        file /etc/pki/tls/certs/server.crt {
            mode = 0444
            source = certs/${var.env}/server.crt
        }
    }
}
```

### Syntax

```
if expression {
    # declarations included when expression is true
}

if expression {
    # true branch
} else {
    # false branch
}

if expression {
    # ...
} elif expression {
    # ...
} else {
    # ...
}
```

Conditionals are evaluated at compile time. The expression must resolve to a boolean. The losing branch is discarded entirely — it does not appear in the compiled tree or the intermediate manifest.

### Expressions

```
# Comparison
if ${var.env} == "production" { ... }
if ${var.http_port} > 1024 { ... }
if ${var.http_port} != 80 { ... }

# Boolean logic
if ${var.enable_ssl} && ${var.env} == "production" { ... }
if ${var.enable_ssl} || ${var.force_tls} { ... }
if !${var.debug_mode} { ... }

# List membership
if "noexec" in ${var.mount_options} { ... }

# Existence check — is a reference valid?
if exists(storage.sda.system) { ... }
if exists(var.custom_config) { ... }
```

### Operator Precedence

Operators are listed from highest to lowest precedence. Operators on the same line have the same precedence and are left-associative.

| Precedence | Operators       | Description                          |
|------------|-----------------|--------------------------------------|
| 1 (highest)| `!`            | Logical NOT (unary, right-to-left)   |
| 2          | `>`, `<`, `>=`, `<=` | Relational comparison          |
| 3          | `==`, `!=`      | Equality comparison                  |
| 4          | `in`            | List membership                      |
| 5          | `&&`            | Logical AND                          |
| 6 (lowest) | `\|\|`          | Logical OR                           |

Parentheses override precedence:

```
# Without parentheses: && binds tighter than ||
if ${a} || ${b} && ${c} { ... }       # parsed as: a || (b && c)

# With parentheses: explicit grouping
if (${a} || ${b}) && ${c} { ... }     # parsed as: (a || b) && c
```

This precedence matches C, Rust, and most other languages operators are familiar with. The `in` operator sits between equality and logical AND, so `x == 1 || y in ${list} && z` parses as `(x == 1) || ((y in list) && z)`.

---

## Loops

Loops generate repetitive declarations from lists or ranges.

```
var vhosts = [
    { name = "api", port = 8080 },
    { name = "admin", port = 8443 },
    { name = "public", port = 80 }
]

dir etc/httpd/conf.d {
    for vhost in ${vhosts} {
        embed func.make_vhost(${vhost.name}, ${vhost.port})
    }
}
```

### Syntax

```
for identifier in expression {
    # declarations generated once per item
}

for i in range(1, 5) {
    # declarations generated for i = 1, 2, 3, 4
}
```

Loops are unrolled at compile time. The compiler expands the loop body for each iteration, substituting the loop variable, and the result is indistinguishable from hand-written declarations. There is no runtime looping.

### Range

```
for i in range(0, 3) {
    dir "data${i}" {
        mode = 0755
    }
}
# produces: dir data0, dir data1, dir data2
```

---

## Classes

Classes are reusable, inheritable tree fragments. They are the primary mechanism for composition across systems.

### Declaration

```
class hardened_base {
    description = "Base hardening for all RHEL-family servers"

    var sshd_port = 22

    file /etc/ssh/sshd_config {
        owner = root
        group = root
        mode = 0600
        source = files/sshd_config_hardened
    }

    file /etc/issue {
        mode = 0644
        content = "Authorized access only."
    }

    dir /var/log/audit {
        owner = root
        group = root
        mode = 0700
        selinux = auditd_log_t
    }
}
```

### Inheritance

```
class httpd_server extends hardened_base {
    # Inherits everything from hardened_base
    # Can add, override, or remove

    var http_port = 80

    file /etc/httpd/conf/httpd.conf {
        mode = 0644
        selinux = httpd_config_t
        source = files/httpd.conf
    }

    # Override inherited property
    file /etc/issue {
        content = "Web server. Authorized access only."
    }

    dir /var/www/html {
        owner = apache
        group = apache
        mode = 0755
        selinux = httpd_sys_content_t
    }
}
```

Inheritance is single-inheritance only. The derived class gets all declarations from the parent. Property-level override rules are identical to the merge semantics defined in `FILESYSTEM_SYNTAX.md` — derived class properties win over base class properties, with soft warnings on conflict.

### Classes Containing Subsystem Blocks

Classes can contain any declaration type — not just files and directories. This is how the bootloader, SELinux policy, firewall rules, services, and other subsystems become composable:

```
class hardened_bootloader {
    description = "GRUB2 UEFI bootloader with password protection and serial console"

    var grub_password_hash = ""    # must be overridden by derived class or system

    bootloader grub2 "main" {
        target = uefi
        timeout = 5
        timeout_style = menu

        password {
            user = "root"
            hash = ${grub_password_hash}
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
        }
    }
}

class web_server_boot extends hardened_bootloader {
    var grub_password_hash = "grub.pbkdf2.sha512.10000.ABC123..."

    # Add entries specific to this role
    bootloader grub2 "main" {
        default = "alma"
        esp = storage.sda.efi

        params "base" {
            root = "UUID=${var.root_uuid}"
            ro = true
            selinux = 1
            enforcing = 1
            quiet = true
        }

        entry "alma" {
            kernel = /boot/vmlinuz-current
            initrd = /boot/initramfs-current.img
            params = params.base
        }
    }
}
```

When a derived class declares the same block name (`bootloader grub2 "main"`), the compiler performs a **deep merge** — properties and child blocks from both classes are merged using the standard conflict resolution rules (derived wins, warning emitted).

### Applying Classes

```
system web01 {
    var root_uuid = "aaaa-bbbb-cccc-dddd"

    apply hardened_base
    apply httpd_server
    apply web_server_boot
}
```

Multiple `apply` statements compose additively. Conflicts between classes follow the merge rules: later `apply` wins over earlier, inline wins over all, soft warnings on every conflict.

---

## The `embed` Keyword

`embed` inserts the output of a function or the contents of a class at a specific point in the tree. It is the general-purpose composition operator.

### Embedding Functions

```
dir etc/httpd/conf.d {
    embed func.make_vhost("api", 8080)
}
```

The function's output (files, directories) is inserted as children of the enclosing block.

### Embedding Classes Inline

```
xfs root {
    size = 50G
    mount = /

    embed class.hardened_base
}
```

When a class is embedded inside a filesystem block, its absolute-path declarations are resolved relative to that filesystem's mount point. Its relative-path declarations are inserted as children of the enclosing block.

### Embedding Files from External Sources

```
dir etc/httpd/conf.d {
    embed include("conf.d/*.ic")    # include all .ic files matching the glob
}
```

`include()` reads other Ironclad source files and inserts their declarations at the call site. This is the mechanism for splitting large declarations across multiple files.

---

## Imports

Imports bring classes, functions, and variables from other Ironclad source files into scope.

```
import "base/hardened.ic"                    # import everything from file
import "base/hardened.ic" { hardened_base }   # import specific class
import "lib/vhosts.ic" { make_vhost }        # import specific function
```

Imports are resolved at compile time relative to the Ironclad source tree root. Circular imports are compile errors.

### Standard Library Imports

```
import std.hardened_rhel_base
import std.s6_container_host
import std.selinux_mls_workstation
```

Standard library classes are prefixed with `std.` and ship with the Ironclad compiler.

---

## Putting It Together: Bootloader in the Tree

With language primitives defined, the bootloader configuration can be authored as a class and embedded in the storage tree — or written directly inline. Both resolve to the same compiled form.

### Option 1: Bootloader as a Class, Pointed into the Tree

```
# --- bootloader.ic ---

class alma_bootloader {
    var root_uuid = ""              # must be provided by system
    var grub_hash = ""              # must be provided by system

    bootloader grub2 "main" {
        target = uefi
        timeout = 5
        default = "alma"

        password {
            user = "root"
            hash = ${grub_hash}
        }

        params "base" {
            root = "UUID=${root_uuid}"
            ro = true
            selinux = 1
            enforcing = 1
        }

        entry "alma" {
            kernel = /boot/vmlinuz-current
            initrd = /boot/initramfs-current.img
            params = params.base
        }

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

    # Bootloader files that live on the ESP
    dir /boot/efi/EFI/almalinux {
        file grubx64.efi { package = grub2-efi-x64 }
        file grub.cfg {
            mode = 0600
            # content generated by compiler from bootloader block above
            content = generated
        }
    }
}

# --- system.ic ---

import "bootloader.ic" { alma_bootloader }

system web01 {
    var root_uuid = "aaaa-bbbb-cccc-dddd"
    var grub_hash = "grub.pbkdf2.sha512.10000.ABC..."

    apply alma_bootloader

    disk /dev/sda {
        label = gpt

        fat32 efi {
            size = 1G
            type = ef00
            mount = /boot/efi
            # class alma_bootloader's /boot/efi files are merged here by compiler
        }

        luks2 system {
            lvm vg0 {
                xfs root {
                    size = 50G
                    mount = /
                }
            }
        }
    }
}
```

### Option 2: Bootloader Written Directly in the Tree

```
system web01 {
    var root_uuid = "aaaa-bbbb-cccc-dddd"
    var grub_hash = "grub.pbkdf2.sha512.10000.ABC..."

    disk /dev/sda {
        label = gpt

        fat32 efi {
            size = 1G
            type = ef00
            mount = /boot/efi

            bootloader grub2 "main" {
                target = uefi
                timeout = 5
                default = "alma"

                password {
                    user = "root"
                    hash = ${grub_hash}
                }

                params "base" {
                    root = "UUID=${root_uuid}"
                    ro = true
                    selinux = 1
                    enforcing = 1
                }

                entry "alma" {
                    kernel = /boot/vmlinuz-current
                    initrd = /boot/initramfs-current.img
                    params = params.base
                }
            }

            dir EFI/almalinux {
                file grubx64.efi { package = grub2-efi-x64 }
            }
        }

        luks2 system {
            lvm vg0 {
                xfs root {
                    size = 50G
                    mount = /
                }
            }
        }
    }
}
```

### Option 3: Function for Parameterized Bootloader

```
func grub2_uefi_boot(root_uuid: string, grub_hash: string, default_entry: string) {
    bootloader grub2 "main" {
        target = uefi
        timeout = 5
        default = ${default_entry}

        password {
            user = "root"
            hash = ${grub_hash}
        }

        params "base" {
            root = "UUID=${root_uuid}"
            ro = true
            selinux = 1
            enforcing = 1
        }

        entry ${default_entry} {
            kernel = /boot/vmlinuz-current
            initrd = /boot/initramfs-current.img
            params = params.base
        }

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
}

# Usage in the tree:
fat32 efi {
    size = 1G
    type = ef00
    mount = /boot/efi

    embed func.grub2_uefi_boot(${root_uuid}, ${grub_hash}, "alma")
}
```

All three options compile to the same intermediate representation.

---

## The `generated` Keyword

Some files have content that is produced by the compiler from other declarations, not authored by the operator. The `generated` keyword marks these:

```
file grub.cfg {
    mode = 0600
    content = generated     # compiler produces this from the bootloader block
}

file /etc/selinux/targeted/policy/policy.33 {
    content = generated     # compiler produces this from SELinux declarations
}

file /etc/nftables.conf {
    content = generated     # compiler produces this from firewall declarations
}
```

`generated` tells the compiler:
1. This file's content comes from a compiler backend, not from `source` or inline `content`
2. The runtime agent should still monitor this file for drift
3. The file is included in the intermediate manifest with its generated hash

The operator can override a `generated` file by declaring `content` or `source` instead — the standard merge rules apply (inline wins, warning emitted that a compiler-generated file is being manually overridden).

---

## Constraints

Constraints are compile-time assertions that validate properties of the resolved tree. They are the mechanism for expressing preconditions, invariants, and requirements that go beyond structural validation.

### System-Level Constraints

```
system web01 {
    constraint "root partition minimum size" {
        check = size_of(storage.nvme0n1.system.vg_system.root) >= 20G
    }

    constraint "SELinux must be enforcing" {
        check = selinux.mode == "enforcing"
    }

    constraint "at least two DNS servers" {
        check = length(network.dns.servers) >= 2
    }
}
```

### Class-Level Constraints

Classes can declare constraints that are validated when the class is applied. This is how a class expresses its preconditions:

```
class httpd_server {
    constraint "httpd package required" {
        check = exists(packages.httpd)
        message = "httpd_server class requires the httpd package to be declared"
    }

    constraint "firewall must allow port 80 or 443" {
        check = exists(firewall) && (
            port_allowed(80, "tcp") || port_allowed(443, "tcp")
        )
        message = "httpd_server expects at least HTTP or HTTPS in the firewall"
        severity = warning     # warning | error (default: error)
    }
}
```

### Constraint Properties

| Property   | Type     | Default  | Description                                              |
|------------|----------|----------|----------------------------------------------------------|
| `check`    | `bool`   | required | Expression that must evaluate to `true` at compile time. |
| `message`  | `string` | (none)   | Custom error/warning message shown on failure.           |
| `severity` | enum     | `error`  | `error` (compile fails) or `warning` (compile continues).|

### Built-In Constraint Functions

| Function          | Returns | Description                                              |
|-------------------|---------|----------------------------------------------------------|
| `exists(ref)`     | `bool`  | Whether a reference resolves to a declaration.           |
| `size_of(ref)`    | `size`  | The declared size of a storage or filesystem block.      |
| `length(list)`    | `int`   | Number of elements in a list.                            |
| `count(ref)`      | `int`   | Number of child declarations under a block.              |
| `port_allowed(port, proto)` | `bool` | Whether the firewall permits the given port/protocol. |
| `user_exists(name)` | `bool` | Whether a user with the given name is declared.        |

Constraints are evaluated after class resolution and merge, but before backend emission. They see the fully resolved tree.

---

## Project Structure

Every Ironclad project has a root directory containing an `Ironclad.toml` manifest file. This file identifies the project, configures the compiler, and declares paths.

### `Ironclad.toml`

```toml
[project]
name = "web-fleet"
version = "1.0.0"
description = "Production web server fleet"

[compiler]
security_floor = "strict"          # baseline | standard | strict | maximum
output = "build/"                  # directory for emitted artifacts
target = "bootc"                   # bootc | kickstart | osbuild

[paths]
source = "systems/"                # directory containing .ic source files
files = "files/"                   # directory containing static files (source = ...)
templates = "templates/"           # directory containing template files
stdlib = "stdlib/"                 # optional: local stdlib override

[signing]
key = "keys/signing.key"          # Ed25519 private key for manifest signing
```

### Project Layout

```
web-fleet/
├── Ironclad.toml                  # project manifest
├── systems/
│   ├── web01.ic                   # system declaration
│   ├── web02.ic
│   └── topology.ic               # fleet topology
├── classes/
│   ├── base.ic                   # organization base class
│   ├── web.ic                    # web server role class
│   └── monitoring.ic
├── files/
│   ├── sshd_config_hardened
│   ├── httpd/
│   │   └── httpd.conf
│   └── certs/
├── templates/
│   └── httpd/
│       └── vhost.conf.tpl
├── keys/
│   └── signing.key
└── build/                         # compiler output (gitignored)
```

### Path Resolution

- `source = files/httpd.conf` in a file declaration resolves relative to `[paths].files`
- `template = templates/vhost.conf.tpl` resolves relative to `[paths].templates`
- `import "classes/web.ic"` resolves relative to the project root
- `import std.hardened_rhel_base` resolves against the compiler's bundled stdlib, then `[paths].stdlib` if set

---

## Reserved Keywords (Language-Level)

The following words are reserved across all contexts:

`var`, `class`, `func`, `extends`, `apply`, `embed`, `import`, `include`, `if`, `elif`, `else`, `for`, `in`, `range`, `exists`, `true`, `false`, `remove`, `generated`, `system`, `std`, `constraint`, `check`, `message`, `severity`

These are in addition to domain-specific reserved keywords defined in `STORAGE_SYNTAX.md`, `BOOTLOADER_SYNTAX.md`, `FILESYSTEM_SYNTAX.md`, and `SELINUX_SYNTAX.md`.

---

## Grammar Summary (Informative)

```
program             = (import_stmt | var_decl | class_decl | func_decl | system_decl)*

import_stmt         = "import" string ("{" identifier ("," identifier)* "}")?

var_decl            = "var" identifier (":" type)? "=" expression

class_decl          = "class" identifier ("extends" identifier)? "{" class_body "}"
class_body          = (var_decl | property | dir_block | file_block | subsystem_block | if_block | for_block | embed_stmt)*

func_decl           = "func" identifier "(" param_list? ")" "{" func_body "}"
param_list          = param ("," param)*
param               = identifier ":" type
func_body           = (var_decl | dir_block | file_block | subsystem_block | if_block | for_block | embed_stmt)*

system_decl         = "system" identifier "{" system_body "}"
system_body         = (var_decl | apply_stmt | storage_decl | subsystem_block | if_block | for_block | embed_stmt)*

apply_stmt          = "apply" reference
embed_stmt          = "embed" (func_call | reference | include_call)

func_call           = "func." identifier "(" arg_list? ")"
arg_list            = expression ("," expression)*
include_call        = "include" "(" string ")"

if_block            = "if" expression "{" body "}" ("elif" expression "{" body "}")* ("else" "{" body "}")?
for_block           = "for" identifier "in" expression "{" body "}"

subsystem_block     = selinux_decl | firewall_decl | init_decl | network_decl
                    | users_decl | packages_decl
                    # compiler-native subsystems, each in its own spec

constraint_decl     = "constraint" string "{" constraint_body "}"
constraint_body     = "check" "=" expression ("message" "=" string)? ("severity" "=" ("error" | "warning"))?

expression          = literal | reference | interpolated_string | func_call
                    | expression binop expression | unop expression
                    | "exists" "(" reference ")" | expression "in" expression
binop               = "==" | "!=" | ">" | "<" | ">=" | "<=" | "&&" | "||"
unop                = "!"

reference           = identifier ("." identifier)*
interpolated_string = '"' (text | "${" expression "}")* '"'
literal             = string | int | bool | size | mode | list | map

type                = "string" | "int" | "bool" | "size" | "mode" | "path" | "reference"
                    | "list" "[" type "]" | "map" "[" type "," type "]"
```

---

## What This Document Does Not Cover

This specification covers the core language primitives. The following topics are defined in separate specifications:

**Compiler-native subsystems** (first-class syntax, cross-validation):
- **Storage topology** — Block devices, encryption, volume management (`STORAGE_SYNTAX.md`)
- **Filesystem declarations** — Files, directories, merge semantics (`FILESYSTEM_SYNTAX.md`)
- **SELinux policy** — Policy types, MLS parameters (`SELINUX_SYNTAX.md`)
- **Services and init systems** — Service declarations, supervision trees (`INIT_SYNTAX.md`)
- **Firewall / network policy** — nftables rule declarations (`FIREWALL_SYNTAX.md`)
- **Network interfaces** — Interface configuration, addressing, routing (`NETWORK_SYNTAX.md`)
- **Users and groups** — Account declarations, password policy (`USERS_SYNTAX.md`)
- **Packages** — Package list and repository declarations (`PACKAGES_SYNTAX.md`)

**Standard library class APIs** (emit files, validated structurally):
- **Bootloader configuration** — Boot entries, kernel parameters, UKI (`BOOTLOADER_SYNTAX.md`)
- **File operations** — Structured file editing, text operations, raw bash (`FILEOPS_SYNTAX.md`)
- **Secret management** — Secrets as typed variables with generation/distribution policies (`SECRETS_SYNTAX.md`)
