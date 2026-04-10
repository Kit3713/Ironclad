# Phase 2 — Toolchain Emission and Proof of Concept (0.2.x)

Phase 2 delivers the first end-to-end build pipeline: Ironclad source goes in, a bootable installed system comes out. This phase adds the intermediate manifest, the toolchain emitter, and the initial standard class library. The toolchain orchestrates certified tools — Kickstart for partitioning and package installation, `cryptsetup` for LUKS, `useradd` for accounts, `nft` for firewall, `semodule` for SELinux policy, and bash for everything else — to build a system from a certified minimal ISO.

The milestone is complete when a single Ironclad source file compiles to a build toolchain that produces a bootable, installed Fedora or AlmaLinux system with LUKS2 encryption, SELinux enforcing, and a signed manifest on disk — starting from a certified minimal ISO with the certification chain preserved.

**Prerequisite:** Phase 1 complete (0.1.x). The parser, class resolver, CLI, and diagnostic infrastructure are working and tested.

---

## 2.1 — Intermediate Manifest: Data Model and Serialization

**Version: 0.2.0-dev**

Define the manifest format — the backend-agnostic serialization of a resolved system that every emitter reads from and the runtime agent will eventually verify against.

**Deliverables:**

- Define the `Manifest` struct in `ironclad-ast` (or a new `ironclad-manifest` crate): a serializable representation of the `ResolvedSystem` stripped of source spans and compiler internals, containing only the declared system state.
- Choose and document the canonical field ordering and naming conventions for the manifest schema.
- Implement CBOR serialization and deserialization using `serde` and `ciborium`.
- Write a function `resolved_to_manifest(resolved: &ResolvedSystem) -> Manifest`.
- Write round-trip tests: resolve a system, serialize to CBOR, deserialize, assert equality.
- Write a `manifest_version` field into the schema (integer, starting at 1) for forward compatibility.

**Done when:** A `ResolvedSystem` serializes to CBOR and deserializes back identically.

---

## 2.2 — Intermediate Manifest: Ed25519 Signing and Verification

**Version: 0.2.0-dev**

Sign the manifest so the runtime agent can later verify it hasn't been tampered with.

**Deliverables:**

- Add `ed25519-dalek` and `rand` dependencies.
- Implement `sign_manifest(manifest_cbor: &[u8]) -> SignedManifest` which generates an Ed25519 keypair, signs the CBOR bytes, and produces a `SignedManifest` containing: the CBOR payload, the signature, and the public key.
- Implement `verify_manifest(signed: &SignedManifest) -> Result<Manifest, VerificationError>` which checks the signature against the embedded public key and deserializes on success.
- Define the on-disk format: a single file (`ironclad-manifest.signed`) containing a CBOR envelope with fields `version`, `public_key`, `signature`, `payload`.
- Write tests: sign and verify a manifest, tamper with one byte of the payload and verify rejection, tamper with the signature and verify rejection.

**Done when:** Manifests are signed, verifiable, and tamper-evident.

---

## 2.3 — Emitter Trait and Compiler Pipeline Integration

**Version: 0.2.0-dev**

Define the emitter abstraction and wire it into the compiler pipeline so adding new build targets is mechanical.

**Deliverables:**

- Create an `ironclad-emit` crate.
- Define a trait:
  ```rust
  pub trait Emitter {
      type Output;
      fn emit(&self, manifest: &Manifest) -> Result<Self::Output, DiagnosticBag>;
  }
  ```
- Define a `BuildTarget` enum: `Iso`, `Chroot`, `Image`, `Bare`, `Delta`.
- Define a `ToolchainPlan` struct that holds a `Manifest`, the selected `BuildTarget`, and the ordered list of toolchain phases to emit.
- Wire the CLI: `ironclad compile <file.ic>` now accepts a `--target` flag (e.g. `--target iso`) and runs the toolchain emitter after resolution.
- Add an `--output-dir` flag specifying where emitted artifacts are written (default: `./build/`).
- With no emitter phases implemented yet, the pipeline writes only the signed manifest to the output directory.
- Write integration tests confirming the manifest is written to the output directory.

**Done when:** `ironclad compile example.ic --output-dir build/` produces `build/ironclad-manifest.signed`.

---

## 2.4 — Toolchain: Storage Phase

**Version: 0.2.0-dev**

