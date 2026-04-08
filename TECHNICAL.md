# Technical Overview — Ironclad

This document describes the architectural principles, language design, compiler pipeline, standard library model, topology system, and runtime model of Ironclad. It is intended for contributors, reviewers, and engineers evaluating the project's approach.

---

## Architectural Philosophy

Ironclad is built on a deliberate separation: the compiler is thin and the standard library is rich. The compiler understands the Linux filesystem — files, directories, permissions, ownership, SELinux labels, mutability, mount points, and the structural relationships between them — and it understands SELinux policy generation, because correct policy requires a global view of the entire declared system topology that no single class can assemble from local context. Beyond SELinux, the compiler does not understand systemd, nftables, Kubernetes, libvirt, Podman, or any other specific subsystem. That domain knowledge is the responsibility of the standard class library, which is written in Ironclad itself.

This separation exists because Linux subsystems are configured by writing files to the filesystem. A systemd service is a unit file at a known path. An nftables firewall is a ruleset file loaded by a service. A Kubernetes node is a machine with the right packages, kernel parameters, and kubeadm configuration files. Ironclad does not need built-in knowledge of these formats. It needs to place the right files at the right paths with the right metadata — and it needs a class system powerful enough to encapsulate the knowledge of what "right" means for each subsystem so that engineers do not have to rediscover it for every system they build. SELinux is the exception because its policy is not a local property of any one service or file — it is a global property of the system's topology, and the compiler is the only component with a complete view of that topology.

The compiler's job is structural correctness and SELinux policy generation: no conflicting declarations for the same path, no files on undeclared mount points, no mutable files on read-only filesystems, enforcement of the security floor, and correct targeted policy derived from the system's global topology. The standard library's job is domain correctness for everything else: the right file contents, the right file locations, the right interdependencies between subsystem configurations.

---

## Architecture Overview

Ironclad operates across four modes spanning the full system lifecycle:

**Build time** — The compiler parses Ironclad source, resolves the class hierarchy, performs structural and semantic validation, and emits backend artifacts. This is a pure static pipeline: source goes in, a bootc Containerfile, a Kickstart configuration, SELinux targeted policy, and a signed manifest come out.

**Install time** — The emitted Kickstart configuration drives Anaconda to partition disks, configure LUKS2 encryption, bind TPM2/Clevis, install the bootloader, and bootstrap the system from the bootc-managed OCI image. The signed intermediate manifest is written to the installed system.

**Runtime auditing** — The runtime agent, embedded in the image at build time, periodically compares live system state against the signed manifest. Drift is reported as structured output.

**Runtime maintenance** — When the system declaration changes, the compiler diffs the old and new ASTs and emits an Ansible playbook representing the delta. The playbook is applied atomically; the agent verifies convergence.

---

## Core Principles

### Atomicity

Every state transition on an Ironclad-managed system is atomic. The system exists in the old declared state or the new declared state; no intermediate condition is observable. For image-based updates, bootc's transactional staging provides this guarantee. For runtime maintenance deltas, the generated Ansible playbook is structured for atomic application where the backend supports it, and the runtime agent verifies convergence before reporting success.

### Immutability

Ironclad defaults to the maximum immutability the target platform supports. On bootc-managed systems, the root filesystem is read-only. Mutable state is confined to paths that the declaration explicitly marks as writable. The compiler enforces this: a file declared on a read-only filesystem without a corresponding writable overlay is a compile-time error. The runtime agent treats any modification to an immutable path as drift. Mutability is never prohibited — it is required to be explicit.

---

## Language Design

### Core Primitives

The language's type system is built around filesystem objects and their metadata. The core primitives are:

**Files** — Declared with a path, content (inline literal, template with variable interpolation, or binary hash reference), permissions, ownership, SELinux label, and mutability flag.

**Directories** — Declared with a path, permissions, ownership, SELinux label, and mutability flag. May contain nested file and directory declarations.

**Symlinks** — Declared with a source path and target path.

