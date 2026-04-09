# Ironclad Firewall Syntax Specification

**Status:** Draft — syntax development, Phase 1  
**Scope:** nftables-based network policy — tables, chains, rules, sets, maps, NAT, rate limiting, connection tracking, logging, and cross-validation against service, user, and topology declarations

---

## Design Principles

Ironclad's firewall syntax is a structured declaration layer over nftables. The syntax maps directly to nftables concepts — tables, chains, rules, sets, maps — without inventing an abstraction layer that hides the underlying model. Engineers who know nftables can read and write Ironclad firewall declarations without learning a new mental model. Engineers who don't know nftables get compile-time validation, sensible defaults, and structured diagnostics that raw nftables ruleset files do not provide.

The key design constraints:

1. **Default deny.** Input and forward chains default to `drop`. Output defaults to `accept`. This matches the security posture of every other Ironclad subsystem — nothing is permitted unless explicitly declared. The operator can override any default policy.

2. **Structured rules with a raw escape hatch.** Most firewall rules — allow TCP 443, rate-limit SSH, NAT a subnet — are expressible in structured syntax with typed fields that the compiler validates. Rules that exceed the structured model's expressiveness use a `raw` block containing literal nftables rule syntax, validated only for basic grammar.

3. **Cross-validation is the point.** A service that declares `listen_stream = [443]` in INIT_SYNTAX but has no firewall rule allowing TCP 443 produces a warning. A firewall rule allowing traffic to port 5432 when no service binds 5432 produces a warning. This bidirectional cross-reference is the primary value of having firewall declarations in the same source tree as everything else.

4. **Named sets are first-class.** IP allowlists, port groups, and interface sets are declared as named blocks and referenced by rules. This matches nftables' native set/map model and enables reuse across chains and cross-system topology references.

5. **Stateful by default.** Connection tracking is enabled. Established and related connections are accepted before rule evaluation. This is how every production firewall works; making it explicit in every declaration would be noise.

6. **One table is usually enough.** Most systems need a single `inet filter` table and optionally an `inet nat` table. The syntax supports arbitrary table/chain topologies for complex setups but defaults to the common case.

---

## System-Level Firewall Block

The `firewall` block is declared at the system level inside a `system` declaration or inside a class.

```
system web01 {
    firewall {
        # tables, chains, rules, sets
    }
}
```

A system may have at most one `firewall` block. When classes contribute firewall declarations, they are merged into the single `firewall` block using standard merge rules.

---

## Tables

A table is a namespace for chains, rules, and sets. Tables have a family that determines which packets they process.

```
firewall {
    table inet filter {
        # chains and rules for IPv4 + IPv6
    }

    table inet nat {
        # NAT chains
    }
}
```

### Syntax

```
table <family> <name> {
    # chain, set, map declarations
}
```

### Families

| Family   | Description                                           |
|----------|-------------------------------------------------------|
| `inet`   | IPv4 and IPv6 combined. The default and recommended family for most rulesets. |
| `ip`     | IPv4 only.                                            |
| `ip6`    | IPv6 only.                                            |
| `arp`    | ARP.                                                  |
| `bridge` | Bridge-level filtering.                               |
| `netdev` | Ingress/egress on a specific device (XDP-adjacent).   |

`inet` is strongly preferred for filter and NAT tables. The compiler emits a warning when `ip` or `ip6` is used for a table that has an `inet` equivalent — separate tables force rule duplication.

### Defaults

When no `table` block is declared inside `firewall`, the compiler generates an implicit `table inet filter` with the default chains and policies described below. The moment the operator declares any explicit `table`, the implicit table is suppressed and the operator owns the full table structure.

---

## Chains

Chains are ordered lists of rules attached to a netfilter hook. A chain has a type, a hook point, a priority, and a default policy.

```
table inet filter {
    chain input {
        type = filter
        hook = input
        priority = 0
        policy = drop

        # rules
    }

    chain forward {
        type = filter
        hook = forward
        priority = 0
        policy = drop
    }

    chain output {
        type = filter
        hook = output
        priority = 0
        policy = accept
    }
}
```

### Properties

| Property   | Type     | Default           | Description                                                                                   |
|------------|----------|--------------------|-----------------------------------------------------------------------------------------------|
| `type`     | enum     | `filter`           | Chain type. `filter`, `nat`, or `route`.                                                      |
| `hook`     | enum     | (from chain name)  | Netfilter hook. `input`, `output`, `forward`, `prerouting`, `postrouting`, `ingress`, `egress`. |
| `priority` | `int`    | `0`                | nftables priority. Lower numbers run first. Named priorities are accepted: `raw` (-300), `mangle` (-150), `dstnat` (-100), `filter` (0), `security` (50), `srcnat` (100). |
| `policy`   | enum     | see defaults below | Default verdict for packets reaching the end of the chain. `accept`, `drop`.                  |

