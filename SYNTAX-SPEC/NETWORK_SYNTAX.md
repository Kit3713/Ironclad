# Ironclad Network Syntax Specification

**Status:** Draft — syntax development, Phase 1  
**Scope:** Network interface declarations, IP addressing, routing, DNS, bonding, bridging, VLANs, and cross-validation against firewall, init, and topology declarations  
**Dependencies:** `LANGUAGE_SYNTAX.md`, `FIREWALL_SYNTAX.md`, `INIT_SYNTAX.md`

---

## Design Principles

Networking is how a system connects to the world. Ironclad declares network configuration so the compiler can cross-validate firewall rules against real interfaces, validate service bind addresses, and emit backend-appropriate configuration (NetworkManager keyfiles, systemd-networkd units, or `/etc/sysconfig/network-scripts/`).

The key design constraints:

1. **Interfaces are declared, not discovered.** The compiler does not probe hardware. The operator declares which interfaces exist and how they are configured. This ensures reproducibility — the declaration is the truth, not the hardware state at build time.

2. **The syntax maps to the network stack, not a specific tool.** Interfaces, addresses, routes, and DNS are concepts in the Linux network stack. The compiler emits backend-specific configuration (NetworkManager or systemd-networkd). The operator chooses the backend. The syntax is the same either way.

3. **Cross-validation with firewall is the primary value.** A firewall rule referencing `iif eth0` when no interface `eth0` is declared is a warning. A service binding `0.0.0.0:443` with no interface having a routable address is a warning. These checks are why network declarations exist in the same source tree.

4. **Static configuration is the default.** Ironclad targets servers, not laptops. DHCP is supported but the expected case is static addressing with explicit routes and DNS. Dynamic configuration is an opt-in, not the default.

5. **Virtual interfaces (bonds, bridges, VLANs) compose from physical interfaces.** A bond references its member interfaces. A bridge references its attached interfaces. A VLAN references its parent. The compiler validates these references and emits correct ordering.

---

## System-Level Network Block

Network declarations live inside a `network` block at the system level or inside a class.

```
system web01 {
    network {
        # backend selection, interfaces, routes, DNS
    }
}
```

A system may have at most one `network` block. When classes contribute network declarations, they are merged into the single `network` block using standard merge rules.

---

## Backend Selection

```
network {
    backend = networkmanager      # or: systemd_networkd, scripts
}
```

| Backend             | Description                                                                          |
|---------------------|--------------------------------------------------------------------------------------|
| `networkmanager`    | Emit NetworkManager keyfiles to `/etc/NetworkManager/system-connections/`. Default on RHEL-family. |
| `systemd_networkd`  | Emit `.network`, `.netdev`, and `.link` files to `/etc/systemd/network/`. Common on minimal or container-host systems. |
| `scripts`           | Emit `/etc/sysconfig/network-scripts/ifcfg-*` files. Legacy, for compatibility.      |

The compiler validates that the chosen backend's package is in the system's package list.

---

## Interface Declarations

### Physical Interfaces

```
network {
    interface eth0 {
        type = ethernet
        mac = "52:54:00:12:34:56"     # optional, for matching
        mtu = 9000

        ip {
            address = "10.0.1.10/24"
            gateway = "10.0.1.1"
        }

        ip6 {
            address = "fd00::10/64"
            gateway = "fd00::1"
        }
    }

    interface eth1 {
        type = ethernet
        ip {
            method = dhcp
        }
    }
}
```

### Interface Properties

| Property     | Type             | Default      | Description                                                     |
|--------------|------------------|--------------|-----------------------------------------------------------------|
| `type`       | enum             | `ethernet`   | Interface type: `ethernet`, `bond`, `bridge`, `vlan`, `dummy`, `wireguard`, `loopback`. |
| `mac`        | `string`         | (none)       | MAC address for matching. Not set on the interface — used to identify hardware. |
| `mtu`        | `int`            | (none)       | Maximum transmission unit. Omit for kernel default.             |
| `state`      | `up\|down`       | `up`         | Administrative state on boot.                                   |
| `zone`       | `string`         | (none)       | Firewall zone assignment (NetworkManager only).                 |