**Mount points** — Declared with a device, path, filesystem type, and mount options. The compiler validates that files declared beneath a mount point are consistent with the mount's properties.

**Packages** — Declared by name and optional version constraint. Packages are a build-time directive: the compiler includes them in the emitted Containerfile.

**Users and groups** — Declared with the attributes that `/etc/passwd`, `/etc/shadow`, and `/etc/group` understand. The compiler ensures these declarations are consistent.

These primitives are sufficient to describe any Linux system configuration that is realized through the filesystem. The language does not include primitives for systemd units, firewall rules, VM definitions, container specifications, or Kubernetes manifests — because all of those are files, and the file primitive already covers them. SELinux policy is the exception: the compiler generates targeted policy directly from the declared system topology rather than relying on classes to emit policy files, because correct policy requires a global view that spans the entire declaration.

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

Subsystem classes encapsulate the knowledge of how a specific Linux subsystem is configured through the filesystem. They accept parameters and emit the correct files to the correct paths. Examples:

A **systemd service class** accepts a service name, an executable path, dependency declarations, and resource limits. It emits a unit file to `/etc/systemd/system/` with the correct `[Unit]`, `[Service]`, and `[Install]` sections, a drop-in directory if overrides are declared, and an enabled symlink if the service is declared as active.

An **nftables class** accepts a structured firewall policy — interfaces, allowed ports, rate limits, default actions — and emits a ruleset file to `/etc/nftables.conf` along with a systemd service declaration (via the systemd class) to load it at boot.

A **Kubernetes node class** accepts a role (control plane or worker), cluster parameters (API server address, token, certificate authority), and network configuration (CNI plugin, pod CIDR). It emits kubeadm configuration files, ensures the required kernel parameters are set, declares the container runtime packages, and configures the kubelet service via the systemd class.

A **libvirt VM class** accepts resource allocations, network attachments, firmware type, and boot configuration. It emits a domain XML file and, if the VM should start automatically, a corresponding autostart symlink.

A **Podman container class** accepts an image reference, network bindings, volume mounts, resource limits, and restart policy. It emits a Quadlet `.container` file integrated with the init system.

### System Base Classes

System base classes compose subsystem classes into complete or near-complete system profiles:

`HardenedRHELBase` — A minimal, hardened RHEL-family server with SELinux enforcing, LUKS2, an immutable root, and a locked-down user configuration. Intended as the foundation from which all role-specific classes inherit.

`S6ContainerHost` — A container host using s6 for process supervision instead of systemd. Declares Podman, rootless container support, and an s6 service tree.

`SystemdServer` — A general-purpose server role using systemd, with common services (sshd, chrony, rsyslog) configured via subsystem classes.

`KubernetesControlPlane` / `KubernetesWorker` — Kubernetes node roles inheriting from an appropriate server base, with the Kubernetes node class parameterized for the declared cluster topology.

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

The compiler validates the resolved AST against structural rules: conflicting declarations for the same path, files on undeclared mount points, mutable files on read-only filesystems without writable overlays, security floor violations (SELinux enforcing mode, LUKS2, immutable root), and — for topologies — cross-system reference consistency. The compiler does not validate the contents of subsystem-specific files (it does not parse systemd unit syntax or nftables grammar). It validates the structural relationships between declared filesystem objects.

### Stage 4 — Manifest Generation

The compiler serializes the resolved AST into a signed intermediate manifest per system in the declaration. The manifest format is CBOR with an Ed25519 signature. For topologies, each system receives its own manifest; the topology-level relationships are encoded in a separate topology manifest that references the per-system manifests.

### Stage 5 — Backend Emission

The compiler emits artifacts for each system in the declaration:

**bootc Containerfile** — Realizes the declared filesystem as an OCI image. Every declared file, directory, permission, label, and package is expressed as Containerfile instructions. The root filesystem is configured as read-only by default; declared mutable paths are realized as writable overlays or bind mounts. The signed manifest is embedded in the image.

