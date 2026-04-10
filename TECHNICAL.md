# Technical Overview — Ironclad

This document describes the philosophy, architecture, language design, compiler pipeline, standard library model, topology system, and runtime model of Ironclad. It is intended for contributors, reviewers, and engineers evaluating the project's approach.

---

## Philosophy

Linux system configuration is a solved problem in pieces. `cryptsetup` knows how to encrypt a disk. `semodule` knows how to load policy. `dnf` knows how to install packages. `useradd` knows how to create accounts. `nft` knows how to load firewall rules. Every individual tool works. The tools are tested, certified, audited, and trusted.

What no tool validates is the relationships between the pieces.

A service runs as user `postgres` with SELinux type `postgresql_t`, listening on port 5432, with data on an encrypted XFS volume mounted at `/var/lib/pgsql`, and a firewall rule allowing TCP 5432 from the application subnet. Today, each of those facts is configured separately with separate tools — `useradd`, `semanage`, `systemctl`, `cryptsetup`, `mkfs.xfs`, `mount`, `nft` — and the only thing that validates they all agree with each other is a human reading five different configuration files and hoping they didn't miss a contradiction. Or a 3am page when the system tells them.

**Ironclad is the compiler for that validation problem.** You write the declaration once. The compiler proves the pieces are consistent — at compile time, not at 3am. Then it hands the pieces to the tools that already know how to execute them.

This philosophy has several consequences:

### The compiler carries the cognitive load

The person who can hold the full picture — storage topology, encryption, SELinux policy, service identities, firewall rules, network interfaces, user accounts — in their head and verify consistency across all of them is rare and expensive. Ironclad moves that verification from human expertise to compiler guarantee. The compiler is not replacing the tools. It is replacing the human effort of verifying that the tools are being used consistently with each other.

### Orchestrate, don't reimplement

Ironclad does not reimplement `parted`, `cryptsetup`, `dnf`, or `semodule`. It generates validated configuration for them and invokes them in the correct order. This keeps Ironclad's codebase small, its attack surface minimal, and its trust inherited from the platform. An auditor does not need to trust Ironclad's implementation of disk encryption — they trust `cryptsetup`, which they've already certified. Ironclad proves the inputs to `cryptsetup` are consistent with everything else.

### Bash is the universal constant

Every Linux system has a shell. A datacenter server running RHEL has bash. A Raspberry Pi running a custom image has bash. A Linux-based point-of-sale terminal has bash. An embedded controller has at least `sh`. This is why Ironclad's toolchain emits bash as its universal fallback — it runs everywhere Linux runs. Where a more specific certified tool exists (Kickstart, `dnf`, `semodule`), the toolchain uses it. Where none exists, bash handles it. This combination — certified tools where possible, bash where necessary — is what makes the same language work from a 10,000-node datacenter fleet to a single-board computer.

### The full lifecycle, not just build

A system is not done when it boots. Ironclad covers the full lifecycle: **build** (compile declaration to toolchain, execute toolchain against target), **runtime** (agent compares live state against signed manifest, reports drift), and **redeployment** (compiler diffs old and new declarations, emits delta toolchain, agent verifies convergence). The declaration is the source of truth at every point — not just at install time.

### Anything that runs Linux

Ironclad's scope is not limited to servers. If it runs Linux, Ironclad can describe it: a hardened enterprise server, a developer desktop, a Kubernetes cluster, a fleet of edge nodes, a home NAS, a point-of-sale kiosk, a Raspberry Pi, a Linux-based calculator. The language is the same. The compiler is the same. The validation is the same. The stdlib provides different base classes for different targets — `HardenedRHELServer`, `SecureWorkstation`, `EmbeddedAppliance` — but the underlying model (declare the system, prove it consistent, hand it to the tools) does not change.

### Trust inheritance, not trust creation

By generating configuration for tools that are already audited and certified, Ironclad inherits their trust rather than asking users to trust a new implementation. This is strategic for regulated environments: the certification chain from a certified ISO to a running system is preserved because every tool the toolchain calls is a tool the certification body already approved. But it also matters for everyone else — using `useradd` instead of writing `/etc/passwd` directly is safer regardless of whether anyone is auditing you.