### IP Configuration Sub-Block

```
ip {
    method = static                  # static | dhcp | disabled
    address = "10.0.1.10/24"        # CIDR notation
    gateway = "10.0.1.1"
    metric = 100                     # route metric for default gateway
}
```

| Property   | Type             | Default    | Description                                               |
|------------|------------------|------------|-----------------------------------------------------------|
| `method`   | enum             | `static`   | `static`, `dhcp`, or `disabled`.                          |
| `address`  | `string` or `list[string]` | (none) | IPv4 address(es) in CIDR notation. Multiple addresses supported. |
| `gateway`  | `string`         | (none)     | Default gateway for this interface.                       |
| `metric`   | `int`            | (none)     | Route metric for the default gateway.                     |

When `method = static`, `address` is required. When `method = dhcp`, `address` and `gateway` are ignored.

### IPv6 Configuration Sub-Block

```
ip6 {
    method = static                  # static | auto | dhcp | disabled
    address = "fd00::10/64"
    gateway = "fd00::1"
    privacy = false                  # RFC 4941 privacy extensions
}
```

| Property   | Type             | Default    | Description                                               |
|------------|------------------|------------|-----------------------------------------------------------|
| `method`   | enum             | `auto`     | `static`, `auto` (SLAAC), `dhcp` (DHCPv6), or `disabled`.|
| `address`  | `string` or `list[string]` | (none) | IPv6 address(es) in CIDR notation.                   |
| `gateway`  | `string`         | (none)     | Default IPv6 gateway.                                     |
| `privacy`  | `bool`           | `false`    | Enable RFC 4941 privacy extensions.                       |

---

## Bond Interfaces

Bonds aggregate physical interfaces for redundancy or throughput.

```
network {
    interface bond0 {
        type = bond
        mtu = 9000

        bond {
            mode = 802.3ad              # balance-rr | active-backup | balance-xor | broadcast | 802.3ad | balance-tlb | balance-alb
            members = [eth0, eth1]
            lacp_rate = fast             # slow | fast (802.3ad only)
            miimon = 100                 # link monitoring interval (ms)
            xmit_hash = "layer3+4"      # layer2 | layer2+3 | layer3+4
            primary = eth0              # active-backup primary
        }

        ip {
            address = "10.0.1.10/24"
            gateway = "10.0.1.1"
        }
    }
}
```

### Bond Properties

| Property      | Type            | Default        | Description                                              |
|---------------|-----------------|----------------|----------------------------------------------------------|
| `mode`        | enum            | required       | Bonding mode. See table below.                           |
| `members`     | `list[string]`  | required       | Member interface names. Must be declared interfaces.     |
| `miimon`      | `int`           | `100`          | MII link monitoring interval in milliseconds.            |
| `lacp_rate`   | `slow\|fast`    | `slow`         | LACPDU transmission rate. Only for `802.3ad`.            |
| `xmit_hash`   | `string`        | `layer2`       | Transmit hash policy for load-balancing modes.           |
| `primary`     | `string`        | (none)         | Primary interface for `active-backup`.                   |
| `downdelay`   | `int`           | `0`            | Milliseconds to wait before disabling a link.            |
| `updelay`     | `int`           | `0`            | Milliseconds to wait before enabling a link.             |

### Bond Modes

| Mode           | Description                                                   |
|----------------|---------------------------------------------------------------|
| `balance-rr`   | Round-robin transmission for load balancing.                  |
| `active-backup`| Only one member active at a time. Failover on link loss.      |
| `balance-xor`  | XOR-based hash for destination selection.                     |
| `broadcast`    | Transmit on all members.                                      |
| `802.3ad`      | IEEE 802.3ad LACP. Requires switch support.                   |
| `balance-tlb`  | Adaptive transmit load balancing. No switch support required. |
| `balance-alb`  | Adaptive load balancing (transmit + receive).                 |

**Compiler validation:** Member interfaces must be declared, must have `type = ethernet`, and must not have their own `ip` block (IP is configured on the bond, not the members). The compiler errors if members have IP addresses assigned.

---

## Bridge Interfaces

Bridges connect interfaces at Layer 2.

