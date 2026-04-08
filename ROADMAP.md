# Roadmap — Ironclad

Development is organized into phased milestones from pre-alpha through stable release. Versions follow semantic versioning: alpha releases span 0.1.x through 0.5.x, beta begins at 0.6.x, and the first stable release is 1.0. Early phases prioritize a correct compiler and a single working end-to-end path — Ironclad source to bootable system — before broadening the standard library or introducing fleet-scale topology.

---

## Phase 1 — Core Parser and Compiler Skeleton (0.1.x)

Define and formalize the Ironclad grammar using pest (PEG). Implement the core parser: tokenization, AST construction, and syntax validation. The grammar covers the language's core primitives — files, directories, symlinks, mount points, packages, users, groups, permissions, ownership, SELinux labels, mutability flags — along with variables, loops, conditionals, and the single-inheritance class system. Implement the class resolution pass: inheritance flattening, property resolution, variable substitution, and origin tracking. Produce structured compiler output: diagnostic logs, the resolved AST, and a human-readable representation of the flattened system declaration. No backend emission at this stage. The objective is a correct, well-tested parser and resolver that accepts valid Ironclad source and rejects invalid input with precise, actionable error reporting.

## Phase 2 — Backend Emission and Proof of Concept (0.2.x)

Implement the intermediate manifest: CBOR serialization of the resolved AST, Ed25519 signing, and manifest verification. Implement the bootc Containerfile emitter: translate the declared filesystem — every file, directory, permission, label, package, and user — into a valid Containerfile targeting a Fedora or AlmaLinux minimal base. The emitted image uses a read-only root filesystem by default with declared mutable paths realized as writable overlays. Implement the Kickstart emitter: disk partitioning, LUKS2, LVM, TPM2/Clevis binding, bootloader installation, and kernel command-line parameters. Embed the signed manifest in the image. Validate the end-to-end pipeline: Ironclad source → compiler → Containerfile + Kickstart → bootable installed system with an immutable root and a verifiable manifest. Ship the initial standard library classes: `HardenedRHELBase` (minimal hardened server), a systemd service class (writes unit files from parameters), and a basic nftables class (writes a ruleset file from declared policy). Standard library classes are written in Ironclad using only the language's core primitives.

## Phase 3 — Semantic Validation and Standard Library Expansion (0.3.x)

Implement the semantic validation pass: conflicting path declarations, files on undeclared mount points, mutable files on read-only filesystems, security floor enforcement (SELinux enforcing, LUKS2, immutable root). Expand the standard library with additional subsystem classes: s6 service supervision (writes s6 service directories), sshd configuration, chrony, rsyslog, and user/group management helpers that compose cleanly with base classes. Ship `S6ContainerHost` and `SystemdServer` base classes built from the subsystem classes. Implement inheritance depth warnings and compiler ergonomics (better error messages, source location tracking through inheritance chains).

## Phase 4 — SELinux Targeted Policy Backend (0.4.x)

Implement the SELinux targeted policy backend in the compiler: analyze the fully resolved AST to generate correct `.te`, `.fc`, and `.if` policy modules using the Reference Policy as a foundation. The compiler uses its global view of the declared system topology — services, files, users, network interfaces, and their labels — to emit policy that no single class could generate from local context. Implement strictness parameterization: a single compiler flag producing policies of graduated restrictiveness. Implement manual override handling: preserve engineer-authored policy files across recompilation, flag conflicts when declarations change. Validate generated policy against real systems — this phase requires extensive integration testing. Engineers who prefer to author SELinux policy entirely by hand can declare their policy files through file primitives; the compiler backend is an accelerator, not a requirement.

## Phase 5 — Workload Classes (0.5.x)

Implement standard library classes for workload declaration. Podman container class: accepts image references, network bindings, volume mounts, resource limits, and restart policy; emits Quadlet `.container` files integrated with the declared init system. Libvirt VM class: accepts resource allocations, network attachments, firmware type, and boot configuration; emits domain XML. Cloud Hypervisor class as a lightweight VM alternative. Extend semantic validation to cover cross-declaration conflicts surfaced by workload classes: port collisions between host services and containers, volume mounts referencing undeclared paths, VM resource demands exceeding declared host capacity. Ship `ContainerHost` and `VMHost` base classes.

## Phase 6 — Runtime Agent and Drift Detection (0.6.x)

Implement the runtime agent in Rust: manifest reading and signature verification, live state comparison against all declared filesystem properties (content hashes, permissions, ownership, SELinux labels), structured drift reporting to configurable sinks (local file, syslog, remote endpoint). Embed the agent in images via the Containerfile emitter. Implement post-maintenance verification: triggered comparison after Ansible playbook application, convergence reporting. The agent performs detection and reporting only — no remediation. Extend the manifest to include workload state (expected running containers, expected VM states) for classes that declare them. Ship the agent as a statically-linked binary with minimal dependencies.

## Phase 7 — Topology, Maintenance Pipeline, and Kubernetes (0.7.x / Beta)

Implement topology declarations: systems as first-class values, variable assignment, composition of multiple system declarations into fleet-level descriptions. Implement cross-system semantic validation: network reference consistency, IP address uniqueness, aggregate resource validation. Implement per-system manifest generation within topologies and per-system backend emission (one Containerfile and Kickstart per system in the topology). Implement the AST delta engine: accept two source trees and produce a structured diff. Implement the Ansible playbook emitter: translate deltas into idempotent playbooks for atomic runtime maintenance. Validate the full maintenance pipeline: declaration change → delta → playbook → agent verification. Ship Kubernetes standard library classes: kubeadm configuration, CNI plugin manifests, kubelet service integration, with control plane and worker node classes that compose into cluster topologies. Implement osbuild blueprint emission as an alternative image backend. Harden the compiler and agent against edge cases and adversarial input.

## Full Release (1.0)

Production-grade compiler with all validation passes and all native backends (bootc Containerfile, Kickstart, SELinux targeted policy). Runtime agent stable for production. Comprehensive standard library: system base classes, subsystem classes (systemd, s6, nftables, sshd, chrony, rsyslog, Podman, libvirt, Cloud Hypervisor, Kubernetes), and topology composition classes. Complete documentation: language reference, grammar specification, class authoring guide, standard library reference, runtime agent configuration, and topology guide. Production-grade for Fedora and AlmaLinux targets.

## Post-1.0

SELinux MLS policy compiler backend, extending the targeted policy architecture with sensitivity levels, categories, and formal information flow verification. Additional distribution targets (Debian, Arch) requiring new standard library base classes and package management adaptations. Community class repository for contributed and shared Ironclad classes. Fleet topology extensions: deployment orchestration, rolling updates across topologies, cross-system drift correlation. Cloud-init compatibility classes for cloud deployments. Terraform provider for infrastructure-as-code integration. RHEL proper support.

---

Early phases prioritize a correct compiler and a single working path from source to bootable system. The first milestone that produces a real, bootable, immutable system from Ironclad source — Phase 2 — is the project's proof of concept. Everything after it builds on that foundation.
