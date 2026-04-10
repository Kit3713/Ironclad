# Security Policy

## Scope

Ironclad is a compiler that generates security-critical system configurations (disk encryption, SELinux policy, firewall rules, user accounts). Bugs in the compiler can produce insecure systems. We treat compiler correctness as a security concern.

Reportable issues include:

- The compiler emits configurations that are weaker than what the source declares (e.g. a LUKS block compiles without encryption, a firewall rule is silently dropped, an SELinux label is omitted).
- The compiler accepts invalid input that should be rejected by validation (e.g. conflicting mount targets, missing required properties that lead to insecure defaults).
- Manifest signing or verification can be bypassed.
- The runtime agent (when implemented) fails to detect drift or reports false negatives.
- Dependency vulnerabilities in the compiler toolchain.

Out of scope:

- Issues in upstream tools Ironclad orchestrates (Anaconda, dnf, cryptsetup, nft, etc.) — report those to their respective projects.
- Feature requests or design disagreements — use the issue tracker.

## Supported Versions

Ironclad is pre-alpha. There are no stable releases yet. Security reports apply to the `main` branch.

| Version     | Supported |
|-------------|-----------|
| main branch | Yes       |
| < 1.0       | Best effort once released |

## Reporting a Vulnerability

**Do not open a public issue for security vulnerabilities.**

Email: **[security contact to be configured]**

Alternatively, use [GitHub private vulnerability reporting](https://github.com/Kit3713/Ironclad/security/advisories/new) if enabled on this repository.

Include:

1. Description of the vulnerability.
2. Minimal `.icl` input that demonstrates the issue (if applicable).
3. Expected vs. actual compiler output.
4. Impact assessment — what configuration is weakened and how.

You should receive an acknowledgment within 72 hours. We aim to provide a fix or mitigation plan within 14 days for confirmed issues.

## Disclosure

We follow coordinated disclosure. We will credit reporters in the fix commit and any advisory unless anonymity is requested.