---

## Architectural Philosophy

The philosophy above — validate relationships, orchestrate certified tools, cover the full lifecycle — is implemented through a deliberate separation between compiler and standard library. The compiler has native knowledge of six tightly coupled domains: **storage topology**, **the filesystem tree**, **SELinux policy**, **services and init systems**, **firewall rules**, and **network interfaces**. These six domains cross-validate against each other — a service binds a port that a firewall rule must allow, runs as a user with an SELinux label, writes to files on a filesystem backed by declared storage, and listens on a declared network interface. The compiler needs to understand all six to deliver compile-time validation of these relationships.

Beyond these six domains, the compiler does not have built-in knowledge of specific subsystems. Bootloader configuration, secrets management, file format editing, Kubernetes manifests, libvirt XML, Podman Quadlet files, and other domain-specific configurations are the responsibility of the standard class library, which is written in Ironclad itself. These subsystems are configured by writing files to the filesystem — and the file primitive, combined with the class system, is powerful enough to encapsulate the knowledge of what "right" means for each subsystem so that engineers do not have to rediscover it for every system they build.

The compiler's job is structural correctness and cross-domain validation: no conflicting declarations for the same path, no files on undeclared mount points, no mutable files on read-only filesystems, enforcement of the security floor, correct targeted policy derived from the system's global topology, bidirectional port validation between services and firewall rules, and identity validation across users, services, and SELinux contexts. The standard library's job is domain correctness for everything outside the core validation loop: bootloader configuration, secrets backend integration, structured file editing, container orchestration, virtualization, and other subsystem-specific knowledge.

---

## Target Audiences

Ironclad is an expert-grade language that serves three primary audiences and benefits a fourth:

**Security and compliance engineers** need auditability, provable compliance, and reproducible systems — from classified datacenters to compliance-sensitive consumer devices. Ironclad gives them compile-time cross-validation (SELinux policy, firewall rules, service identities, file permissions all checked against each other), signed manifests for drift detection, a non-negotiable security floor, and an inspectable build toolchain that an auditor can read end to end. The emitted toolchain maximizes dependency on certified tools — Kickstart, Anaconda, `dnf`, `cryptsetup` — so that the certification chain from ISO to running system is preserved and verifiable.

**Low-level system designers and distribution developers** need full control over every layer of the system. Ironclad's nesting-mirrors-reality storage model, raw escape hatches, and multiple build targets (certified ISO, bare disk, chroot, OCI image) give them the ability to build anything from a hardened enterprise server to a custom Linux distribution to an embedded appliance. A distro developer could use Ironclad as the backend for defining and pushing their own distribution. A hardware vendor could use it to define the firmware and OS for an IoT gateway. The class system makes system profiles composable and distributable.

**Engineers who want secure, reproducible systems without deep specialization** benefit through the standard library class system. Whether the target is a desktop workstation, a home server, a Raspberry Pi, or a production web server — a well-authored class encapsulates years of domain expertise behind a declaration that reads like a configuration file. The stdlib does the heavy lifting; the engineer declares intent.

**AI agents** are a natural fit as a fourth audience. Ironclad's structured, typed, compile-time-validated syntax is exactly what an AI is good at producing. An AI can generate `.ic` files from natural language requirements, the compiler validates the result before anything touches hardware, and the human reviews structured declarations rather than shell scripts or live system state. The AI never needs SSH access or live system knowledge — it writes code, the compiler catches mistakes, the toolchain executes. This makes secure system configuration accessible to anyone who can describe what they want, regardless of what they're building.

---

## Architecture Overview

Ironclad operates across four modes spanning the full system lifecycle:

**Build time** — The compiler parses Ironclad source, resolves the class hierarchy, performs structural and semantic validation, and emits a build toolchain. The toolchain orchestrates existing certified tools — Kickstart, Anaconda, `parted`, `cryptsetup`, `dnf`, `useradd`, `semodule`, `nft`, `grub2-install` — feeding them validated configuration derived from the declaration. Where no certified tool exists for an operation, the compiler emits bash. The output is always inspectable: an ordered set of scripts and configuration files that an auditor can read end to end.

