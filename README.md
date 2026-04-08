# Ironclad

**A declarative language and compiler for building, auditing, and maintaining secure, reproducible, atomic Linux systems — from a single machine to an entire datacenter — described in code.**

---

## Overview

Ironclad is a domain-specific language paired with a Rust-based compiler and a runtime agent. The language describes Linux systems at the level of their filesystem — files, directories, permissions, ownership, SELinux labels, and the relationships between them — with enough structure and type safety to validate those descriptions at compile time and detect drift from them at runtime.

The compiler itself is deliberately minimal. It understands the filesystem, file metadata, class inheritance, SELinux policy, and how to emit artifacts to a small set of backends — primarily a bootc Containerfile for image construction, a Kickstart configuration for disk layout and installation, and SELinux targeted policy from the declared system topology. Beyond SELinux, the compiler does not have built-in knowledge of systemd, nftables, Kubernetes, or any other specific Linux subsystem. That domain knowledge lives in the **standard class library**: Ironclad classes that know which files to write, where to place them, and what their contents should be in order to configure a given subsystem. A systemd service is not a compiler primitive; it is a class that writes unit files to `/etc/systemd/system/` with the correct structure. An nftables firewall is not a language keyword; it is a class that writes a ruleset to `/etc/nftables.conf` and enables the corresponding service.

SELinux is the exception to the standard-library-only rule because it is not merely another subsystem configured by files. SELinux labels are intrinsic metadata on every file the compiler manages, and correct policy generation requires a global view of the entire declared system topology — services, files, users, network interfaces, and their relationships — that no single class can assemble from local information alone. The compiler already possesses this global view during the semantic validation pass, making it the natural place to generate policy. Targeted policy generation is a compiler backend. MLS policy generation is a long-term compiler goal. Engineers who prefer to author SELinux policy entirely by hand can always do so, declaring their policy files through the language's file primitives with the compiler incorporating them into the build and the agent monitoring them for drift.

For everything other than SELinux, the separation between compiler and standard library means the language never needs to grow new syntax to support a new subsystem. If it can be configured by writing files to a Linux filesystem — and virtually everything in Linux can — it can be declared in Ironclad.

Ironclad systems are atomic and preferably immutable. Every state transition is applied as a complete, indivisible operation. Where the underlying platform supports true immutability — read-only root filesystems, image-based updates, sealed boot chains — Ironclad enforces it by default. Where full immutability is impractical, the mutable surface is minimized to explicitly declared writable paths, and every undeclared mutation is drift.

### Topology and Fleet Composition

Ironclad's class system is not limited to describing a single machine. A system declaration is a first-class object that can be assigned to a variable, parameterized, and composed with other system declarations. This means a single Ironclad source tree can describe an entire datacenter — hundreds or thousands of machines — by defining base system classes, deriving role-specific variants, and composing them into a topology declaration that maps systems to physical or virtual hosts.

A Kubernetes cluster is not a special compiler feature. It is a topology: a set of system declarations (control plane nodes, workers, etcd members) composed together with the network relationships between them. A hyperconverged infrastructure stack is a topology. A development environment with a dozen VMs is a topology. The class system — single inheritance, variable assignment, parameterization — is what makes this practical at scale. A fleet of a thousand identical edge nodes is one base class and a loop. A datacenter with fifty distinct roles is fifty derived classes composed into one topology file that inherits from all of them.

This is why the object-oriented model matters. The alternative — flat configuration files per host — does not scale past a handful of machines without external templating tools that reintroduce the fragmentation Ironclad exists to eliminate.

---

## The Problem With Existing Tools

The Linux ecosystem already contains capable tools for individual parts of the system lifecycle. bootc manages image lifecycle. Kickstart handles installer-time disk configuration. osbuild constructs images. Ansible mutates running systems. None of them share a language, a validation model, or a source of truth. A real production system's definition is scattered across Containerfiles, Kickstart scripts, osbuild blueprints, hand-authored SELinux policy, Ansible playbooks, and whatever ad hoc scripts handle the parts that none of those tools cover — with no single place to read what the system is supposed to be, no compile-time guarantee that the pieces are consistent, and no runtime mechanism to detect when reality has diverged from intent.

**bootc** manages image updates and rollbacks but does not build images. System definitions live in Containerfiles, which are inherently imperative — `RUN` commands with no semantic validation, no type checking, and no guarantee that a silent failure does not produce a subtly broken image. Disk layout, encryption, and TPM binding are entirely outside bootc's scope.

**osbuild / Image Builder** constructs images from a blueprint format covering packages, users, and enabled services. Complex declarations — arbitrary file contents, fine-grained permissions, custom supervision trees, detailed firewall rules — fall outside the blueprint schema and require shell scripts, abandoning the declarative model.

