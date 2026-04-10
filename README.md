# Ironclad

**A declarative language and compiler for building, auditing, and maintaining secure, reproducible Linux systems — from datacenters to desktops — described in code.**

---

## Philosophy

Linux system configuration is a solved problem in pieces. `cryptsetup` knows how to encrypt a disk. `semodule` knows how to load SELinux policy. `dnf` knows how to install packages. `useradd` knows how to create accounts. `nft` knows how to load firewall rules. Every individual tool works.

What no tool validates is the relationships between the pieces.

A service runs as user `postgres` with SELinux type `postgresql_t`, listening on port 5432, with data on an encrypted XFS volume mounted at `/var/lib/pgsql`, and a firewall rule allowing TCP 5432 from the application subnet. Today, each of those facts is configured separately with separate tools, and the only thing that validates they all agree with each other is a human reading five different configuration files and hoping they didn't miss a contradiction.

**Ironclad is the compiler for that validation problem.** You write the declaration once. The compiler proves the pieces are consistent — at compile time, not at runtime. Then it hands the pieces to the tools that already know how to execute them.

The compiler doesn't reimplement `cryptsetup` or `dnf` or `semodule`. It generates validated configuration for them and orchestrates them in the correct order. This means Ironclad's codebase stays small, its attack surface stays minimal, and its trust is inherited from the platform — an auditor doesn't need to trust Ironclad's implementation of disk encryption, they trust `cryptsetup`, which they've already certified. Ironclad proves the inputs are consistent with everything else.

This works for anything that runs Linux. A hardened enterprise server, a developer desktop, a Kubernetes cluster, a fleet of edge nodes, a home NAS, a point-of-sale kiosk, a Raspberry Pi. The tools are the same. The validation problem is the same. The language is the same.

---

## The Problem

The Linux ecosystem contains capable tools for individual parts of the system lifecycle. But no tool validates the relationships between them, and no single source of truth describes what the system is supposed to be.

**For servers and datacenters:** A production system's definition is scattered across Kickstart scripts, Ansible playbooks, hand-authored SELinux policy, nftables rulesets, and ad hoc shell scripts — with no compile-time guarantee that the pieces are consistent and no runtime mechanism to detect when reality has diverged from intent.

**For desktops and workstations:** Hardening a Linux desktop means following a 200-step checklist — LUKS, SELinux, firewall, user lockdown, service hardening — with no way to verify it was done correctly, no way to reproduce it on another machine, and no way to detect when an update quietly undoes a hardening step.

**For embedded and appliances:** Building a custom Linux image for an appliance means hand-wiring partitioning, packages, services, and security configuration with no validation tooling, no reproducibility guarantee, and no drift detection once deployed.

**For fleets at scale:** Describing a thousand machines means a thousand configuration files, or an external templating system that reintroduces the fragmentation the tooling was supposed to eliminate.

The individual tools are fine. The gap is the layer between them — the layer that proves they're being used consistently with each other, across the full lifecycle, at any scale.

---

## How It Works

### The Language

Ironclad source files declare systems at the filesystem level. The language's core primitives are files, directories, permissions, ownership, SELinux labels, mount points, and storage topology. The compiler also has native understanding of services, firewall rules, network interfaces, users, groups, and packages — because these participate in a closed cross-validation loop where every domain references the others.

Subsystems outside this loop — bootloader configuration, secrets management, structured file editing, Kubernetes manifests, VM definitions, container specifications — are implemented as standard library classes that compose the core primitives into the correct file structures for a given subsystem. If it's configured by writing files to a Linux filesystem, an Ironclad class can describe it. The standard library provides classes for the most common subsystems. Engineers write their own for anything the stdlib doesn't cover.

The language provides variables, loops, conditionals, and a single-inheritance class system. Classes encapsulate reusable configurations — a hardened server base, a secure desktop profile, an embedded appliance template — which derived classes extend or override. The class hierarchy is flattened by the compiler during resolution; the resulting AST contains no unresolved inheritance, and every property has an explicit, traceable value.

### The Compiler

The compiler processes source files through five stages:

1. **Parsing** — Reads Ironclad source and produces an abstract syntax tree. Implemented in Rust using a PEG grammar (pest). Invalid input is rejected with structured diagnostics.

2. **Class resolution** — Traverses the class hierarchy, resolves inheritance, and flattens derived classes. After this pass, every property has an explicit value with a traceable origin.

3. **Semantic validation** — The core of Ironclad's value. The compiler validates cross-domain consistency: services reference declared users and SELinux types, firewall rules reference declared interfaces, service ports have corresponding firewall rules (and vice versa), file owners exist, SELinux labels are valid, storage topology is sound, mount points resolve, and the security floor is met. This is where the relationships between pieces are proven consistent.

4. **Manifest generation** — Serializes the resolved AST into a signed intermediate manifest (CBOR with Ed25519 signature). This manifest is the ground truth for runtime auditing.

5. **Backend emission** — Emits a build toolchain. The toolchain orchestrates certified tools for each operation — Kickstart for partitioning, `dnf` for packages, `cryptsetup` for encryption, `semodule` for SELinux, `nft` for firewall rules — and falls back to bash where no specialized tool exists. The output is a set of ordered, inspectable scripts and configuration files.

The compiler supports multiple build targets:

| Target  | Use Case |
|---------|----------|
| `iso`   | Build from a certified minimal ISO. Preserves the certification chain. Primary target for regulated environments. |
| `chroot`| Build into a chroot directory. For development and testing. |
| `image` | Build an OCI container image via bootc. For container-native deployments. |
| `bare`  | Full toolchain for a bare disk. For LFS-style builds, custom distros, and embedded appliances. |
| `delta` | Emit changes from old manifest to new declaration. For redeployment and maintenance of running systems. |

### Standard Library

The standard class library is where domain expertise is encoded. It ships with Ironclad and provides vetted, composable classes spanning the full range of Linux deployments:

**Subsystem classes** encapsulate the file structures of specific subsystems — a bootloader class knows how to write `grub.cfg`, a Kubernetes class knows how to write kubeadm manifests, a secrets class knows how to configure Vault or age or systemd-creds. Each accepts parameters and emits the correct files to the correct paths.

**System base classes** compose subsystem classes into complete profiles:
- `HardenedRHELBase` — Hardened RHEL-family foundation: SELinux enforcing, LUKS2, immutable root
- `SecureWorkstation` — Hardened desktop with display server, audio, and sandboxed applications
- `EmbeddedAppliance` — Minimal, read-only-root, single-purpose device image
- `EdgeNode` — Hardened, remotely managed node for fleet deployment at scale
- `KubernetesControlPlane` / `KubernetesWorker` — Cluster node roles with topology-aware configuration

Every class is written in Ironclad, inspectable, overridable, and forkable.

### Runtime Agent

The runtime agent is a statically-linked Rust binary installed on every Ironclad-built system. It reads the signed manifest, verifies its signature, and periodically compares declared state against live state. Drift — a modified file, a changed permission, an altered SELinux label, an added user — is reported as structured JSON.

The agent covers the full lifecycle beyond initial build. When the declaration changes, the compiler emits a delta toolchain — the minimum operations to move from the old state to the new. After the delta is applied, the agent verifies convergence. This is how Ironclad handles redeployment and maintenance without requiring a full rebuild.

### Topology and Fleet Composition

System declarations are first-class values. They can be parameterized and composed into topologies describing interconnected infrastructure — a Kubernetes cluster, a datacenter, a fleet of edge nodes. Cross-system references (one system's firewall rules referencing another system's IP address) are validated at compile time. A thousand identical nodes is one base class and a loop. A datacenter with fifty roles is fifty derived classes in one topology file.

---

## Target Audiences

**Security and compliance engineers** — Compile-time cross-validation, signed manifests, certified tool chains, inspectable builds. From classified datacenters to compliance-sensitive consumer devices.

**Low-level system designers and distro developers** — Full control over every layer. Build anything from a custom distribution to an embedded appliance. Use Ironclad as the backend for defining and pushing your own distro.

**Engineers who want secure systems without deep specialization** — Apply a class, set your variables, build. The stdlib encapsulates the expertise. Whether the target is a desktop, a home server, or a Raspberry Pi.

**AI agents** — Structured, typed, compile-time-validated syntax is ideal for AI-generated configuration. The AI writes code, the compiler catches mistakes before anything touches hardware, the human reviews structured declarations. Secure system configuration becomes accessible to anyone who can describe what they want.

---

## SELinux Policy Generation

SELinux is the subsystem where the compiler's domain knowledge runs deepest, because correct policy requires a global view of the entire declared system — every process, file, user, network interface, and their labels. The compiler generates targeted policy from the resolved AST using the Reference Policy as a foundation.

Generated policy is fully inspectable and overridable. Engineers who prefer to author policy by hand can declare policy files through file primitives — the compiler incorporates them into the build and the agent monitors them for drift.

MLS policy generation is a long-term compiler goal. In the interim, organizations requiring MLS author policy manually.

---

## Features

- **Cross-system validation.** The compiler proves that storage, filesystems, users, services, firewall rules, SELinux labels, and network interfaces are consistent with each other — at compile time.
- **Orchestrator of certified tools.** The compiler generates validated configuration for existing tools (`cryptsetup`, `dnf`, `semodule`, `nft`, etc.) rather than reimplementing them. Trust is inherited from the platform.
- **Any Linux system.** Servers, desktops, embedded devices, appliances, fleets. If it runs Linux, Ironclad can describe it.
- **Full lifecycle.** Build, runtime audit, and redeployment from a single declaration. The manifest is the source of truth at every point.
- **Atomic state transitions.** Every change is applied as an indivisible operation. The system is in one verified state or the next.
- **Immutable by default.** Read-only root where the platform supports it. Mutable paths are explicitly declared. Undeclared mutations are drift.
- **Object-oriented class system.** Base classes define roles. Derived classes specialize them. Variables parameterize them. Loops replicate them. One source tree, any number of machines.
- **SELinux policy generation.** The compiler generates targeted policy from the declared topology. Manual authoring is always supported.
- **Signed manifests and drift detection.** An embedded agent continuously compares live state against the signed manifest.
- **Inspectable builds.** Every script and configuration file the compiler emits is readable. No opaque build engine.
- **Datacenter-scale topology.** Compose system declarations into fleets, clusters, and datacenters from a single source tree.

---

## Status & Version

**Pre-alpha v0.0.1** — concept and architecture phase. This repository contains the project vision, architectural documentation, and initial scaffolding. Alpha development begins at 0.1.0 with the core parser and grammar.

---

## Roadmap

Development is organized into phased milestones from pre-alpha through stable release. See [ROADMAP.md](ROADMAP.md) for the full plan.

---

## Contributing

Contributions are welcome at every stage. Fork the repository and submit pull requests for grammar definitions, class library designs, or architectural feedback. Issues are encouraged for design discussion and feature proposals.

---

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