**Install time** — The emitted toolchain executes against a target. The primary target is a certified minimal ISO (RHEL, AlmaLinux, Fedora), where the toolchain preserves the ISO's signed package chain and certification status. But the same declaration can also target a bare disk for Linux From Scratch-style builds, a chroot for development, or an OCI image for container-native deployments. The toolchain adapts its bootstrap phase to the target; the configuration phases are identical.

**Runtime auditing** — The runtime agent, installed during build, periodically compares live system state against the signed manifest. Drift is reported as structured output.

**Runtime maintenance** — When the system declaration changes, the compiler diffs the old and new manifests and emits a delta toolchain — the minimum set of operations to move from the old state to the new state. The runtime agent verifies convergence after application.

---

## Core Principles

### Atomicity

Every state transition on an Ironclad-managed system is atomic. The system exists in the old declared state or the new declared state; no intermediate condition is observable. The emitted toolchain scripts are structured so that each phase either completes fully or rolls back. For runtime maintenance deltas, the delta toolchain is structured for atomic application where the underlying operations support it, and the runtime agent verifies convergence before reporting success.

### Immutability

Ironclad defaults to the maximum immutability the target platform supports. When the build target supports read-only roots (e.g., via `mount -o ro`, overlayfs, or dm-verity), the compiler emits the toolchain steps to enforce it. Mutable state is confined to paths that the declaration explicitly marks as writable. The compiler enforces this: a file declared on a read-only filesystem without a corresponding writable overlay or bind mount is a compile-time error. The runtime agent treats any modification to an immutable path as drift. Mutability is never prohibited — it is required to be explicit.

---

## Language Design

### Core Primitives

The language's type system is built around filesystem objects and their metadata. The core primitives are:

**Files** — Declared with a path, content (inline literal, template with variable interpolation, or binary hash reference), permissions, ownership, SELinux label, and mutability flag.

**Directories** — Declared with a path, permissions, ownership, SELinux label, and mutability flag. May contain nested file and directory declarations.

**Symlinks** — Declared with a source path and target path.

**Mount points** — Declared with a device, path, filesystem type, and mount options. The compiler validates that files declared beneath a mount point are consistent with the mount's properties.

**Packages** — Declared by name and optional version constraint. Packages are a build-time directive: the compiler includes them in the emitted toolchain's package installation phase.

**Users and groups** — Declared with the attributes that `/etc/passwd`, `/etc/shadow`, and `/etc/group` understand. The compiler ensures these declarations are consistent.

In addition to these filesystem primitives, the compiler has native support for three subsystem domains that participate in the cross-validation loop:

**Services** — Declared with a name, executable, identity (user, group, SELinux label), dependencies, and resource limits. The compiler validates service identities against user and SELinux declarations, cross-references bound ports against firewall rules, and emits backend-specific artifacts (systemd units or s6 service directories).

**Firewall rules** — Declared as tables, chains, and rules mapping to nftables concepts. The compiler validates interface references against network declarations, cross-references allowed ports against service declarations, and generates `/etc/nftables.conf`.

**Network interfaces** — Declared with type, addressing, and topology. The compiler validates firewall interface references, service bind addresses, and cross-system network references in topology mode.

These six compiler-native domains (storage, filesystem, SELinux, services, firewall, network) form a closed validation loop. Subsystems outside this loop — bootloader configuration, secrets management, file format editing, VM definitions, container specifications, Kubernetes manifests — are handled by standard library classes that emit files. The compiler places those files in the image through the filesystem primitive without needing to understand their formats.

### General-Purpose Constructs

The language provides variables, loops, conditionals, and class definitions. These operate over the domain-typed primitives. A variable is not an untyped string; it has a type that the compiler validates in context. A loop can replicate a file declaration across a set of paths. A conditional can include or exclude a configuration block based on a parameter. These constructs make the language expressive enough to describe complex, parameterized systems without sacrificing the compiler's ability to validate structure.

### Class System

Ironclad uses a single-inheritance object-oriented class system. A base class declares a complete or partial system configuration. Derived classes extend or override specific properties. The full hierarchy is flattened during the compiler's resolution pass; the resulting AST contains no unresolved inheritance, and every property has an explicit, traceable value and origin.