Emit the first toolchain phase: storage setup. This is where the toolchain orchestrates certified tools for disk partitioning, encryption, and volume management.

**Deliverables:**

- Implement the storage phase emitter. For the `iso` target, emit a Kickstart `%pre` section and partitioning directives: `ignoredisk`, `clearpart`, `zerombr`, `part`, `volgroup`, `logvol`.
- For the `chroot` target, emit bash scripts calling `parted`, `mkfs.*`, and `mount` directly.
- Emit LUKS2 encryption: Kickstart `--encrypted --luks-version=luks2` for `iso` target; `cryptsetup luksFormat` calls for `chroot` target.
- Emit LVM: `volgroup` and `logvol` directives for Kickstart; `pvcreate`/`vgcreate`/`lvcreate` calls for chroot.
- Write the storage phase script to `<output-dir>/phases/01-storage.sh` (chroot) or integrate into Kickstart (iso).
- Write tests: a system declaring a root LV, a home LV, and swap on a single VG produces correct output for both `iso` and `chroot` targets.

**Done when:** The storage phase generates correct disk setup for both build targets.

---

## 2.5 — Toolchain: Package Installation Phase

**Version: 0.2.0-dev**

Emit the package installation phase. This is where the toolchain leans hardest on certified tools.

**Deliverables:**

- For the `iso` target, emit a Kickstart `%packages` section from the declared package list. Include declared package groups. Handle `state = absent` packages as excludes.
- For the `chroot` target, emit a bash script calling `dnf --installroot=<chroot> install -y <packages>` with declared repository configuration.
- Emit repository setup: write `.repo` files to the appropriate location before package installation.
- Write the package phase script to `<output-dir>/phases/02-packages.sh` (chroot) or integrate into Kickstart (iso).
- Write tests: a system declaring packages and repositories produces correct output for both targets.

**Done when:** The package installation phase correctly uses Kickstart `%packages` (iso) or `dnf` (chroot).

---

## 2.6 — Toolchain: Users, Files, and Configuration Phases

**Version: 0.2.0-dev**

Emit the phases that configure the system after package installation.

**Deliverables:**

- **Users/groups phase:** Emit `groupadd`, `useradd`, `usermod`, `chpasswd`, and `chage` calls from declared users and groups. For `iso` target, emit in Kickstart `%post`. For `chroot`, emit as standalone bash script.
- **File placement phase:** Emit `mkdir`, `install` (or `cat <<'EOF'`), `chown`, `chmod`, and `chattr` calls for all declared files and directories. Handle `content`, `source` (copy from build context), and `template` (variable-substituted) content sources.
- **Service configuration phase:** Write systemd unit files to `/etc/systemd/system/` and emit `systemctl enable` calls. For s6, write service directories and run scripts.
- **Network configuration phase:** Write NetworkManager keyfiles (or systemd-networkd units, depending on declared backend) and hostname configuration.
- **Firewall phase:** Write `/etc/nftables.conf` from declared firewall rules and emit `systemctl enable nftables`.
- Write each phase as a separate script in `<output-dir>/phases/`. For `iso` target, consolidate into Kickstart `%post`.
- Write tests for each phase independently and as a combined toolchain.

**Done when:** All configuration phases generate correct scripts for both build targets.

---

## 2.7 — Toolchain: SELinux, Bootloader, Manifest, and Seal Phases

**Version: 0.2.0-dev**

Emit the final toolchain phases that complete the system.

**Deliverables:**

- **SELinux phase:** Emit `semanage` calls for custom user/role mappings. Write SELinux mode configuration to `/etc/selinux/config`. Emit `restorecon -R /` as a final relabeling step. (Full policy generation is Phase 4 — this phase handles configuration and relabeling only.)
- **Bootloader phase:** Emit `grub2-install` and `grub2-mkconfig` calls (or `bootctl install` for systemd-boot) from stdlib bootloader class output. For `iso` target, emit Kickstart `bootloader` directive.
- **Manifest phase:** Copy the signed manifest to `/etc/ironclad/manifest.signed` on the target system.
- **Seal phase:** Set immutable bits with `chattr +i` on declared immutable files. Configure read-only root if declared. Unmount filesystems in correct order.
- **Orchestrator script:** Emit a top-level `build.sh` that runs all phase scripts in order (chroot target) or assemble the final Kickstart file with all sections (iso target).
- Write tests for each phase and for the complete assembled toolchain.