### Default Policies

| Hook           | Default Policy |
|----------------|----------------|
| `input`        | `drop`         |
| `forward`      | `drop`         |
| `output`       | `accept`       |
| `prerouting`   | `accept`       |
| `postrouting`  | `accept`       |

### Well-Known Chain Names

When a chain is named `input`, `output`, `forward`, `prerouting`, or `postrouting`, the compiler infers `hook` from the name. Explicit `hook` overrides this inference. Custom chain names require an explicit `hook`.

```
# Hook inferred from name
chain input { policy = drop }

# Custom name — hook required
chain wan_filter {
    type = filter
    hook = input
    priority = 10
    policy = drop
}
```

### Default Chains

When a `table inet filter` is declared (or implicitly generated) with no explicit chains, the compiler generates:

```
chain input  { type = filter; hook = input;   priority = 0; policy = drop   }
chain forward { type = filter; hook = forward; priority = 0; policy = drop  }
chain output { type = filter; hook = output;  priority = 0; policy = accept }
```

Once the operator declares any chain, only declared chains exist — no implicit chains are generated for that table.

### Implicit Stateful Rules

Every filter chain with `policy = drop` or `policy = reject` automatically has the following rules prepended before any operator-declared rules:

```
# Compiler-generated — always first in chain
rule accept_established {
    match { ct_state = [established, related] }
    action = accept
}

rule drop_invalid {
    match { ct_state = [invalid] }
    action = drop
}
```

For the `input` chain specifically, the compiler also prepends:

```
rule accept_loopback {
    match { iif = "lo" }
    action = accept
}
```

These implicit rules can be suppressed with `stateful = false` on the chain, in which case the operator is responsible for all connection tracking rules:

```
chain input {
    policy = drop
    stateful = false    # no implicit ct rules — operator manages all state

    # ...
}
```

---

## Rules

Rules are named declarations inside a chain. Each rule has a match condition and an action.

### Basic Syntax

```
chain input {
    policy = drop

    rule allow_ssh {
        match { protocol = tcp; dport = 22 }
        action = accept
    }

    rule allow_http {
        match { protocol = tcp; dport = [80, 443] }
        action = accept
    }

    rule allow_icmp {
        match { protocol = icmp }
        action = accept
    }

    rule allow_icmpv6 {
        match { protocol = icmpv6 }
        action = accept
    }
}
```

Rules are evaluated in declaration order. The first matching rule's action is applied. Rules that do not match pass evaluation to the next rule.

### The `match` Block

The `match` block specifies packet matching criteria. All fields within a match are AND-combined — every field must match for the rule to apply.

| Field       | Type                    | Description                                                                           |
|-------------|-------------------------|---------------------------------------------------------------------------------------|
| `protocol`  | enum or `int`           | Protocol. `tcp`, `udp`, `icmp`, `icmpv6`, `sctp`, `dccp`, `gre`, `esp`, `ah`, or IANA number. |
| `dport`     | `int` or `list[int]` or `range` or `reference` | Destination port(s). Single port, list, range (`1024-65535`), or reference to a named set. |
| `sport`     | `int` or `list[int]` or `range` or `reference` | Source port(s).                                                                       |
| `saddr`     | `string` or `list[string]` or `reference` | Source address. CIDR notation, single IP, or reference to a named set.                |
| `daddr`     | `string` or `list[string]` or `reference` | Destination address.                                                                  |
| `iif`       | `string` or `list[string]` | Input interface name(s). Exact match.                                                 |
| `oif`       | `string` or `list[string]` | Output interface name(s). Exact match.                                                |
| `iifname`   | `string` or `list[string]` | Input interface name(s). Supports wildcard (`eth*`).                                  |
| `oifname`   | `string` or `list[string]` | Output interface name(s). Supports wildcard.                                          |
| `ct_state`  | `list[enum]`            | Connection tracking state: `new`, `established`, `related`, `invalid`, `untracked`.   |
| `ct_mark`   | `int`                   | Connection tracking mark value.                                                       |
| `mark`      | `int`                   | Packet mark (nfmark).                                                                 |
| `tcp_flags` | `list[enum]`            | TCP flags to match: `syn`, `ack`, `fin`, `rst`, `psh`, `urg`.                        |
| `icmp_type` | `enum` or `list[enum]`  | ICMP type: `echo-request`, `echo-reply`, `destination-unreachable`, `time-exceeded`, `parameter-problem`, etc. |
| `icmpv6_type` | `enum` or `list[enum]` | ICMPv6 type: `echo-request`, `echo-reply`, `nd-neighbor-solicit`, `nd-neighbor-advert`, `nd-router-solicit`, `nd-router-advert`, etc. |
| `limit`     | rate expression         | Inline rate limit. See Rate Limiting section.                                         |