Classes are the unit of reuse and composition. A base server class declares the common configuration shared by all servers in an organization. A web server class inherits from it and adds the files specific to a web server role. A production web server class inherits from the web server class and overrides the logging configuration for production. This hierarchy is expressed once and produces consistent, traceable systems at any scale.

The object-oriented model was chosen over a functional approach because it maps to the way infrastructure teams reason about roles and role hierarchies, and because it makes the inheritance chain inspectable at any layer without requiring fluency in a functional paradigm. The tradeoff — deep hierarchies can become hard to follow — is managed by keeping the standard library shallow and emitting compiler warnings when inheritance depth exceeds a configurable threshold.

---

## Standard Class Library

The standard library is where domain expertise is encoded. It ships with Ironclad and provides classes for common subsystems and system roles. Every standard library class is written in Ironclad, inspectable, overridable, and forkable.

### Subsystem Classes

Subsystem classes encapsulate the knowledge of how a specific Linux subsystem is configured through the filesystem. Services, firewall rules, and network interfaces are compiler-native — they have first-class syntax and participate in the cross-validation loop. The standard library covers everything outside that loop. Examples:

A **bootloader class** accepts backend type (GRUB2, systemd-boot), kernel parameters, boot entries, and an ESP reference. It emits the appropriate configuration files (`grub.cfg`, `loader.conf`) and validates its storage references through the compiler's reference system.

A **secrets keeper class** accepts a backend type (Vault, age, SOPS, systemd-creds) and configuration parameters. It emits backend-specific configuration files and integrates with the compiler's `var secret` type for build-time and runtime secret resolution.

A **Kubernetes node class** accepts a role (control plane or worker), cluster parameters (API server address, token, certificate authority), and network configuration (CNI plugin, pod CIDR). It emits kubeadm configuration files, ensures the required kernel parameters are set, declares the container runtime packages, and configures the kubelet service via the compiler-native service declarations.

A **libvirt VM class** accepts resource allocations, network attachments, firmware type, and boot configuration. It emits a domain XML file and, if the VM should start automatically, a corresponding autostart symlink.

A **Podman container class** accepts an image reference, network bindings, volume mounts, resource limits, and restart policy. It emits a Quadlet `.container` file integrated with the init system.

### System Base Classes

System base classes compose subsystem classes into complete or near-complete system profiles. They span the full range of Linux deployments:

**Server classes:**

`HardenedRHELBase` — A minimal, hardened RHEL-family server with SELinux enforcing, LUKS2, an immutable root, and a locked-down user configuration. Intended as the foundation from which all server role classes inherit.

`SystemdServer` — A general-purpose server role using systemd, with common services (sshd, chrony, rsyslog) configured via subsystem classes.

`S6ContainerHost` — A container host using s6 for process supervision instead of systemd. Declares Podman, rootless container support, and an s6 service tree.

`KubernetesControlPlane` / `KubernetesWorker` — Kubernetes node roles inheriting from an appropriate server base, with the Kubernetes node class parameterized for the declared cluster topology.

**Desktop and workstation classes:**

`SecureWorkstation` — A hardened desktop environment with LUKS2, SELinux enforcing, a curated desktop package set, firewall defaults for a workstation (no inbound services except SSH), and user-level sandboxing. Inherits from `HardenedRHELBase` and adds display server, audio, and desktop application declarations.

`DeveloperWorkstation` — Extends `SecureWorkstation` with development toolchains, container runtimes, debuggers, and relaxed SELinux booleans for development workflows.

**Embedded and appliance classes:**

`EmbeddedAppliance` — A minimal, read-only-root system with a single declared purpose. No SSH by default, no package manager on the running system, watchdog-supervised services. Designed for kiosks, IoT gateways, point-of-sale terminals, and single-purpose devices.

`EdgeNode` — A hardened, remotely managed node designed for deployment at scale. Inherits from `HardenedRHELBase`, adds fleet management agent configuration, and exposes variables for per-node parameterization in topology loops.

### Custom Classes