```
network {
    interface br0 {
        type = bridge

        bridge {
            members = [eth0, veth-container0]
            stp = true
            forward_delay = 4
        }

        ip {
            address = "10.0.2.1/24"
        }
    }
}
```

### Bridge Properties

| Property        | Type            | Default | Description                                            |
|-----------------|-----------------|---------|--------------------------------------------------------|
| `members`       | `list[string]`  | `[]`    | Interfaces attached to the bridge. Must be declared.   |
| `stp`           | `bool`          | `false` | Enable Spanning Tree Protocol.                         |
| `forward_delay` | `int`           | `15`    | STP forward delay in seconds.                          |
| `hello_time`    | `int`           | `2`     | STP hello time in seconds.                             |
| `max_age`       | `int`           | `20`    | STP max message age in seconds.                        |
| `ageing_time`   | `int`           | `300`   | MAC address ageing time in seconds.                    |

---

## VLAN Interfaces

VLANs tag traffic on a parent interface.

```
network {
    interface eth0.100 {
        type = vlan

        vlan {
            id = 100
            parent = eth0
        }

        ip {
            address = "10.100.0.10/24"
        }
    }
}
```

### VLAN Properties

| Property  | Type     | Default  | Description                                                 |
|-----------|----------|----------|-------------------------------------------------------------|
| `id`      | `int`    | required | VLAN ID (1-4094).                                           |
| `parent`  | `string` | required | Parent interface name. Must be a declared interface.        |

---

## DNS Configuration

DNS is configured globally in the `network` block.

```
network {
    dns {
        servers = ["10.0.0.1", "10.0.0.2"]
        search = ["example.com", "internal.example.com"]
    }
}
```

### DNS Properties

| Property   | Type            | Default | Description                                        |
|------------|-----------------|---------|----------------------------------------------------|
| `servers`  | `list[string]`  | `[]`    | DNS nameserver addresses, in priority order.       |
| `search`   | `list[string]`  | `[]`    | DNS search domains.                                |

**Compiler behavior:** Emits `/etc/resolv.conf` (or NetworkManager/systemd-resolved configuration depending on backend). The file is marked `generated`.

**DHCP interaction:** When any interface uses `method = dhcp`, the compiler emits a warning if static DNS is also declared — DHCP-provided DNS may override static configuration depending on the backend. The operator can suppress with `dns_override = true` on the `dns` block.

---

## Static Routes

Routes beyond the default gateway are declared in a `routes` block.

```
network {
    routes {
        route "db-subnet" {
            destination = "10.10.0.0/16"
            gateway = "10.0.1.254"
            interface = eth0
            metric = 200
        }

        route "vpn-tunnel" {
            destination = "172.16.0.0/12"
            gateway = "10.0.1.253"
            interface = eth0
        }
    }
}
```

### Route Properties

| Property      | Type     | Default  | Description                                              |
|---------------|----------|----------|----------------------------------------------------------|
| `destination` | `string` | required | Destination network in CIDR notation.                    |
| `gateway`     | `string` | required | Next-hop gateway address.                                |
| `interface`   | `string` | (none)   | Interface for this route. Must be a declared interface.   |
| `metric`      | `int`    | (none)   | Route metric. Lower is preferred.                        |
| `table`       | `int`    | (none)   | Routing table ID. Omit for the main table.               |

---

## Hostname

```
network {
    hostname = "web01.example.com"
}
```

The compiler writes `/etc/hostname` with the short name and ensures the FQDN is resolvable via `/etc/hosts` or DNS. If the system-level `var hostname` exists, it can be referenced:

```
network {
    hostname = ${hostname}
}
```

---

## Classes and Networking

Classes can declare partial network configuration:

```
class dual_nic_server {
    network {
        interface eth0 {
            type = ethernet
            ip { method = static }
        }

        interface eth1 {
            type = ethernet
            ip { method = static }
        }
    }
}
```

The class declares the interface structure. The system provides concrete addresses:

```
system web01 {
    apply dual_nic_server

    network {
        interface eth0 {
            ip { address = "10.0.1.10/24"; gateway = "10.0.1.1" }
        }
        interface eth1 {
            ip { address = "10.0.2.10/24" }
        }
    }
}
```