### Actions

| Action       | Description                                                                    |
|--------------|--------------------------------------------------------------------------------|
| `accept`     | Allow the packet.                                                              |
| `drop`       | Silently discard the packet.                                                   |
| `reject`     | Discard and send an ICMP error. Optional: `reject_with = <type>`.              |
| `jump`       | Jump to another chain. Requires `target = <chain_name>`.                       |
| `goto`       | Go to another chain (no return). Requires `target = <chain_name>`.             |
| `log`        | Log and continue. See Logging section.                                         |
| `counter`    | Count and continue. Named counter with `counter = <name>`.                     |
| `mark`       | Set packet mark. Requires `set_mark = <value>`.                               |
| `ct_mark`    | Set connection mark. Requires `set_ct_mark = <value>`.                         |

Actions are specified as a single `action` property or as an `action` block for actions with parameters:

```
# Simple action
rule allow_ssh {
    match { protocol = tcp; dport = 22 }
    action = accept
}

# Action with parameters
rule reject_telnet {
    match { protocol = tcp; dport = 23 }
    action {
        type = reject
        reject_with = tcp-reset
    }
}

# Log then accept
rule allow_and_log_ssh {
    match { protocol = tcp; dport = 22 }
    action {
        type = accept
        log {
            prefix = "SSH_ACCEPT: "
            level = info
        }
    }
}

# Jump to custom chain
rule check_rate_limits {
    match { protocol = tcp; dport = [80, 443] }
    action {
        type = jump
        target = rate_limit_chain
    }
}
```

### Port Ranges

```
rule allow_ephemeral {
    match { protocol = tcp; dport = 1024-65535 }
    action = accept
}

rule allow_high_ports {
    match { protocol = udp; sport = 1024-65535 }
    action = accept
}
```

### Negation

Any match field can be negated with the `not` prefix:

```
rule drop_non_lan {
    match {
        not saddr = "10.0.0.0/8"
        dport = 5432
        protocol = tcp
    }
    action = drop
}
```

---

## Named Sets

Sets are named collections of values (IPs, ports, interfaces) that can be referenced by rules. They map directly to nftables named sets.

```
firewall {
    set trusted_networks {
        type = ipv4_addr
        elements = [
            "10.0.0.0/8",
            "172.16.0.0/12",
            "192.168.0.0/16",
        ]
    }

    set web_ports {
        type = inet_service
        elements = [80, 443, 8080, 8443]
    }

    set management_hosts {
        type = ipv4_addr
        elements = [
            "10.0.100.10",
            "10.0.100.11",
        ]
    }

    table inet filter {
        chain input {
            policy = drop

            rule allow_web {
                match { protocol = tcp; dport = set.web_ports }
                action = accept
            }

            rule allow_ssh_from_mgmt {
                match {
                    protocol = tcp
                    dport = 22
                    saddr = set.management_hosts
                }
                action = accept
            }
        }
    }
}
```

### Set Properties

| Property    | Type             | Default   | Description                                                              |
|-------------|------------------|-----------|--------------------------------------------------------------------------|
| `type`      | enum             | required  | Element type: `ipv4_addr`, `ipv6_addr`, `inet_service`, `ether_addr`, `iface_index`, `mark`. |
| `elements`  | `list`           | `[]`      | Static elements. Can be IPs, CIDRs, ports, or MAC addresses depending on `type`. |
| `flags`     | `list[enum]`     | `[]`      | Set flags: `interval` (for CIDR ranges), `timeout` (for auto-expiring elements), `constant` (immutable after load). |
| `timeout`   | duration         | (none)    | Default element timeout. Elements expire after this duration.            |
| `size`      | `int`            | (none)    | Maximum number of elements. Required for dynamic sets.                   |
| `comment`   | `string`         | `""`      | Description.                                                             |

### Dynamic Sets

Sets with no `elements` or with `timeout` are dynamic — elements are added at runtime (e.g., by fail2ban or manual `nft add element`). The compiler emits the empty set structure and the runtime agent monitors it.

```
set blocklist {
    type = ipv4_addr
    flags = [timeout]
    timeout = 1h
    size = 65536
    comment = "Dynamically populated blocklist"
}

table inet filter {
    chain input {
        rule drop_blocked {
            match { saddr = set.blocklist }
            action = drop
        }
    }
}
```