**Done when:** The complete toolchain is emitted — all phases, the orchestrator, and the signed manifest.

---

## 2.8 — Standard Class Library: Core Base Classes

**Version: 0.2.0-dev**

Ship the initial set of reusable classes written in Ironclad. These are the starting point for users and the proof that the class system works at scale.

**Deliverables:**

- Create a `stdlib/` directory in the repository.
- Write `HardenedRHELBase` class: SELinux enforcing, LUKS2 required, firewall enabled, password policy, audit logging enabled, unnecessary services disabled, RHEL/Alma/Fedora compatible package set.
- Write `FedoraBase` class extending `HardenedRHELBase`: Fedora-specific package names where they differ.
- Write `AlmaLinuxBase` class extending `HardenedRHELBase`: AlmaLinux-specific packages.
- Write `SystemdServer` class: systemd as init system, standard server packages (sshd, chrony, rsyslog), journald configuration.
- Each class must compile, resolve, and produce a correct toolchain for both `iso` and `chroot` targets.
- Write tests for each stdlib class: compile and assert the resolved output matches expected values.

**Done when:** Four stdlib classes compile and emit correct toolchains.

---

## 2.9 — End-to-End Pipeline Validation

**Version: 0.2.0**

Prove the pipeline works by building a real system from Ironclad source.

**Deliverables:**

- Write a complete example system declaration (`examples/fedora-server.ic`) that extends `FedoraBase` with: custom disk layout (3 LVs + swap), LUKS2, two user accounts, a set of server packages, a static network configuration, firewall rules allowing SSH, SELinux enforcing.
- Compile to a complete toolchain (both `iso` and `chroot` targets).
- For the `iso` target: document how to boot a Fedora minimal ISO with the generated Kickstart in a VM (libvirt/QEMU) and verify the installed system.
- For the `chroot` target: document how to run the toolchain against a chroot directory and verify the result.
- Document the manual validation steps in `docs/VALIDATION.md`: partitions, encryption, users, packages, firewall, SELinux mode, manifest presence at `/etc/ironclad/manifest.signed`.
- If full automated VM testing is feasible (e.g. via `testcloud` or a CI VM), implement it. If not, document why and what manual steps are required.
- Fix any issues discovered during validation and add regression tests.

**Done when:** The example system compiles, the toolchain produces a working system, and the validation procedure is documented. Tag 0.2.0.

---

## 2.10 — Documentation and Release Polish

**Version: 0.2.1**

Document the toolchain architecture, update the class authoring guide, and make 0.2.x releasable.

**Deliverables:**

- Write `docs/TOOLCHAIN.md`: architecture of the toolchain emitter, the phase model, how the compiler selects tools for each operation, how to add new build targets.
- Write `docs/STDLIB.md`: catalog of standard library classes with descriptions, inheritance relationships, and overridable properties.
- Update `docs/LANGUAGE_GUIDE.md` with toolchain-related content: how `--target` works, output directory structure, manifest signing, the relationship between the declaration and the emitted scripts.
- Update `ROADMAP.md` to mark Phase 2 complete.
- Review and clean up all compiler warnings, clippy lints, and TODO comments from Phase 2 development.
- Tag 0.2.1.

**Done when:** Documentation is complete, the codebase is clean, and 0.2.1 is tagged.

---

## Dependency Graph

```
Phase 1 complete (0.1.x)
 │
 ├── 2.1 (manifest data model)
 │    └── 2.2 (manifest signing)
 │         └── 2.3 (emitter trait + pipeline)
 │              ├── 2.4 (toolchain: storage phase)
 │              │    └── 2.5 (toolchain: packages phase)
 │              │         └── 2.6 (toolchain: users/files/services/network/firewall)
 │              │              └── 2.7 (toolchain: SELinux/bootloader/manifest/seal)
 │              │
 │              └── (2.4-2.7 are sequential — each phase depends on the previous)
 │
 ├── 2.8 (stdlib: base classes)  ← can start once 2.7 lands
 │
 └── 2.9 (end-to-end validation) ← requires 2.7 + 2.8
      └── 2.10 (docs + release)
```

The toolchain phases (2.4–2.7) are sequential because each phase builds on the previous one's output structure. The stdlib (2.8) requires the complete toolchain to be functional for testing. End-to-end validation (2.9) is the integration point where everything must work together.