**Kickstart configuration** — Covers disk partitioning, LUKS2, LVM, TPM2/Clevis binding, bootloader installation, and kernel command-line parameters. The `%post` section is generated and minimal; complex configuration lives in the image.

**SELinux targeted policy** — The compiler analyzes the fully resolved AST — every declared file, service, user, network interface, and their labels — and generates correct `.te`, `.fc`, and `.if` policy modules using the Reference Policy as a foundation. See the SELinux section below.

These are the only backends the compiler natively emits. Everything else — systemd units, nftables rulesets, Kubernetes manifests, libvirt XML, Podman Quadlet files — is emitted by standard library classes as declared files. The compiler places them in the image through the Containerfile. The compiler does not need to understand their formats; it places the files the classes declare.

For topologies, the compiler emits a Containerfile, Kickstart configuration, and SELinux policy per system, plus any topology-level artifacts (deployment ordering, cross-system configuration distribution).

---

## SELinux Policy Generation

SELinux is the one subsystem where the compiler has built-in domain knowledge. Correct policy generation requires a global view of the entire declared system — every process, file, user, network interface, and the relationships between them. The compiler already possesses this view after the class resolution and semantic validation passes, making it the natural and only correct place to generate policy. No single standard library class has access to the complete topology required for sound policy generation.

Initial development targets **targeted policy**, the enforcement mode used by the vast majority of production RHEL-family systems. During backend emission, the compiler generates type enforcement rules and file context definitions using the Reference Policy as a foundation. Custom policy modules are emitted for declared services and file contexts that fall outside the distribution's base policy coverage. Strictness is configurable: a single compiler flag shifts the generated policy from a development-friendly permissive baseline to a restrictive production posture.

Generated policy is fully inspectable and overridable. Engineers can review the emitted `.te`, `.fc`, and `.if` files, modify them, or override specific rules in the Ironclad source. Manual overrides are preserved across recompilation; the compiler flags conflicts when a declaration change invalidates an existing override. Engineers who prefer to author policy entirely by hand can declare their policy files through file primitives — the compiler will incorporate them into the build and the agent will monitor them for drift. The compiler-generated policy is an accelerator, not a requirement.

**MLS policy generation** is a long-term compiler goal. Multi-Level Security introduces sensitivity levels, categories, dominance relationships, and cross-level information flow constraints that require formal verification against the declared system model. This is a substantially harder problem than targeted policy and requires extensive real-world validation before it can be considered production-grade. The targeted policy backend establishes the architectural foundations — topology analysis, policy module emission, override handling, conflict detection — that MLS generation will extend. In the interim, organizations requiring MLS author policy manually and declare it through file primitives.

---

## Runtime Agent

The runtime agent is a statically-linked Rust binary embedded in every Ironclad-built image. It reads the signed manifest, verifies its signature, and periodically compares declared state against live system state. The checked property set includes file content hashes, permissions, ownership, and SELinux labels on all declared paths; user and group declarations; and any other filesystem state recorded in the manifest.

Drift is reported as structured JSON to configurable sinks (local file, syslog, remote endpoint). The agent performs detection and reporting only — no remediation. Remediation is the responsibility of the maintenance pipeline: AST delta → Ansible playbook → agent verification of convergence. The verification step is what closes an atomic transition; until the agent confirms convergence, the transition is considered in progress.

---

## Security Floor

Ironclad enforces a non-negotiable security floor: SELinux in enforcing mode, LUKS2 full-disk encryption, and an immutable root filesystem where the platform supports it. A declaration that falls below the security floor is a compile-time error. The floor is not configurable by end users. Declarations may exceed it; they may not fall below it.

---

## Build and Image Model

Ironclad-built images are OCI-compliant container images managed by bootc. The image contains the complete declared system as an immutable artifact. Updates follow bootc's transactional model: the new image is staged alongside the running system and activated atomically on reboot. Failed boots trigger automatic rollback. For environments without OCI infrastructure, the compiler can target osbuild's blueprint format as an alternative backend.