### Set References

Sets are referenced in match fields using `set.<name>`:

```
match { saddr = set.trusted_networks }
match { dport = set.web_ports }
```

The compiler validates that the set's `type` is compatible with the match field: `ipv4_addr` and `ipv6_addr` sets can only be referenced in `saddr`/`daddr` fields, `inet_service` sets only in `dport`/`sport` fields.

---

## Verdict Maps

Maps associate keys with verdicts or values. They map directly to nftables verdict maps.

```
firewall {
    map port_policy {
        type = inet_service : verdict
        elements = {
            22 = accept,
            80 = accept,
            443 = accept,
            23 = drop,
        }
    }

    table inet filter {
        chain input {
            rule apply_port_policy {
                match { protocol = tcp }
                action {
                    type = vmap
                    map = map.port_policy
                    field = dport
                }
            }
        }
    }
}
```

### Map Properties

| Property    | Type                  | Default   | Description                                               |
|-------------|-----------------------|-----------|-----------------------------------------------------------|
| `type`      | `<key_type> : <value_type>` | required | Key-value type pair. Value can be `verdict`, `ipv4_addr`, `inet_service`, `mark`. |
| `elements`  | `map`                 | `{}`      | Static key-value entries.                                 |
| `flags`     | `list[enum]`          | `[]`      | Same as set flags.                                        |
| `size`      | `int`                 | (none)    | Maximum entries.                                          |

---

## Rate Limiting

Rate limits restrict how many packets per interval a rule matches before the action stops applying.

### Inline Rate Limit

```
rule rate_limit_ssh {
    match {
        protocol = tcp
        dport = 22
        limit = 3/minute burst 5
    }
    action = accept
}
```

### Rate Limit Syntax

```
limit = <count>/<interval>
limit = <count>/<interval> burst <count>
```

Intervals: `second`, `minute`, `hour`, `day`.

### Per-Source Rate Limiting

For per-IP rate limiting, use a meter (nftables dynamic set with a rate limit):

```
rule per_ip_ssh_limit {
    match {
        protocol = tcp
        dport = 22
    }
    action {
        type = accept
        meter {
            name = "ssh_limit"
            key = saddr
            rate = 3/minute burst 5
            over_action = drop
        }
    }
}
```

The compiler emits an nftables meter backed by a dynamic set. When a source address exceeds the rate, subsequent packets from that address receive the `over_action` verdict.

---

## NAT

NAT rules are declared in chains with `type = nat` inside a `nat` table (or an `inet` table with nat chains).

```
table inet nat {
    chain prerouting {
        type = nat
        hook = prerouting
        priority = dstnat

        rule dnat_web {
            match { protocol = tcp; dport = 80; iif = "eth0" }
            action {
                type = dnat
                to = "10.0.1.100:8080"
            }
        }

        rule dnat_https {
            match { protocol = tcp; dport = 443; iif = "eth0" }
            action {
                type = dnat
                to = "10.0.1.100:8443"
            }
        }
    }

    chain postrouting {
        type = nat
        hook = postrouting
        priority = srcnat

        rule masquerade_outbound {
            match { oif = "eth0" }
            action = masquerade
        }

        rule snat_internal {
            match { saddr = "10.0.1.0/24"; oif = "eth0" }
            action {
                type = snat
                to = "203.0.113.1"
            }
        }
    }
}
```

### NAT Actions

| Action        | Description                                                                                |
|---------------|--------------------------------------------------------------------------------------------|
| `dnat`        | Destination NAT. Requires `to = "<addr>:<port>"` or `to = "<addr>"`.                      |
| `snat`        | Source NAT. Requires `to = "<addr>"` or `to = "<addr>-<addr>"` for a range.                |
| `masquerade`  | Dynamic source NAT using the outgoing interface's address. No `to` required.               |
| `redirect`    | Redirect to a local port. Requires `to = <port>`.                                          |

---

## Logging

Logging can be attached to any rule as part of the action block:

```
rule log_and_drop_ssh_brute {
    match {
        protocol = tcp
        dport = 22
        ct_state = [new]
        limit = 10/minute burst 20
    }
    action {
        type = drop
        log {
            prefix = "SSH_BRUTE: "
            level = warn
            group = 1
        }
    }
}
```

### Log Properties

| Property  | Type     | Default          | Description                                                  |
|-----------|----------|------------------|--------------------------------------------------------------|
| `prefix`  | `string` | `""`             | Log message prefix. Max 127 characters (nftables limit).     |
| `level`   | enum     | `warn`           | Syslog level: `emerg`, `alert`, `crit`, `err`, `warn`, `notice`, `info`, `debug`. |
| `group`   | `int`    | (none)           | NFLOG group number. When set, packets are sent to the NFLOG group instead of syslog. |