**Butane / Ignition** provides YAML-based first-boot configuration for CoreOS and RHCOS. It is not a language — no variables, no loops, no conditionals, no inheritance. Parameterizing for multiple environments requires external templating.

**Kickstart** is a one-shot installer format with declarative syntax for the easy parts and raw bash for everything else. No reuse, no validation, no type system. Once installation completes, the system is orphaned from its definition.

**Ansible / Salt / Puppet** are configuration management tools that operate on running systems but have no concept of the image that produced them, the disk layout underneath them, or a compiled manifest of intended state that can be cryptographically verified.

None of these tools can together describe a full system — let alone a fleet of systems — in a single unified source of truth with a real language, semantic validation, and lifecycle-spanning drift detection. That is the gap Ironclad fills.

---

## How It Works

### The Language

Ironclad source files describe systems at the filesystem level. The language's core primitives are files, directories, symlinks, permissions, ownership, SELinux labels, and mutability flags. Higher-level concepts — services, firewall rules, user accounts, kernel parameters, virtual machines, containers, Kubernetes clusters — are not built into the compiler. They are implemented as standard library classes that compose these filesystem primitives into the correct file structures for a given subsystem.

This design has a critical consequence: Ironclad does not need to anticipate every subsystem an engineer might want to configure. If a subsystem is configured by files on a Linux filesystem, an Ironclad class can describe it. The standard library provides classes for the most common subsystems. Engineers can write their own classes for anything the standard library does not cover, using the same primitives and the same inheritance model.

The language provides variables, loops, conditionals, and a single-inheritance class system. Classes encapsulate reusable configurations — a hardened server base, a container host profile, a Kubernetes worker node — which derived classes extend or override. The class hierarchy is flattened by the compiler during resolution; the resulting AST contains no unresolved inheritance, and every property has an explicit, traceable value.

### Standard Library

The standard class library is where domain knowledge lives. It ships with Ironclad and provides vetted, composable classes for common configurations:

**System base classes** define foundational system profiles — a hardened RHEL server, a minimal container host, a desktop workstation — by declaring the packages, files, and configurations that characterize each role. These classes expose variables for site-specific parameterization: hostnames, network addresses, storage layouts, and credentials are declared once and inherited throughout the system definition.

**Subsystem classes** encapsulate the file structures of specific Linux subsystems. A systemd class knows how to write unit files. An nftables class knows how to write rulesets. A Kubernetes class knows how to write kubeadm configurations and CNI manifests. These classes accept parameters — a service name, a set of firewall rules, a list of allowed ports — and emit the correct files to the correct paths with the correct permissions and labels.

**Topology classes** compose multiple system declarations into fleet-level descriptions. A Kubernetes cluster class takes control plane, worker, and etcd system declarations as parameters and produces a topology with the correct network relationships, certificate distribution, and bootstrap ordering.

The standard library is written in Ironclad. Every class is inspectable, overridable, and forkable. Engineers who disagree with a standard library class's decisions can extend it, override specific properties, or replace it entirely.

### The Compiler

The compiler processes source files through several stages:

**Parsing** — The parser reads Ironclad source and produces an abstract syntax tree. The parser is implemented in Rust using a PEG grammar (pest). Invalid input is rejected with structured diagnostics.

**Class resolution** — The compiler traverses the class hierarchy, resolves inheritance, and flattens derived classes into fully specified AST nodes. After this pass, every property has an explicit value with a traceable origin.

**Semantic validation** — The compiler applies validation rules against the resolved AST. Because the compiler understands the filesystem, it can catch structural errors: conflicting declarations for the same path, a file declared on an undeclared mount point, a mutable file on a read-only filesystem without an explicit writable overlay, and security floor violations (SELinux enforcing mode required, LUKS2 required, immutable root required). The compiler does not validate the *contents* of subsystem-specific files — it does not parse systemd unit syntax or nftables grammar — but it validates the structural relationships between declared filesystem objects.

**Manifest generation** — The compiler serializes the resolved AST into a signed intermediate manifest (CBOR with Ed25519 signature). This manifest is the canonical representation of the declared system state and the ground truth for runtime auditing.

**Backend emission** — The compiler emits artifacts for the backends it natively supports: a bootc Containerfile with a read-only root filesystem by default, a Kickstart configuration covering disk layout, LUKS2, LVM, TPM2/Clevis binding, and bootloader installation, SELinux targeted policy generated from the declared system topology, and the signed manifest embedded in the image. The Containerfile is how the declared filesystem becomes a real system — every declared file, directory, permission, label, and symlink is realized as image content through the Containerfile's build instructions.

The compiler's backend list is deliberately short. bootc, Kickstart, and SELinux targeted policy are sufficient to go from Ironclad source to a bootable, installed, immutable, policy-enforced system. Everything else — nftables rules, systemd units, Kubernetes manifests, VM definitions — is emitted by standard library classes writing files to the declared filesystem, not by compiler backends. The compiler does not need to understand those formats; it just needs to place the files correctly in the image.