Engineers are expected to write classes for configurations that the standard library does not cover. If a subsystem is configured by writing files — and virtually everything in Linux is — an Ironclad class can describe it. Custom classes use the same primitives, the same inheritance model, and the same validation as standard library classes. There is no distinction between "built-in" and "user-defined" at the language level.

---

## Topology and Fleet Composition

A system declaration in Ironclad is a first-class value. It can be assigned to a variable, parameterized, and composed with other system declarations. This is the mechanism for describing infrastructure at scale.

### Systems as Variables

A declared system — for example, a web server class parameterized with a specific hostname, IP address, and storage layout — is a value that can be bound to a variable. Multiple systems can be declared in the same source file, each as a separate variable. Systems can reference each other: a database server's firewall rules can reference the IP addresses of the application servers that connect to it, validated at compile time.

### Topology Declarations

A topology declaration composes a set of system declarations into a description of interconnected infrastructure. The topology expresses which systems exist, their network relationships, their physical or virtual placement, and any cross-system dependencies.

A Kubernetes cluster, for example, is not a special compiler feature. It is a topology: three control plane system declarations, ten worker system declarations, and a set of etcd system declarations, all inheriting from appropriate base classes and parameterized with their cluster roles. The topology declaration binds them together and ensures that the network configuration, certificate distribution, and bootstrap ordering are consistent.

A datacenter is a topology of topologies. A fleet of a thousand identical edge nodes is a base class, a loop with per-node parameters, and a topology declaration that maps them. The object-oriented model — inheritance, parameterization, variable assignment, composition — is what makes this tractable. Without it, describing a thousand nodes would require a thousand files or an external templating system that reintroduces the fragmentation Ironclad eliminates.

### Compile-Time Topology Validation

When the compiler resolves a topology, it can validate cross-system properties: network references between systems resolve to declared interfaces, port dependencies are satisfiable, no two systems in the same topology claim the same IP address, and aggregate resource demands of VMs and containers do not exceed their host systems' declared capacity. These validations are structural — the compiler does not need to understand the subsystem-specific semantics; it validates the relationships between declared filesystem objects across system boundaries.

---

## Compiler Pipeline

### Stage 1 — Parsing

The parser reads Ironclad source files and produces an abstract syntax tree. Implemented in Rust using pest (PEG grammar). The grammar is the canonical specification of valid syntax. Invalid input is rejected with structured diagnostics.

### Stage 2 — Class Resolution

The compiler traverses the class hierarchy, resolves inheritance, and flattens derived classes into fully specified AST nodes. For topology declarations, each composed system is resolved independently and then the cross-system references are linked. After this pass, every property has an explicit value with a traceable origin.

### Stage 3 — Semantic Validation

The compiler validates the resolved AST against structural rules and cross-domain consistency. Structural checks include: conflicting declarations for the same path, files on undeclared mount points, mutable files on read-only filesystems without writable overlays, security floor violations (SELinux enforcing mode, LUKS2, immutable root), and — for topologies — cross-system reference consistency. Cross-domain checks include: services reference declared users and SELinux types, firewall rules reference declared interfaces, service ports have corresponding firewall rules (and vice versa), network interface addresses are unique, and package references are satisfied. For subsystems outside the compiler's native scope (bootloader, secrets, file operations), validation is limited to structural properties — correct file paths, ownership, and permissions.

### Stage 4 — Manifest Generation

The compiler serializes the resolved AST into a signed intermediate manifest per system in the declaration. The manifest format is CBOR with an Ed25519 signature. For topologies, each system receives its own manifest; the topology-level relationships are encoded in a separate topology manifest that references the per-system manifests.

### Stage 5 — Backend Emission

The compiler emits a build toolchain for each system in the declaration. The toolchain is an orchestrator: for each operation, it selects the most appropriate certified tool and feeds it validated configuration derived from the resolved AST. Where a certified tool exists — Kickstart for partitioning, `dnf` for package management, `cryptsetup` for encryption, `semodule` for policy loading — the compiler generates configuration for that tool. Where no certified tool covers the operation, the compiler emits bash. The result is a set of ordered, inspectable scripts and configuration files that maximize dependency on audited, certified tooling while using bash as the universal fallback.