A log-only rule (log and continue without dropping or accepting) uses `action = log`:

```
rule log_all_new {
    match { ct_state = [new] }
    action {
        type = log
        log { prefix = "NEW_CONN: " }
    }
}
```

---

## Raw nftables Rules

For rules that exceed the structured syntax's expressiveness, the `raw` block injects literal nftables rule text:

```
chain input {
    policy = drop

    rule allow_ssh {
        match { protocol = tcp; dport = 22 }
        action = accept
    }

    raw {
        "tcp dport 80 tproxy to 127.0.0.1:3128 meta mark set 1"
        "ip saddr @blocklist counter drop"
        "meta l4proto ospf accept"
    }
}
```

Raw rules are inserted at the position they appear in the chain, preserving order relative to structured rules. The compiler does not validate raw rule content beyond basic string well-formedness. Raw rules bypass cross-validation — the compiler cannot verify port or address references inside raw strings.

A warning is emitted for every `raw` block — it indicates the structured syntax does not cover the operator's use case, and the project should consider extending the syntax.

---

## Interaction with Service Declarations

The compiler cross-validates firewall rules against service port declarations from `INIT_SYNTAX.MD`.

### Service → Firewall Validation

When a service declares listening ports (via `listen_stream` on a systemd socket or inline in the service), the compiler checks that the firewall contains a rule allowing traffic to those ports.

```
init systemd {
    socket httpd {
        listen_stream = [80, 443]       # compiler checks: firewall allows TCP 80, 443?
    }
}
```

If no allow rule exists for a declared service port, the compiler emits a warning:

```
WARNING: service "httpd" listens on TCP 80, 443 but no firewall rule allows inbound traffic on these ports
```

This is a warning, not an error — the service may be intentionally firewalled off (listening only on localhost, or reachable only via a tunnel).

### Firewall → Service Validation

When a firewall rule allows traffic to a port, the compiler checks that a service binds that port:

```
rule allow_mysql {
    match { protocol = tcp; dport = 3306 }
    action = accept
}
```

If no service declares port 3306, the compiler emits a warning:

```
WARNING: firewall allows TCP 3306 but no declared service listens on this port
```

This is also a warning — the port may be used by a package-managed service not declared in Ironclad, or by a runtime-started process.

### Suppressing Cross-Validation Warnings

When a port is intentionally open without a corresponding service declaration (or vice versa), the operator can suppress the warning with a comment annotation:

```
rule allow_external_mysql {
    match { protocol = tcp; dport = 3306 }
    action = accept
    # suppress: no_service_binding
}
```

The compiler recognizes `# suppress:` comments as structured annotations on the preceding or enclosing block.

---

## Interaction with Topology Declarations

In topology mode (multiple systems declared in the same source tree), firewall rules can reference other systems' addresses:

```
topology web_stack {
    system web01 {
        var ip = "10.0.1.10"

        firewall {
            table inet filter {
                chain output {
                    rule allow_db {
                        match {
                            protocol = tcp
                            dport = 5432
                            daddr = system.db01.var.ip
                        }
                        action = accept
                    }
                }
            }
        }
    }

    system db01 {
        var ip = "10.0.1.20"

        firewall {
            table inet filter {
                chain input {
                    rule allow_from_web {
                        match {
                            protocol = tcp
                            dport = 5432
                            saddr = system.web01.var.ip
                        }
                        action = accept
                    }
                }
            }
        }
    }
}
```

The compiler validates cross-system references: `system.db01.var.ip` must resolve to a declared variable on a declared system. A firewall rule referencing an undeclared system or variable is a compile error. The compiler also validates symmetry — if web01 allows outbound to db01:5432, it checks that db01 allows inbound from web01 on 5432 (warning if asymmetric, not error — asymmetric rules are valid for unidirectional flows).

---

## Firewall in Classes

Firewall rules can be declared in classes for reuse:

```
class web_server_firewall {
    firewall {
        set web_ports {
            type = inet_service
            elements = [80, 443]
        }

        table inet filter {
            chain input {
                rule allow_web {
                    match { protocol = tcp; dport = set.web_ports }
                    action = accept
                }
            }
        }
    }
}

class ssh_access {
    firewall {
        table inet filter {
            chain input {
                rule allow_ssh {
                    match { protocol = tcp; dport = 22 }
                    action = accept
                }
            }
        }
    }
}

system web01 {
    apply web_server_firewall
    apply ssh_access

    # Both classes' firewall rules are merged into the single firewall block
    # Rules in the same chain are concatenated in apply order
}
```