Merge rules are standard: system inline wins over class, property-level merge, later `apply` wins between classes.

---

## Compiler Cross-Validation

| Validation | Description |
|---|---|
| **Firewall interface references** | Every `iif` and `oif` in firewall rules must reference a declared interface name. Warning if not found. |
| **Firewall interface name references** | Every `iifname` and `oifname` in firewall rules should match a declared interface. Warning if not. |
| **Service bind addresses** | A service binding an address validates that some declared interface carries that address or subnet. |
| **Bond/bridge members exist** | Member interfaces in bonds and bridges must be declared in the same `network` block. |
| **VLAN parent exists** | The VLAN `parent` must be a declared interface. |
| **No IP on bond/bridge members** | Interfaces that are bond or bridge members must not have their own `ip` block. |
| **Default gateway exists** | At least one interface must declare a `gateway` (warning, not error — may be DHCP-provided). |
| **DNS servers reachable** | Static DNS server addresses should be on a subnet reachable from a declared interface (warning). |
| **Duplicate addresses** | No two interfaces may declare the same IP address. |
| **Route interface exists** | Route `interface` must reference a declared interface. |
| **Route gateway reachable** | Route `gateway` should be on a subnet reachable from the route's interface (warning). |
| **Backend package exists** | The chosen backend's package (`NetworkManager`, `systemd-networkd`) must be in the package list. |
| **Topology cross-references** | In topology mode, `system.web01.network.eth0.ip` references are validated against the target system's network declaration. |

---

## Security Floor Enforcement

| Level | Enforcement |
|---|---|
| **Baseline** | No network enforcement. |
| **Standard** | Warning if any interface uses DHCP on a server system. Warning if IPv6 is neither explicitly configured nor explicitly disabled (ambiguous state). |
| **Strict** | Standard warnings become errors. All interfaces must have explicit IP configuration (no reliance on defaults). DNS servers must be explicitly declared. |
| **Maximum** | Strict rules plus: no DHCP permitted. All interfaces must have explicit MAC matching. IPv6 privacy extensions must be disabled (stable addresses required for audit). All routes must be explicitly declared (no implicit connected routes). |

---

## Reserved Keywords

The following words are reserved in network context:

`network`, `interface`, `bond`, `bridge`, `vlan`, `ip`, `ip6`, `dns`, `routes`, `route`, `hostname`, `backend`, `type`, `ethernet`, `loopback`, `dummy`, `wireguard`, `mac`, `mtu`, `state`, `zone`, `method`, `static`, `dhcp`, `auto`, `disabled`, `address`, `gateway`, `metric`, `privacy`, `mode`, `members`, `miimon`, `lacp_rate`, `xmit_hash`, `primary`, `stp`, `forward_delay`, `id`, `parent`, `servers`, `search`, `destination`, `table`

---

## Grammar Summary (Informative)

```
network_block    = "network" "{" network_body "}"
network_body     = property* (interface_decl | dns_block | routes_block)*

interface_decl   = "interface" identifier "{" interface_body "}"
interface_body   = property* (ip_block | ip6_block | bond_block | bridge_block | vlan_block)*

ip_block         = "ip" "{" property* "}"
ip6_block        = "ip6" "{" property* "}"
bond_block       = "bond" "{" property* "}"
bridge_block     = "bridge" "{" property* "}"
vlan_block       = "vlan" "{" property* "}"

dns_block        = "dns" "{" property* "}"
routes_block     = "routes" "{" route_decl* "}"
route_decl       = "route" (identifier | string) "{" property* "}"

property         = identifier "=" value
```

---

## What This Document Does Not Cover

This specification covers network interface and addressing declarations. The following topics are defined in separate specifications:

- **Firewall rules** — Network security policy built on declared interfaces (`FIREWALL_SYNTAX.md`)
- **Service bind addresses** — How services reference network addresses (`INIT_SYNTAX.md`)
- **Topology networking** — Cross-system network references in fleet declarations (future specification)
- **VPN configuration** — WireGuard, IPsec tunnel configuration (future specification or stdlib class)
- **Advanced routing** — Policy routing, VRF, multipath routing (future specification)