### Runtime Agent

The runtime agent is a Rust binary embedded in every Ironclad-built image. It reads the signed manifest, verifies its signature, and periodically compares declared state against live system state. Drift — a modified file, a changed permission, an altered SELinux label, an added user, any deviation from the manifest — is reported as structured JSON to configurable sinks. The agent performs detection and reporting only; it does not remediate.

Runtime maintenance is handled by diffing two Ironclad declarations at the AST level and emitting an Ansible playbook representing the delta. After the playbook is applied, the agent verifies convergence to the new declared state. This verification closes the atomic transition.

---

## SELinux Policy Generation

SELinux is the one subsystem where the compiler has built-in domain knowledge, because correct policy generation requires a global view of the entire declared system that no single class can assemble from local context. The compiler already possesses this view — it knows every declared file, service, user, network interface, and their SELinux labels — making it the natural place to generate policy.

Initial development targets SELinux **targeted policy**, the enforcement mode used by the vast majority of production RHEL-family systems. During backend emission, the compiler analyzes the fully resolved AST and generates correct `.te`, `.fc`, and `.if` policy modules using the Reference Policy as a foundation. Custom modules are emitted for declared services and file contexts that fall outside the distribution's base policy coverage. Strictness is configurable: a single compiler flag shifts the generated policy from a development-friendly permissive baseline to a restrictive production posture.

The generated policy is fully inspectable and overridable. Engineers can review the emitted policy files, modify them, or override specific rules in the Ironclad source. Engineers who prefer to author policy entirely by hand can do so — declare the policy files through file primitives, and the compiler will incorporate them into the build and the agent will monitor them for drift. The compiler-generated policy is an accelerator, not a requirement.

**MLS policy generation** is a long-term compiler goal. Generating correct Multi-Level Security policy introduces sensitivity levels, categories, dominance relationships, and cross-level information flow constraints that require formal verification against the declared system model. This is a substantially harder problem than targeted policy and requires extensive real-world validation. The targeted policy backend establishes the architectural foundations — topology analysis, policy module emission, override handling — that MLS generation will extend. In the interim, organizations requiring MLS author policy manually and declare it through file primitives.

---

## Features

- **Atomic state transitions.** Every change is applied as an indivisible operation. The system is in one verified state or the next, never in between.
- **Immutable by default.** Root filesystems are read-only where the platform supports it. Mutable paths are explicitly declared. Undeclared mutations are drift.
- **Thin compiler, rich standard library.** The compiler understands the filesystem and SELinux policy. All other domain knowledge — systemd, nftables, Kubernetes, containers, VMs — lives in standard library classes that write the right files to the right paths.
- **Filesystem-level declaration.** Any file, directory, permission, label, or symlink can be declared, templated, and drift-detected. If Linux configures it with a file, Ironclad can declare it.
- **Datacenter-scale topology.** System declarations are first-class objects. Compose them into topologies describing entire fleets, clusters, or datacenters from a single source tree.
- **Object-oriented class system with inheritance.** Base classes define roles. Derived classes specialize them. Variables parameterize them. Loops replicate them. One source tree, any number of machines.
- **Backend integration, not replacement.** The compiler emits to bootc, Kickstart, and SELinux targeted policy. Standard library classes handle everything else by writing the correct configuration files.
- **SELinux policy generation.** The compiler generates targeted policy from the declared system topology. MLS generation is a long-term compiler goal. Manual policy authoring is always supported.
- **Compile-time validation.** Conflicting paths, files on undeclared mounts, mutable files on read-only filesystems, security floor violations — caught before an image is built.
- **Runtime drift detection.** An embedded agent continuously compares live state against the signed manifest.
- **Signed intermediate manifest.** The compiled system state is serialized, signed, and baked into every image as the ground truth for auditing.
- **RHEL ecosystem first.** Initial targets are Fedora and AlmaLinux. Debian and Arch are planned.

---

## Status & Version

**Pre-alpha v0.0.1** — concept and architecture phase. This repository contains the project vision, architectural documentation, and initial scaffolding. Alpha development begins at 0.1.0 with the core parser and grammar.

---

## Target Audience

Ironclad is designed for security administrators, Linux platform engineers, and DevOps engineers in environments where system auditability, reproducibility, and compliance are non-negotiable — particularly defense, government, and regulated industry contexts where SELinux enforcement, drift detection, atomic updates, and a complete chain of custody from declaration to running system are required.

---

## Roadmap

Development is organized into phased milestones from pre-alpha through stable release. See [ROADMAP.md](ROADMAP.md) for the full plan.

---

## Contributing

Contributions are welcome at every stage. Fork the repository and submit pull requests for grammar definitions, class library designs, or architectural feedback. Issues are encouraged for design discussion and feature proposals.

---

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