### Merge Rules

Firewall declarations from multiple classes merge as follows:

1. **Sets and maps:** Merged by name. If two classes declare the same set name with different elements, elements are unioned. Conflicting set properties (different `type`) are compile errors.
2. **Tables:** Merged by family and name. If two classes declare `table inet filter`, their chains are merged.
3. **Chains:** Merged by name within a table. If two classes declare `chain input` in `table inet filter`, their rules are concatenated in `apply` order. Conflicting chain properties (`policy`, `priority`) follow standard merge rules — later apply wins, inline wins over class.
4. **Rules:** Never merged — rules with the same name from different sources are a compile error. Rules are appended in declaration/apply order.

Inline firewall declarations (directly in the system block) always win over class declarations for chain-level properties. Rules from inline declarations are appended after class rules.

---

## Compiler Output

The compiler generates:

- `/etc/nftables.conf` — Complete nftables ruleset assembled from all declared tables, chains, rules, sets, and maps. Includes a generation header with the source hash.
- `/etc/sysconfig/nftables.conf` — On RHEL-family systems, configuration for the nftables service to load the ruleset at boot.
- If using systemd: a `nftables.service` dependency ensuring the firewall is loaded before network-facing services. If using s6: equivalent service ordering.

### File Properties

The generated `/etc/nftables.conf`:

```
# Compiler generates (equivalent to):
file /etc/nftables.conf {
    owner = root
    group = root
    mode = 0600
    selinux = system_conf_t
    content = generated
    immutable = true
}
```

---

## Compiler Validation Summary

### Errors (halt compilation)

- Duplicate chain name within the same table
- Duplicate set or map name within the same firewall block
- Set referenced in a match field with incompatible type (e.g., `ipv4_addr` set in `dport`)
- Set or map referenced by rule does not exist
- Cross-system firewall reference to undeclared system or variable
- Two classes declare the same set name with different `type`
- Two classes declare a rule with the same name in the same chain
- `reject_with` value is not a valid ICMP type
- NAT `to` address is not valid for the NAT type
- Chain declares `type = nat` but is inside a table with family `arp` or `bridge`
- `log.prefix` exceeds 127 characters

### Warnings

- Service port has no corresponding firewall allow rule
- Firewall allows a port with no corresponding service binding
- `raw` block used — structured syntax may need extending
- `ip` or `ip6` table used where `inet` would suffice
- Asymmetric cross-system firewall rules in topology
- Rate limit on an accept rule without a corresponding drop rule for over-limit traffic (unless using meter with `over_action`)
- Dynamic set with no `size` limit
- Rule with `dport` but no `protocol` specified (nftables requires a protocol for port matching)

---

## Security Floor Enforcement

The security floor can enforce minimum firewall requirements:

- **Floor: baseline** — A `firewall` block must be present. At least one `table inet filter` with an `input` chain must exist. Input policy must be `drop` or `reject`.
- **Floor: high** — Baseline plus: output chain must have `policy = drop` (explicit allow for outbound traffic). Forward chain must be present with `policy = drop`. All `raw` blocks promoted from warning to error.
- **Floor: maximum** — High plus: every service port must have a corresponding firewall rule (warning promoted to error). Every firewall allow rule must have a corresponding service (warning promoted to error). No `accept` rules with empty match blocks (would match all traffic).

---

## Reserved Keywords

The following keywords are reserved within firewall declarations:

`firewall`, `table`, `chain`, `rule`, `set`, `map`, `raw`, `inet`, `ip`, `ip6`, `arp`, `bridge`, `netdev`, `filter`, `nat`, `route`, `input`, `output`, `forward`, `prerouting`, `postrouting`, `ingress`, `egress`, `accept`, `drop`, `reject`, `jump`, `goto`, `log`, `counter`, `masquerade`, `dnat`, `snat`, `redirect`, `vmap`, `meter`, `policy`, `priority`, `hook`, `type`, `protocol`, `tcp`, `udp`, `icmp`, `icmpv6`, `dport`, `sport`, `saddr`, `daddr`, `iif`, `oif`, `iifname`, `oifname`, `ct_state`, `ct_mark`, `mark`, `tcp_flags`, `new`, `established`, `related`, `invalid`, `untracked`, `limit`, `burst`, `second`, `minute`, `hour`, `day`, `prefix`, `level`, `group`, `flags`, `interval`, `timeout`, `constant`, `elements`, `size`, `stateful`, `not`

---

## Grammar Summary (Informative)