This design minimizes Ironclad's own codebase: the compiler does not reimplement `parted`, `mkfs`, or `useradd`. It generates the correct invocation with validated inputs. The less Ironclad reimplements, the smaller its attack surface and the greater the trust inherited from the underlying tools.

#### Toolchain Structure

The build toolchain is organized into ordered phases. Each phase selects the right tool for the job:

| Phase | Operation | Tools Used |
|-------|-----------|------------|
| 1. Storage | Partitioning, RAID, LUKS, LVM, filesystems, mounts | Kickstart `%pre`/`part`, `parted`, `mdadm`, `cryptsetup`, `pvcreate`/`lvcreate`, `mkfs.*`, `mount` |
| 2. Base install | Package installation from declared set | Kickstart `%packages`, Anaconda, `dnf --installroot`, or source build toolchain |
| 3. Files | Directory creation, file placement, permissions, attributes | `mkdir`, `install`, `chown`, `chmod`, `chattr`, bash |
| 4. Users/groups | Account creation, password policy, SSH keys | `groupadd`, `useradd`, `usermod`, `chpasswd`, `chage` |
| 5. Services | Unit files, service directories, enablement | `systemctl`, bash (for s6-rc compilation) |
| 6. Network | Interface configuration | `nmcli`, NetworkManager keyfiles, or systemd-networkd units |
| 7. Firewall | Ruleset generation and loading | `nft`, direct write of `/etc/nftables.conf` |
| 8. SELinux | Policy compilation, loading, relabeling | `checkmodule`, `semodule_package`, `semodule`, `restorecon` |
| 9. Bootloader | Bootloader installation and configuration | `grub2-install`, `grub2-mkconfig`, `bootctl`, bash |
| 10. Manifest | Signed manifest installation | Ironclad agent binary, bash |
| 11. Seal | Immutable bits, read-only root, cleanup | `chattr`, `mount -o remount,ro`, bash |

Each phase is a standalone script that can be inspected, modified, or executed independently. The toolchain emits a top-level orchestrator that runs the phases in order.

The key principle: **when a certified tool can do the job, the compiler generates config for that tool. When it can't, the compiler generates bash.** This keeps Ironclad's codebase small and maximizes trust inheritance from the underlying platform.

#### Build Targets

The compiler supports multiple build targets, selected via `Ironclad.toml` or command-line flag:

| Target        | Description                                                                                    |
|---------------|------------------------------------------------------------------------------------------------|
| `iso`         | Build from a certified minimal ISO. The primary target for regulated environments. The toolchain uses Kickstart/Anaconda for partitioning and package installation (preserving the ISO's certification chain), then orchestrates certified tools for all subsequent configuration. |
| `chroot`      | Build into a chroot directory using `dnf --installroot`. No ISO required. Suitable for development and testing. |
| `image`       | Build an OCI container image via bootc Containerfile. For container-native deployments. |
| `bare`        | Emit the full toolchain for execution on a bare disk. For Linux From Scratch-style builds, custom distributions, embedded appliances, or heavily customized environments where the operator controls every layer. |
| `delta`       | Emit a delta toolchain from an old manifest to the current declaration. For runtime maintenance of existing systems. |

The `iso` target is the default and the design center. It exists because Ironclad's primary value proposition for regulated environments is: **start from a certified ISO, end with a fully customized system, and the certification chain is never broken** — because every tool the toolchain calls is the same tool the certification body audited.

For topologies, the compiler emits a toolchain per system, plus any topology-level artifacts (deployment ordering, cross-system configuration distribution, shared secrets).

---

## SELinux Policy Generation

SELinux is the subsystem where the compiler's built-in domain knowledge runs deepest. Correct policy generation requires a global view of the entire declared system — every process, file, user, network interface, and the relationships between them. The compiler already possesses this view after the class resolution and semantic validation passes, making it the natural and only correct place to generate policy. No single standard library class has access to the complete topology required for sound policy generation.

Initial development targets **targeted policy**, the enforcement mode used by the vast majority of production RHEL-family systems. During backend emission, the compiler generates type enforcement rules and file context definitions using the Reference Policy as a foundation. Custom policy modules are emitted for declared services and file contexts that fall outside the distribution's base policy coverage. Strictness is configurable: a single compiler flag shifts the generated policy from a development-friendly permissive baseline to a restrictive production posture.

Generated policy is fully inspectable and overridable. Engineers can review the emitted `.te`, `.fc`, and `.if` files, modify them, or override specific rules in the Ironclad source. Manual overrides are preserved across recompilation; the compiler flags conflicts when a declaration change invalidates an existing override. Engineers who prefer to author policy entirely by hand can declare their policy files through file primitives — the compiler will incorporate them into the build and the agent will monitor them for drift. The compiler-generated policy is an accelerator, not a requirement.

**MLS policy generation** is a long-term compiler goal. Multi-Level Security introduces sensitivity levels, categories, dominance relationships, and cross-level information flow constraints that require formal verification against the declared system model. This is a substantially harder problem than targeted policy and requires extensive real-world validation before it can be considered production-grade. The targeted policy backend establishes the architectural foundations — topology analysis, policy module emission, override handling, conflict detection — that MLS generation will extend. In the interim, organizations requiring MLS author policy manually and declare it through file primitives.

---

## Runtime Agent

The runtime agent is a statically-linked Rust binary installed on every Ironclad-built system by the toolchain. It reads the signed manifest, verifies its signature, and periodically compares declared state against live system state. The checked property set includes file content hashes, permissions, ownership, and SELinux labels on all declared paths; user and group declarations; and any other filesystem state recorded in the manifest.

Drift is reported as structured JSON to configurable sinks (local file, syslog, remote endpoint). The agent performs detection and reporting only — no remediation. Remediation is the responsibility of the maintenance pipeline: the compiler emits a delta toolchain from the old manifest to the new declaration, the delta scripts are applied, and the agent verifies convergence. The verification step is what closes an atomic transition; until the agent confirms convergence, the transition is considered in progress.

---

## Security Floor

Ironclad enforces a non-negotiable security floor: SELinux in enforcing mode, LUKS2 full-disk encryption, and an immutable root filesystem where the platform supports it. A declaration that falls below the security floor is a compile-time error. The floor is not configurable by end users. Declarations may exceed it; they may not fall below it.

---

## Build Model

Ironclad's build model is orchestration-based. The compiler selects the right certified tool for each operation and generates validated configuration for it. Bash fills the gaps where no specialized tool exists. The same model works whether the target is a 10,000-node datacenter fleet, a single developer workstation, or a Raspberry Pi — because every Linux system has the same building blocks (filesystems, users, services, packages) and the same tools to configure them. This design has several consequences:

**ISO certification is preserved.** When building from a certified minimal ISO (RHEL, AlmaLinux), the toolchain uses Kickstart/Anaconda for partitioning and package installation — the same tools the certification body audited. Subsequent configuration uses `useradd`, `semodule`, `nft`, `systemctl`, and other tools already present on the certified platform. The certification chain from ISO to running system is never broken because Ironclad generates configuration for certified tools rather than reimplementing their functionality.

**The codebase stays small.** By maximizing dependency on existing tools, Ironclad minimizes the code it must maintain, test, and audit. The compiler's job is validation and orchestration — deciding what to do and in what order. The tools do the execution.

**The build is inspectable end-to-end.** Every script and configuration file the compiler emits is readable. An auditor can trace exactly what happens: this Kickstart file drives partitioning, this `dnf` invocation installs packages, this bash script writes config files, this `semodule` call loads policy. There is no opaque build engine.

**Multiple entry points are supported.** The same declaration can target a certified ISO, a bare disk, a chroot, or an OCI image. The compiler adjusts the bootstrap phase for the target; the configuration phases are identical.

**Heavy modification is possible.** The structured syntax covers the common cases. The `raw` escape hatch covers everything else — because bash is always available as the universal fallback. A distro developer building a custom distribution from bare disk has the same language as an enterprise engineer customizing a certified RHEL ISO. The toolchain scales from simple to arbitrarily complex.

**Updates are delta toolchains.** When the declaration changes, the compiler emits a delta toolchain — the minimum set of operations to move from the old manifest to the new state. The runtime agent verifies convergence.