```
firewall_decl       = "firewall" "{" firewall_body "}"
firewall_body       = (set_decl | map_decl | table_decl)*

table_decl          = "table" family identifier "{" table_body "}"
family              = "inet" | "ip" | "ip6" | "arp" | "bridge" | "netdev"
table_body          = (chain_decl | set_decl | map_decl)*

chain_decl          = "chain" identifier "{" chain_body "}"
chain_body          = (chain_property | rule_decl | raw_block)*
chain_property      = "type" "=" chain_type
                    | "hook" "=" hook_name
                    | "priority" "=" (int | named_priority)
                    | "policy" "=" ("accept" | "drop")
                    | "stateful" "=" bool

rule_decl           = "rule" identifier "{" rule_body "}"
rule_body           = match_block action_expr
match_block         = "match" "{" match_field* "}"
match_field         = ("not")? field_name "=" field_value

action_expr         = "action" "=" simple_action
                    | "action" "{" action_body "}"
simple_action       = "accept" | "drop" | "masquerade" | "log"
action_body         = "type" "=" action_type
                    (action_params)*
action_params       = "reject_with" "=" string
                    | "to" "=" string
                    | "target" "=" identifier
                    | "set_mark" "=" int
                    | "set_ct_mark" "=" int
                    | "map" "=" reference
                    | "field" "=" identifier
                    | log_block
                    | meter_block

log_block           = "log" "{" log_property* "}"
log_property        = "prefix" "=" string
                    | "level" "=" syslog_level
                    | "group" "=" int

meter_block         = "meter" "{" meter_property* "}"
meter_property      = "name" "=" string
                    | "key" "=" field_name
                    | "rate" "=" rate_expr
                    | "over_action" "=" simple_action

rate_expr           = int "/" interval ("burst" int)?
interval            = "second" | "minute" | "hour" | "day"

set_decl            = "set" identifier "{" set_body "}"
set_body            = (set_property)*
set_property        = "type" "=" set_type
                    | "elements" "=" list_expr
                    | "flags" "=" list_expr
                    | "timeout" "=" duration
                    | "size" "=" int
                    | "comment" "=" string

map_decl            = "map" identifier "{" map_body "}"
map_body            = (map_property)*
map_property        = "type" "=" map_type_expr
                    | "elements" "=" map_elements
                    | "flags" "=" list_expr
                    | "size" "=" int

raw_block           = "raw" "{" raw_line* "}"
raw_line            = string
```

---

## Full Example: Hardened Web Server

```
system web01 {
    firewall {
        set management_hosts {
            type = ipv4_addr
            elements = [
                "10.0.100.10",
                "10.0.100.11",
            ]
        }

        set web_ports {
            type = inet_service
            elements = [80, 443]
        }

        set blocklist {
            type = ipv4_addr
            flags = [timeout]
            timeout = 24h
            size = 65536
            comment = "fail2ban populated blocklist"
        }

        table inet filter {
            chain input {
                policy = drop

                # implicit: accept established/related, drop invalid, accept lo

                rule drop_blocklist {
                    match { saddr = set.blocklist }
                    action = drop
                }

                rule allow_web {
                    match { protocol = tcp; dport = set.web_ports }
                    action = accept
                }

                rule rate_limit_ssh {
                    match {
                        protocol = tcp
                        dport = 22
                        saddr = set.management_hosts
                        ct_state = [new]
                    }
                    action {
                        type = accept
                        meter {
                            name = "ssh_meter"
                            key = saddr
                            rate = 5/minute burst 10
                            over_action = drop
                        }
                    }
                }

                rule allow_icmp {
                    match { protocol = icmp; icmp_type = [echo-request] }
                    action {
                        type = accept
                        log { prefix = "ICMP: "; level = debug }
                    }
                }

                rule allow_icmpv6 {
                    match {
                        protocol = icmpv6
                        icmpv6_type = [
                            echo-request,
                            nd-neighbor-solicit,
                            nd-neighbor-advert,
                            nd-router-solicit,
                            nd-router-advert,
                        ]
                    }
                    action = accept
                }

                rule log_dropped {
                    match { limit = 5/minute burst 10 }
                    action {
                        type = log
                        log { prefix = "INPUT_DROP: "; level = warn }
                    }
                }
            }

            chain forward {
                policy = drop
            }

            chain output {
                policy = accept

                rule allow_dns {
                    match { protocol = udp; dport = 53 }
                    action = accept
                }

                rule allow_ntp {
                    match { protocol = udp; dport = 123 }
                    action = accept
                }

                rule allow_https_out {
                    match { protocol = tcp; dport = 443 }
                    action = accept
                }
            }
        }
    }
}
```

---

## Full Example: NAT Gateway

```
system gateway01 {
    firewall {
        set internal_nets {
            type = ipv4_addr
            flags = [interval]
            elements = [
                "10.0.0.0/8",
                "172.16.0.0/12",
                "192.168.0.0/16",
            ]
        }

        table inet filter {
            chain input {
                policy = drop

                rule allow_ssh {
                    match { protocol = tcp; dport = 22; iif = "eth1" }
                    action = accept
                }

                rule allow_dns {
                    match { protocol = udp; dport = 53; saddr = set.internal_nets }
                    action = accept
                }

                rule allow_icmp {
                    match { protocol = icmp }
                    action = accept
                }
            }

            chain forward {
                policy = drop

                rule allow_internal_out {
                    match { iif = "eth1"; oif = "eth0"; saddr = set.internal_nets }
                    action = accept
                }

                rule allow_return {
                    match { iif = "eth0"; oif = "eth1"; ct_state = [established, related] }
                    action = accept
                }

                rule forward_web_to_dmz {
                    match {
                        iif = "eth0"
                        protocol = tcp
                        dport = [80, 443]
                    }
                    action = accept
                }
            }

            chain output {
                policy = accept
            }
        }

        table inet nat {
            chain postrouting {
                type = nat
                hook = postrouting
                priority = srcnat

                rule masq_outbound {
                    match { oif = "eth0"; saddr = set.internal_nets }
                    action = masquerade
                }
            }

            chain prerouting {
                type = nat
                hook = prerouting
                priority = dstnat

                rule dnat_web {
                    match { iif = "eth0"; protocol = tcp; dport = [80, 443] }
                    action {
                        type = dnat
                        to = "10.0.1.100"
                    }
                }
            }
        }
    }
}
```

---

## Full Example: Firewall as Reusable Class

```
# --- classes/firewall_base.ic ---

class firewall_base {
    description = "Base firewall: default deny, accept ICMP, accept lo"

    firewall {
        table inet filter {
            chain input {
                policy = drop

                rule allow_icmp {
                    match { protocol = icmp; icmp_type = [echo-request] }
                    action = accept
                }

                rule allow_icmpv6 {
                    match {
                        protocol = icmpv6
                        icmpv6_type = [
                            echo-request,
                            nd-neighbor-solicit,
                            nd-neighbor-advert,
                            nd-router-solicit,
                            nd-router-advert,
                        ]
                    }
                    action = accept
                }

                rule log_dropped {
                    match { limit = 5/minute }
                    action {
                        type = log
                        log { prefix = "DROP: " }
                    }
                }
            }

            chain forward {
                policy = drop
            }

            chain output {
                policy = accept
            }
        }
    }
}

# --- classes/firewall_ssh.ic ---

class firewall_ssh extends firewall_base {
    firewall {
        set ssh_allowed {
            type = ipv4_addr
            elements = ["10.0.0.0/8"]
        }

        table inet filter {
            chain input {
                rule allow_ssh {
                    match {
                        protocol = tcp
                        dport = 22
                        saddr = set.ssh_allowed
                    }
                    action = accept
                }
            }
        }
    }
}

# --- classes/firewall_web.ic ---

class firewall_web extends firewall_ssh {
    firewall {
        table inet filter {
            chain input {
                rule allow_http {
                    match { protocol = tcp; dport = [80, 443] }
                    action = accept
                }
            }
        }
    }
}

# --- system.ic ---

import "classes/firewall_web.ic" { firewall_web }

system web01 {
    apply firewall_web

    # Adds rules from firewall_base + firewall_ssh + firewall_web
    # Chain "input" contains (in order):
    #   implicit: accept established/related, drop invalid, accept lo
    #   from firewall_base: allow_icmp, allow_icmpv6, log_dropped
    #   from firewall_ssh: allow_ssh
    #   from firewall_web: allow_http

    # Inline override: tighten SSH to management hosts only
    firewall {
        set ssh_allowed {
            elements = ["10.0.100.0/24"]     # overrides inherited ["10.0.0.0/8"]
        }
    }
}
```

---

## What This Document Does Not Cover

This specification covers nftables-based firewall declarations, cross-validation with services and topology, and class composition. The following topics are defined in separate specifications:

- **Interface declarations** — Named network interfaces, bonding, VLANs, IP addressing (future specification)
- **Traffic shaping and QoS** — tc qdisc/class/filter declarations (future specification)
- **IPsec / WireGuard** — Tunnel declarations and key management (future specification)
- **fail2ban / dynamic blocking** — Integration with dynamic blocklist population (future specification)
- **Compiler flags and build configuration** — Security floor levels that enforce firewall requirements (future specification)
- **Secret management** — Pre-shared keys, WireGuard keys, IPsec credentials (future specification)
