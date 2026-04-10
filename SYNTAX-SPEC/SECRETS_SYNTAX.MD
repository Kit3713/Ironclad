# Ironclad Secrets Syntax Specification

**Status:** Draft — syntax development, Phase 1  
**Category:** Hybrid — the `var secret` type, generation profiles, and the `@@` redaction operator are compiler-native language features. Secret keeper backend configurations (Vault, age, SOPS, systemd-creds, Clevis, cloud KMS, etc.) are standard library class APIs. The compiler understands secret declarations and routes them to keepers, but individual keeper implementations are stdlib classes.  
**Scope:** Secret variable types, user-definable secret generation profiles, build-time and runtime secret resolution, secret keeper backend configuration, the `@@` redaction operator, audit storage, SELinux MLS labeling, and cross-validation against user, storage, service, and SELinux declarations

---

## Design Principles

Ironclad's secret system is a declaration and reference layer integrated into the variable type system. It does not implement a secrets engine. It declares what secrets exist, how they are generated, how they are typed, and which existing secret keeper backend manages their lifecycle. The compiler resolves build-time secrets during compilation and emits backend-appropriate configuration for runtime secrets. Secret values never appear in Ironclad source files.

The key design constraints:

1. **Secrets are a variable type.** A secret is declared with `var secret` or with a user-defined secret type name. It participates in the standard reference and scope system. It can be used inline wherever a value is expected. The compiler handles routing to the configured backend — the operator declares secrets as they go, not in a separate manifest.

2. **Secret types are user-definable.** A `secret_type` declaration defines a generation profile — character set, length, algorithm, format constraints. Once defined, the type name is used like any other type. `var my_type db_pass` creates a secret generated according to that profile. This allows different applications and backends to receive validly-formatted credentials without the operator hand-crafting each one.

3. **The backend is configured once, used everywhere.** A `secret_keeper` block configures the backend system — Vault, systemd-creds, age, SOPS, TPM2, Clevis/Tang, AWS SSM, or a simple encrypted directory. Secrets are routed to the configured keeper automatically. The operator does not annotate each secret with backend details unless they need to override the default.

4. **Build-time and runtime are separate resolution paths.** Build-time secrets are resolved during compilation — the compiler reads or generates a value and injects it into the output artifacts. Runtime secrets are never seen by the compiler — it emits configuration that tells the keeper to deliver the value to a declared path on the running system. The scope is explicit on each secret.

5. **Redaction is explicit, not automatic.** Nothing is automatically stripped from the on-system source copy — not even secrets. The operator uses the `@@` redaction operator to mark any code that should be compiled but not copied to the deployed system. This is a general-purpose language feature, not secret-specific. A `@@`-wrapped secret declaration, a bootstrap IP, a temporary credential, or an entire setup block can be redacted. The operator decides what the deployed system's source copy reveals.

6. **SELinux MLS aware.** Secret declarations carry an optional sensitivity label. The compiler validates that files receiving secret values have SELinux contexts with ranges that dominate the secret's declared sensitivity. The audit store path is labeled at the declared sensitivity. This ensures secrets do not flow to contexts below their classification.

---

## Secret Keeper Configuration

Before any secrets can be declared, the system must configure at least one secret keeper backend. The `secret_keeper` block declares how secrets are stored, retrieved, and (for runtime) delivered.

A system may declare multiple keepers for different scopes or purposes. One keeper is marked `default` and receives all secrets that do not explicitly specify a keeper.

### Syntax

```
secret_keeper <type> "<n>" {
    # type-specific configuration
    default = true      # optional — makes this the default keeper
    scope = build        # optional — restricts this keeper to build or runtime
}
```

### The `default` Property

At most one keeper may have `default = true` per scope. If only one keeper is declared, it is implicitly default for both scopes. If multiple keepers exist, at least one must be marked default or the compiler emits an error when a secret does not specify a keeper.

### The `scope` Property

| Value     | Description                                                                |
|-----------|----------------------------------------------------------------------------|
| `build`   | This keeper handles build-time secrets only.                               |
| `runtime` | This keeper handles runtime secrets only.                                  |
| `both`    | This keeper handles both scopes. Default when `scope` is omitted.          |

---

## Built-In Backend Types

### `directory` — Encrypted Directory on Build Host

The simplest backend. Secret files are read from a directory on the build host. The operator is responsible for encrypting the directory (LUKS, eCryptfs, etc.). Suitable for single-developer workflows, airgapped builds, and small teams.

```
secret_keeper directory "local" {
    default = true
    scope = build
    path = "/opt/ironclad/secrets/web01"
}
```

| Property  | Type   | Default  | Description                                                                     |
|-----------|--------|----------|---------------------------------------------------------------------------------|
| `path`    | `path` | required | Absolute path to the secrets directory on the build host.                       |

**File mapping:** A secret named `db_pass` resolves to the file `<path>/db_pass`. A secret declared inside a class or nested scope uses its fully qualified name: a secret `pass` inside class `postgres_role` resolves to `<path>/postgres_role/pass` — dots and scope separators map to directory separators.

**Compiler behavior:**
- Verifies the path exists and is readable.
- Checks whether the path resides on an encrypted filesystem. If not, emits a warning: `WARNING: secret_keeper "local" path is not on an encrypted filesystem — secrets at rest may be exposed`.
- For generated secrets: writes the generated value to the corresponding file if it does not already exist. If the file exists, uses the existing value (idempotent generation).

---

### `age` — Age-Encrypted Files

Secrets are stored as age-encrypted files in the source tree or a designated directory. The compiler decrypts at build time using a specified identity file. Suitable for teams that commit encrypted secrets alongside source.

```
secret_keeper age "team" {
    default = true
    scope = build
    path = "secrets/"                       # relative to source tree root
    identity = "/home/builder/.age/key.txt"
    recipients = [
        "age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p",
        "age1tg0v5ew8jcr4a3qhm9kyavnxmf0ew5ugn43vqw7n83hx8jklqfqsa7dlf",
    ]
}
```

| Property     | Type             | Default  | Description                                                              |
|--------------|------------------|----------|--------------------------------------------------------------------------|
| `path`       | `path`           | required | Directory containing `.age` encrypted files. Relative to source root or absolute. |
| `identity`   | `path`           | required | Path to the age identity (private key) used for decryption at build time.|
| `recipients` | `list[string]`   | `[]`     | Age public keys. Used when the compiler generates a new secret and encrypts it for storage. All recipients can decrypt. |

**File mapping:** Secret `db_pass` resolves to `<path>/db_pass.age`. The compiler decrypts with the identity file.

**Generated secrets:** When the compiler generates a value, it encrypts the value to all declared recipients and writes `<path>/<name>.age`. Subsequent builds read the existing encrypted file rather than regenerating.

---

### `sops` — Mozilla SOPS Structured Encryption

Secrets are stored in a SOPS-encrypted structured file (YAML, JSON, or dotenv). SOPS supports age, GPG, AWS KMS, GCP KMS, Azure Key Vault, and HashiCorp Vault as encryption backends. Suitable for teams with existing SOPS workflows or cloud KMS requirements.

```
secret_keeper sops "cloud" {
    default = true
    scope = build
    file = "secrets/web01.sops.yaml"
    key_type = age
    age_identity = "/home/builder/.age/key.txt"
}
```

```
secret_keeper sops "aws" {
    scope = build
    file = "secrets/web01.sops.yaml"
    key_type = kms
    kms_arn = "arn:aws:kms:us-east-1:123456789:key/abcdef..."
}
```

| Property       | Type     | Default  | Description                                                             |
|----------------|----------|----------|-------------------------------------------------------------------------|
| `file`         | `path`   | required | Path to the SOPS-encrypted file.                                        |
| `key_type`     | enum     | required | Encryption backend: `age`, `gpg`, `aws_kms`, `gcp_kms`, `azure_kv`, `vault`. |
| `age_identity` | `path`   | (none)   | Age identity file. Required when `key_type = age`.                      |
| `gpg_fingerprint` | `string` | (none) | GPG key fingerprint. Required when `key_type = gpg`.                   |
| `kms_arn`      | `string` | (none)   | AWS KMS key ARN. Required when `key_type = aws_kms`.                    |
| `gcp_kms_key`  | `string` | (none)   | GCP KMS resource name. Required when `key_type = gcp_kms`.             |
| `azure_kv_url` | `string` | (none)   | Azure Key Vault key URL. Required when `key_type = azure_kv`.          |
| `vault_address`| `string` | (none)   | Vault address. Required when `key_type = vault`.                        |
| `vault_path`   | `string` | (none)   | Vault transit key path. Required when `key_type = vault`.               |

**Key mapping:** Secret `db_pass` resolves to the key `db_pass` inside the SOPS file. Hierarchical names use dot notation in YAML: `users.kit.password` maps to `users.kit.password` in the YAML structure.

---

### `vault` — HashiCorp Vault

Secrets are stored in and retrieved from a Vault server. Supports both build-time (compiler fetches from Vault during build) and runtime (system retrieves from Vault at boot or on demand via Vault Agent).

```
secret_keeper vault "production" {
    default = true
    scope = both

    address = "https://vault.internal:8200"
    auth_method = approle
    role_id_file = "/opt/ironclad/vault-role-id"
    secret_id_file = "/opt/ironclad/vault-secret-id"
    mount = "secret"
    namespace = "infrastructure"
    base_path = "ironclad/web01"
    tls {
        ca_cert = "/etc/pki/tls/certs/vault-ca.crt"
        client_cert = "/etc/pki/tls/certs/vault-client.crt"
        client_key = "/etc/pki/tls/private/vault-client.key"
    }

    # Runtime-specific: Vault Agent config emitted into image
    runtime {
        agent_config_path = /etc/vault-agent.d
        auto_auth {
            method = approle
            role_id_file = "/run/vault/role-id"
            secret_id_file = "/run/vault/secret-id"
        }
        template_dir = /etc/vault-agent.d/templates
    }
}
```

| Property          | Type     | Default      | Description                                                               |
|-------------------|----------|--------------|---------------------------------------------------------------------------|
| `address`         | `string` | required     | Vault server URL.                                                         |
| `auth`            | block    | required     | Authentication configuration. See auth method sub-blocks below.           |
| `mount`           | `string` | `"secret"`   | Default secrets engine mount path.                                        |
| `namespace`       | `string` | `""`         | Vault namespace (enterprise feature).                                     |
| `base_path`       | `string` | `""`         | Prefix prepended to all secret paths. `db_pass` becomes `<base_path>/db_pass`. |
| `tls`             | block    | (none)       | TLS configuration for Vault connection.                                   |
| `engine`          | block    | (none)       | Secrets engine configuration. Optional — defaults to KV v2.               |
| `agent`           | block    | (none)       | Vault Agent runtime configuration. Required when `scope` includes `runtime`. |
| `backend_config`  | block    | (none)       | Pass-through configuration. See Backend Config Escape Hatch below.        |

#### Auth Method Sub-Blocks

Each auth method has its own structured configuration. The compiler validates required fields per method.

**AppRole** — the most common machine auth method:

```
auth {
    method = approle
    mount = "approle"                        # auth mount path (default: "approle")
    role_id_file = "/opt/vault/role-id"
    secret_id_file = "/opt/vault/secret-id"
    secret_id_wrapping = true                # expect response-wrapped secret ID
    token_policies = ["ironclad-read"]       # expected policies (validated at build time)
}
```

| Property               | Type           | Default      | Description                                              |
|------------------------|----------------|--------------|----------------------------------------------------------|
| `role_id_file`         | `path`         | required     | Path to file containing the role ID.                     |
| `secret_id_file`       | `path`         | required     | Path to file containing the secret ID.                   |
| `secret_id_wrapping`   | `bool`         | `false`      | Whether the secret ID is response-wrapped.               |
| `token_policies`       | `list[string]` | `[]`         | Expected token policies. Compiler warns if mismatch.     |

**Kubernetes** — for systems running in K8s:

```
auth {
    method = kubernetes
    mount = "kubernetes"
    role = "ironclad-db01"
    jwt_path = "/var/run/secrets/kubernetes.io/serviceaccount/token"
    kubernetes_host = "https://kubernetes.default.svc"
    kubernetes_ca_cert = "/var/run/secrets/kubernetes.io/serviceaccount/ca.crt"
}
```

| Property            | Type     | Default                                                        | Description                                |
|---------------------|----------|----------------------------------------------------------------|--------------------------------------------|
| `role`              | `string` | required                                                       | Vault Kubernetes auth role.                |
| `jwt_path`          | `path`   | `/var/run/secrets/kubernetes.io/serviceaccount/token`           | Path to the service account JWT.           |
| `kubernetes_host`   | `string` | `"https://kubernetes.default.svc"`                             | Kubernetes API server URL.                 |
| `kubernetes_ca_cert`| `path`   | `/var/run/secrets/kubernetes.io/serviceaccount/ca.crt`         | Path to the K8s CA cert.                   |

**TLS Certificate:**

```
auth {
    method = cert
    mount = "cert"
    client_cert = "/etc/pki/tls/certs/vault-client.crt"
    client_key = "/etc/pki/tls/private/vault-client.key"
    role = "ironclad"
}
```

| Property      | Type     | Default  | Description                              |
|---------------|----------|----------|------------------------------------------|
| `client_cert` | `path`   | required | Client certificate path.                 |
| `client_key`  | `path`   | required | Client private key path.                 |
| `role`        | `string` | `""`     | TLS auth role name.                      |

**Token** — simplest method, typically only for development:

```
auth {
    method = token
    token_file = "/opt/vault/token"
    renewable = true
}
```

| Property     | Type   | Default  | Description                              |
|--------------|--------|----------|------------------------------------------|
| `token_file` | `path` | required | Path to file containing the Vault token. |
| `renewable`  | `bool` | `true`   | Whether the token is renewable.          |

**LDAP:**

```
auth {
    method = ldap
    mount = "ldap"
    username_file = "/opt/vault/ldap-user"
    password_file = "/opt/vault/ldap-pass"
}
```

| Property        | Type   | Default  | Description                              |
|-----------------|--------|----------|------------------------------------------|
| `username_file` | `path` | required | Path to file containing LDAP username.   |
| `password_file` | `path` | required | Path to file containing LDAP password.   |

**JWT/OIDC:**

```
auth {
    method = jwt
    mount = "jwt"
    role = "ironclad"
    jwt_file = "/opt/vault/jwt-token"
}
```

| Property   | Type     | Default  | Description                              |
|------------|----------|----------|------------------------------------------|
| `role`     | `string` | required | JWT auth role name.                      |
| `jwt_file` | `path`   | required | Path to file containing the JWT.         |

#### TLS Block

```
tls {
    ca_cert = "/etc/pki/tls/certs/vault-ca.crt"
    client_cert = "/etc/pki/tls/certs/vault-client.crt"
    client_key = "/etc/pki/tls/private/vault-client.key"
    tls_server_name = "vault.internal"
    skip_verify = false
}
```

| Property          | Type     | Default  | Description                                          |
|-------------------|----------|----------|------------------------------------------------------|
| `ca_cert`         | `path`   | system   | CA certificate for verifying Vault's TLS cert.       |
| `client_cert`     | `path`   | (none)   | Client cert for mutual TLS (if not using cert auth). |
| `client_key`      | `path`   | (none)   | Client key for mutual TLS.                           |
| `tls_server_name` | `string` | (none)   | SNI server name to send. Useful behind load balancers.|
| `skip_verify`     | `bool`   | `false`  | Disable TLS verification. **Never use in production.**|

#### Secrets Engine Configuration

By default, the compiler assumes KV v2 at the declared `mount` path. For other engines, declare an `engine` block:

```
# PKI engine — issue TLS certificates from Vault's CA
engine pki "internal_ca" {
    type = pki
    mount = "pki"
    role = "web-server"
    common_name_template = "${system.name}.internal"
    ttl = "720h"
    alt_names = ["${system.name}", "${var.fqdn}"]
    ip_sans = ["${var.ip}"]
}

# Database engine — dynamic credentials
engine database "postgres" {
    type = database
    mount = "database"
    role = "readonly"
    ttl = "1h"
    max_ttl = "24h"
}

# Transit engine — encryption as a service
engine transit "encrypt" {
    type = transit
    mount = "transit"
    key_name = "ironclad-data"
}
```

| Property               | Type     | Default   | Description                                                        |
|------------------------|----------|-----------|--------------------------------------------------------------------|
| `type`                 | enum     | required  | Engine type: `kv2`, `pki`, `database`, `transit`, `ssh`, `aws`, `gcp`, `azure`. |
| `mount`                | `string` | required  | Secrets engine mount path.                                         |
| `role`                 | `string` | (none)    | Role for engines that use roles (PKI, database, SSH).              |
| `ttl`                  | duration | (none)    | Default TTL for dynamic secrets / issued certs.                    |
| `max_ttl`              | duration | (none)    | Maximum TTL.                                                       |
| `common_name_template` | `string` | (none)    | PKI: template for certificate CN. Supports `${}` interpolation.   |
| `alt_names`            | `list[string]` | `[]` | PKI: Subject Alternative Names.                                   |
| `ip_sans`              | `list[string]` | `[]` | PKI: IP SANs.                                                     |
| `key_name`             | `string` | (none)    | Transit: encryption key name.                                      |

Engine-specific properties beyond this table use `backend_config` (see below).

#### Vault Agent Block

Controls the Vault Agent process emitted into the image for runtime secret delivery.

```
agent {
    config_path = /etc/vault-agent.d
    user = vault-agent
    group = vault-agent

    auto_auth {
        # Inherits from the keeper's auth block by default.
        # Override here for runtime-specific auth (e.g., different role).
        method = kubernetes
        role = "ironclad-db01-runtime"
    }

    cache {
        enabled = true
        use_auto_auth_token = "force"
        persist = true
        persist_dir = /var/lib/vault-agent/cache
    }

    template_config {
        static_secret_render_interval = "5m"
        exit_on_retry_failure = true
    }

    listener {
        type = tcp
        address = "127.0.0.1:8200"
        tls_disable = true
    }

    env_template {
        enabled = false        # render secrets as env vars for services
    }
}
```

| Property          | Type     | Default                      | Description                                                   |
|-------------------|----------|------------------------------|---------------------------------------------------------------|
| `config_path`     | `path`   | `/etc/vault-agent.d`         | Directory for agent config and templates on the target system.|
| `user`            | `string` | `vault-agent`                | User to run the agent as. Validated against user declarations.|
| `group`           | `string` | `vault-agent`                | Group for the agent.                                          |
| `auto_auth`       | block    | (inherits keeper `auth`)     | Runtime auth override. Uses the keeper's `auth` if omitted.  |
| `cache`           | block    | (none)                       | Agent caching configuration.                                  |
| `template_config` | block    | (none)                       | Template rendering settings.                                  |
| `listener`        | block    | (none)                       | Local API proxy listener.                                     |
| `env_template`    | block    | (none)                       | Environment variable rendering.                               |

The compiler generates:
- Agent HCL config at `<config_path>/agent.hcl`
- Template files at `<config_path>/templates/` for each runtime secret
- A systemd service or s6 longrun (matching the system's init) for the agent process
- The `vault-agent` user and group (if not already declared)

**Build-time behavior:** Compiler authenticates to Vault using the `auth` block, reads `<mount>/data/<base_path>/<secret_name>` (KV v2) or issues/generates via the declared engine, and uses the value.

**Runtime behavior:** Compiler emits Vault Agent configuration into the image. Vault Agent runs as a service, authenticates, and renders secret templates to declared destination paths. The compiler generates the Vault Agent template files from secret declarations.

---

### `systemd_creds` — systemd Credentials

Secrets are encrypted at build time using `systemd-creds` and bound to the system's TPM2 PCR state. Decrypted at runtime by systemd when the consuming service starts. Suitable for service credentials on systemd-managed systems with TPM2.

```
secret_keeper systemd_creds "tpm" {
    default = true
    scope = runtime

    tpm2 = true
    pcr_mask = "7+11+14"
    credentials_dir = /etc/credstore.encrypted
    with_key = tpm2
    name_prefix = ""
}
```

| Property          | Type     | Default                        | Description                                                       |
|-------------------|----------|--------------------------------|-------------------------------------------------------------------|
| `tpm2`            | `bool`   | `true`                         | Bind credentials to TPM2. Requires TPM2 hardware at runtime.      |
| `pcr_mask`        | `string` | `"7+11+14"`                    | PCR registers to bind against. Default covers Secure Boot + kernel + shim. |
| `credentials_dir` | `path`   | `/etc/credstore.encrypted`     | Directory where encrypted credential files are stored in the image.|
| `with_key`        | enum     | `tpm2`                         | Encryption binding: `tpm2`, `host`, `tpm2-absent` (host key fallback when no TPM2), `host+tpm2`. |
| `name_prefix`     | `string` | `""`                           | Prefix added to credential names in the credstore.                |
| `not_after`       | `string` | (none)                         | Credential expiry timestamp (ISO 8601). `systemd-creds` rejects decryption after this time. |
| `backend_config`  | block    | (none)                         | Pass-through configuration. See Backend Config Escape Hatch below.|

**Compiler behavior:** At build time, the compiler invokes `systemd-creds encrypt` for each runtime secret assigned to this keeper, producing an encrypted credential file. The file is placed at `<credentials_dir>/<secret_name>.cred` in the image. The consuming service unit receives a `SetCredentialEncrypted=` or `LoadCredentialEncrypted=` directive.

---

### `clevis` — Clevis/Tang Network-Bound Encryption

Secrets are encrypted to a Tang server's key. Decrypted at runtime only when the Tang server is reachable on the network. Suitable for LUKS key escrow and network-bound secret release in datacenter environments.

```
secret_keeper clevis "datacenter" {
    scope = runtime

    pin = tang
    tang {
        url = "http://tang.internal:8080"
        thumbprint = "abc123def456..."
    }
}
```

```
secret_keeper clevis "multi" {
    scope = runtime

    pin = sss
    sss {
        threshold = 2
        pins = [
            { pin = tang; tang { url = "http://tang1:8080"; thumbprint = "aaa..." } },
            { pin = tang; tang { url = "http://tang2:8080"; thumbprint = "bbb..." } },
            { pin = tpm2; tpm2 { pcr_mask = "7" } },
        ]
    }
}
```

| Property    | Type     | Default  | Description                                                                  |
|-------------|----------|----------|------------------------------------------------------------------------------|
| `pin`       | enum     | required | Clevis pin: `tang`, `tpm2`, `sss` (Shamir Secret Sharing for multi-pin).    |
| `tang`      | block    | (none)   | Tang server configuration. Required when `pin = tang` or inside `sss`.       |
| `tpm2`      | block    | (none)   | TPM2 configuration. Required when `pin = tpm2` or inside `sss`.             |
| `sss`       | block    | (none)   | Shamir configuration. Required when `pin = sss`.                             |

Tang block properties:

| Property      | Type     | Default  | Description                              |
|---------------|----------|----------|------------------------------------------|
| `url`         | `string` | required | Tang server URL.                         |
| `thumbprint`  | `string` | required | Tang server key thumbprint for pinning.  |

SSS block properties:

| Property    | Type             | Default  | Description                                                      |
|-------------|------------------|----------|------------------------------------------------------------------|
| `threshold` | `int`            | required | Number of pins that must succeed for decryption.                 |
| `pins`      | `list[pin_decl]` | required | Child pin declarations (tang, tpm2, or nested sss).              |

**Compiler behavior:** Emits `clevis luks bind` commands in the Kickstart configuration for LUKS-bound secrets. For file-based secrets, emits encrypted blobs and a systemd service that calls `clevis decrypt` at boot to deliver plaintext to a tmpfs-mounted path.

---

### `aws_ssm` — AWS Systems Manager Parameter Store

```
secret_keeper aws_ssm "cloud" {
    scope = runtime
    region = "us-east-1"
    prefix = "/ironclad/web01"
    kms_key_id = "alias/ironclad-secrets"
    
    auth {
        method = iam_role                   # default: instance/task IAM role
    }
}
```

```
secret_keeper aws_ssm "cross_account" {
    scope = runtime
    region = "us-east-1"
    prefix = "/ironclad/web01"
    
    auth {
        method = assume_role
        role_arn = "arn:aws:iam::123456789:role/ironclad-secrets"
        external_id = "ironclad-web01"
    }
}
```

| Property     | Type     | Default     | Description                                                        |
|--------------|----------|-------------|--------------------------------------------------------------------|
| `region`     | `string` | required    | AWS region.                                                        |
| `prefix`     | `string` | `""`        | Prefix prepended to parameter names.                               |
| `kms_key_id` | `string` | (none)      | KMS key for encryption. Uses AWS-managed key if omitted.           |
| `auth`       | block    | `iam_role`  | Auth config. `iam_role` (instance/task role), `assume_role`, `profile` (named credential profile). |
| `tier`       | enum     | `standard`  | Parameter tier: `standard` (4KB), `advanced` (8KB, param policies).|
| `backend_config` | block | (none)     | Pass-through configuration. See Backend Config Escape Hatch.       |

**Compiler behavior:** Emits a helper service/script that fetches parameters from SSM at boot using the configured auth method, writing values to declared destination paths on a tmpfs mount.

---

### `aws_secrets_manager` — AWS Secrets Manager

```
secret_keeper aws_secrets_manager "prod" {
    scope = runtime
    region = "us-east-1"
    prefix = "ironclad/web01/"
    
    auth {
        method = iam_role
    }
}
```

| Property        | Type     | Default    | Description                                             |
|-----------------|----------|------------|---------------------------------------------------------|
| `region`        | `string` | required   | AWS region.                                             |
| `prefix`        | `string` | `""`       | Prefix prepended to secret names.                       |
| `auth`          | block    | `iam_role` | Same auth methods as `aws_ssm`.                         |
| `version_stage` | `string` | `"AWSCURRENT"` | Version stage to retrieve.                         |
| `backend_config` | block   | (none)     | Pass-through configuration. See Backend Config Escape Hatch. |

---

### `gcp_secret_manager` — Google Cloud Secret Manager

```
secret_keeper gcp_secret_manager "gcp" {
    scope = runtime
    project = "my-project-123"
    prefix = "ironclad-web01-"
    
    auth {
        method = metadata_server             # default: GCE metadata server
    }
}
```

| Property            | Type     | Default            | Description                                             |
|---------------------|----------|--------------------|---------------------------------------------------------|
| `project`           | `string` | required           | GCP project ID.                                         |
| `prefix`            | `string` | `""`               | Prefix prepended to secret names.                       |
| `auth`              | block    | `metadata_server`  | Auth config. `metadata_server`, `service_account_key` (JSON key file), `workload_identity`. |
| `service_account_key_file` | `path` | (none)      | Path to service account JSON key. For `auth.method = service_account_key`. |
| `backend_config`    | block    | (none)             | Pass-through configuration. See Backend Config Escape Hatch. |

---

### `azure_keyvault` — Azure Key Vault

```
secret_keeper azure_keyvault "azure" {
    scope = runtime
    vault_url = "https://mykeyvault.vault.azure.net"
    prefix = "ironclad-web01-"
    
    auth {
        method = managed_identity            # default: Azure managed identity
    }
}
```

| Property        | Type     | Default             | Description                                             |
|-----------------|----------|---------------------|---------------------------------------------------------|
| `vault_url`     | `string` | required            | Azure Key Vault URL.                                    |
| `prefix`        | `string` | `""`                | Prefix prepended to secret names.                       |
| `auth`          | block    | `managed_identity`  | Auth config. `managed_identity`, `service_principal` (client ID + client secret/cert), `cli` (development only). |
| `tenant_id`     | `string` | (none)              | Azure AD tenant ID. Required for `service_principal`.   |
| `client_id`     | `string` | (none)              | Application client ID. For `service_principal` or user-assigned managed identity. |
| `client_secret_file` | `path` | (none)           | Path to client secret. For `service_principal`.         |
| `backend_config` | block   | (none)              | Pass-through configuration. See Backend Config Escape Hatch. |

---

### `pkcs11` — Hardware Security Module (PKCS#11)

For HSM-backed secret storage. Suitable for classified/high-security environments where key material must never leave hardware.

```
secret_keeper pkcs11 "hsm" {
    scope = both
    module = "/usr/lib64/pkcs11/opensc-pkcs11.so"
    slot = 0
    pin_file = "/opt/ironclad/hsm-pin"
    token_label = "ironclad-secrets"
}
```

| Property        | Type     | Default  | Description                                                |
|-----------------|----------|----------|------------------------------------------------------------|
| `module`        | `path`   | required | Path to the PKCS#11 provider shared library.               |
| `slot`          | `int`    | `0`      | HSM slot number.                                           |
| `pin_file`      | `path`   | (none)   | Path to file containing the HSM PIN.                       |
| `token_label`   | `string` | (none)   | Token label to use on the HSM.                             |
| `key_type`      | enum     | `aes256` | Key type for wrapping secrets: `aes256`, `rsa2048`, `rsa4096`, `ec_p256`, `ec_p384`. |
| `backend_config` | block   | (none)   | Pass-through configuration. See Backend Config Escape Hatch. |

---

### `custom` — User-Defined Backend

For backends not covered by built-in types. The operator provides a script or binary that the compiler invokes for read/write/generate operations, plus a runtime delivery mechanism.

```
secret_keeper custom "corp_vault" {
    scope = both

    build {
        read_command = "/usr/local/bin/corp-secret read"
        write_command = "/usr/local/bin/corp-secret write"
        generate_command = "/usr/local/bin/corp-secret generate"
    }

    runtime {
        service_unit = "corp-secret-agent.service"
        delivery_dir = /run/secrets
    }
}
```

| Property   | Type  | Description                                                                                         |
|------------|-------|-----------------------------------------------------------------------------------------------------|
| `build`    | block | Build-time commands. `read_command` is invoked as `<cmd> <name>` and must print the value to stdout. `write_command` is invoked as `<cmd> <name> <value>`. `generate_command` is invoked as `<cmd> <name> <type_params_json>` and must print the generated value to stdout. |
| `runtime`  | block | Runtime delivery. `service_unit` names a service that delivers secrets. `delivery_dir` is where the service writes plaintext files. |

The custom backend allows organizations with proprietary secret management infrastructure to integrate without forking Ironclad.

---

## Backend Config Escape Hatch

Every `secret_keeper` type supports a `backend_config` block for configuration that exceeds the structured syntax. This follows the same philosophy as `raw` blocks in `FIREWALL_SYNTAX.MD` and raw bash in `FILEOPS_SYNTAX.MD`: use the structured properties for the common cases the compiler validates, and `backend_config` for the long tail the compiler passes through without parsing.

### Syntax

```
secret_keeper vault "production" {
    # Structured — compiler validates
    address = "https://vault.internal:8200"
    auth {
        method = approle
        role_id_file = "/opt/vault/role-id"
        secret_id_file = "/opt/vault/secret-id"
    }
    mount = "secret"

    # Pass-through — compiler emits verbatim into backend config
    backend_config {
        format = hcl            # output format: hcl, json, yaml, toml, ini, raw

        set max_retries = 3
        set retry_interval = "5s"
        set disable_mlock = true

        set telemetry.statsite_address = "statsite.internal:8125"
        set telemetry.disable_hostname = true

        set service_registration.consul.address = "consul.internal:8500"
        set service_registration.consul.scheme = "https"
        set service_registration.consul.service = "vault-agent"
    }
}
```

### Properties

| Property | Type   | Default | Description                                                                                      |
|----------|--------|---------|--------------------------------------------------------------------------------------------------|
| `format` | enum   | varies  | Output format for the pass-through config. Default is backend-native: `hcl` for Vault, `ini` for systemd-creds, `json` for cloud backends. Options: `hcl`, `json`, `yaml`, `toml`, `ini`, `raw`. |

### Operations

`backend_config` supports the same format-aware operations as the `edit` block in `FILEOPS_SYNTAX.MD`:

```
backend_config {
    format = hcl

    # Set a key-path value
    set vault.retry.num_retries = 5

    # Set a block value
    set vault.listener "tcp" {
        address = "127.0.0.1:8200"
        tls_disable = true
    }

    # Delete a key
    delete vault.seal.transit

    # Set only if not already set by structured properties
    set_default vault.log_level = "info"
}
```

The compiler:
1. Generates the backend configuration from the structured properties.
2. Applies `backend_config` operations on top, using the format-aware editor for the declared format.
3. Emits the final merged configuration file.

This means structured properties and `backend_config` compose — the operator uses structured syntax for the validated core, then `backend_config` for advanced tuning without rewriting the entire config.

### Raw Pass-Through

When `format = raw`, the content is emitted verbatim with no format-aware editing. This is the absolute last resort:

```
backend_config {
    format = raw

    content = """
        max_retries = 3
        disable_mlock = true

        telemetry {
          statsite_address = "statsite.internal:8125"
        }
    """
}
```

### Per-Backend Examples

**Vault** — advanced agent caching and listener config:

```
secret_keeper vault "production" {
    address = "https://vault.internal:8200"
    auth { method = approle; role_id_file = "/opt/vault/role-id"; secret_id_file = "/opt/vault/secret-id" }

    backend_config {
        format = hcl

        set cache.use_auto_auth_token = "force"
        set cache.persist.type = "kubernetes"
        set cache.persist.path = "/vault/agent-cache"

        set listener "tcp" {
            address = "127.0.0.1:8200"
            tls_disable = true
        }

        set listener "unix" {
            address = "/run/vault-agent/socket"
            tls_disable = true
        }

        set vault.retry.num_retries = 5
        set vault.retry.backoff = "250ms"
    }
}
```

**systemd-creds** — non-standard credential flags:

```
secret_keeper systemd_creds "local" {
    tpm2 = true
    pcr_mask = "7+11+14"

    backend_config {
        format = raw

        # Passed as additional flags to systemd-creds encrypt invocations
        content = "--with-key=host+tpm2 --tpm2-public-key=/etc/systemd/tpm2-pcr-public-key.pem"
    }
}
```

**AWS SSM** — IAM role assumption chain:

```
secret_keeper aws_ssm "cross_account" {
    scope = runtime
    region = "us-east-1"
    prefix = "/ironclad/web01"

    backend_config {
        format = json

        set assume_role.role_arn = "arn:aws:iam::123456789:role/ironclad-secrets-reader"
        set assume_role.session_name = "ironclad-build"
        set assume_role.external_id = "ironclad-web01"
        set retry.max_attempts = 5
        set retry.mode = "adaptive"
    }
}
```

### Compiler Behavior

- The compiler does **not** validate `backend_config` content beyond basic format well-formedness (valid HCL, valid JSON, etc.).
- A warning is emitted for every `backend_config` block — it indicates the structured syntax does not cover the operator's use case. This mirrors the `raw` block warning in the firewall spec.
- At higher security floor levels, `backend_config { format = raw }` is promoted from warning to error. Format-aware `backend_config` remains a warning.

---

## Secret Types

### The Built-In `secret` Type

The base `secret` type is a variable modifier. It marks a variable as sensitive — the compiler routes it to the default keeper. The operator wraps secret declarations in `@@` to redact them from the on-system source copy.

```
var secret db_pass = "initial_value"          # literal (build-time, stored in keeper)
var secret luks_key                            # no value — must exist in keeper already
var secret api_token: generated                # compiler generates using default profile
```

### User-Defined Secret Types: `secret_type`

A `secret_type` declaration defines a generation profile. Once declared, the type name is used in place of `secret` to create variables that are automatically generated with the specified parameters.

```
secret_type strong_pass {
    length = 32
    charset = alphanumeric_special
    description = "Strong password for human accounts"
}

secret_type hex_key {
    length = 64
    charset = hex
    description = "Hex-encoded symmetric key"
}

secret_type db_credential {
    length = 24
    charset = alphanumeric
    exclude = "0O1lI"       # avoid ambiguous characters
    description = "Database credential safe for connection strings"
}

secret_type tls_key {
    algorithm = ed25519
    description = "TLS private key"
}

secret_type rsa_key {
    algorithm = rsa
    bits = 4096
    description = "RSA private key"
}

secret_type uuid_secret {
    format = uuid_v4
    description = "Random UUID"
}
```

### `secret_type` Properties

| Property      | Type             | Default           | Description                                                                   |
|---------------|------------------|--------------------|-------------------------------------------------------------------------------|
| `length`      | `int`            | `32`               | Length in characters (for string-based secrets) or bytes (for raw keys).       |
| `charset`     | enum or `string` | `alphanumeric`     | Character set for generation. See charset table below.                        |
| `exclude`     | `string`         | `""`               | Characters to exclude from the charset.                                       |
| `algorithm`   | enum             | (none)             | Cryptographic algorithm for key generation: `ed25519`, `ecdsa_p256`, `ecdsa_p384`, `rsa`, `x25519`. Mutually exclusive with `charset`/`length`. |
| `bits`        | `int`            | (none)             | Key size for `rsa`. Ignored for other algorithms.                             |
| `format`      | enum             | (none)             | Output format: `uuid_v4`, `hex`, `base64`, `base64url`. Mutually exclusive with `charset`. |
| `prefix`      | `string`         | `""`               | Static prefix prepended to the generated value. Useful for tokens with a type prefix. |
| `description` | `string`         | `""`               | Human-readable description.                                                   |

### Charset Values

| Charset                 | Characters                                                       |
|-------------------------|------------------------------------------------------------------|
| `alpha`                 | `a-zA-Z`                                                        |
| `alphanumeric`          | `a-zA-Z0-9`                                                     |
| `alphanumeric_special`  | `a-zA-Z0-9!@#$%^&*()-_=+[]{}` (printable ASCII subset)          |
| `hex`                   | `0-9a-f`                                                        |
| `base64`                | `A-Za-z0-9+/=`                                                  |
| `numeric`               | `0-9`                                                            |
| `ascii_printable`       | All printable ASCII (0x20–0x7E)                                  |
| (custom string)         | Literal characters to use. `charset = "abc123"` uses only those. |

---

## Using Secret Variables

### Standalone Declaration

```
var strong_pass admin_password
var hex_key luks_system_key
var secret api_token
```

### Inline Declaration

Secret variables can be declared inline wherever a value is expected:

```
user kit {
    group = kit
    password {
        hash = var strong_pass kit_password
    }
}

user deploy {
    group = ops
    password {
        hash = var db_credential deploy_password
    }
}
```

The inline form declares the secret variable and uses its resolved value in a single expression. The compiler:
1. Checks if the secret already exists in the keeper. If so, uses the existing value.
2. If not, generates a value using the type's profile and stores it in the keeper.
3. Substitutes the resolved value into the consuming property.

### Referencing Existing Secrets

A secret declared anywhere in the source tree is referenceable via the standard pointer system:

```
var hex_key luks_key

disk /dev/sda {
    luks2 system {
        passphrase = secrets.luks_key       # reference to the secret variable
    }
}
```

The `secrets.` prefix is the namespace for all secret variables, analogous to `storage.`, `var.`, `class.`, etc.

### Explicit Scope

By default, the compiler infers scope from usage context: a secret used in a LUKS passphrase or `/etc/shadow` hash is build-time; a secret written to a runtime-populated credentials directory is runtime. The operator can override:

```
var secret build tls_signing_key             # explicitly build-time
var secret runtime db_session_token           # explicitly runtime
```

### Explicit Keeper Assignment

When multiple keepers are configured, a secret can be assigned to a specific keeper:

```
var secret(keeper = vault "production") service_token
var secret(keeper = systemd_creds "tpm") db_credential
```

When no keeper is specified, the secret routes to the default keeper for its scope.

---

## The `@@` Redaction Operator

The `@@` operator marks code that the compiler fully processes — parses, resolves, compiles, emits into build artifacts — but strips from the Ironclad source copy deployed onto the target system. The parser does not ignore `@@` content. It executes it completely. It then ensures that no trace of the `@@`-wrapped source text persists in any readable form in the `.ic` files on the deployed system.

This is a general-purpose language feature, not limited to secrets. Any code or values the operator writes between `@@` markers will function normally during compilation but will not be remembered on the deployed system.

### Inline Redaction

```
user kit {
    group = kit
    password {
        hash = @@var strong_pass kit_password@@
    }
}
```

The compiler sees `var strong_pass kit_password`, generates or retrieves the value, hashes it, and injects the hash into `/etc/shadow`. The password works — the system has it. But the Ironclad source on the deployed system shows:

```
user kit {
    group = kit
    password {
        hash =
    }
}
```

The declaration of how the password was created, what secret type generated it, and what keeper held it is gone. Someone reading the `.ic` files on the running system cannot reconstruct the secret or its provenance from source alone.

### Block Redaction

```
@@
var strong_pass admin_password
var hex_key luks_system_key
var db_credential deploy_password
@@
```

Everything between the opening `@@` and closing `@@` is compiled and executed but stripped from the on-system source. The lines are removed entirely — no placeholder, no line count, no trace.

### Multiline Block with Context

```
@@
user bootstrap_admin {
    system = true
    uid = 999
    group = wheel
    home = /dev/null
    shell = /bin/bash
    password {
        hash = var strong_pass bootstrap_temp_pass
    }
}
@@
```

The entire user declaration is compiled into the system (the account is created, the password is set) but the on-system source shows no evidence that this user was declared in code. This is useful for temporary install-time accounts that should not be discoverable by reading the declaration.

### Redaction Inside Functions and Classes

When `@@` appears inside a `func` or `class` body, the redaction markers are **preserved** in the declaration. Redaction is applied when the function is `embed`-ed or the class is `apply`-ed into a concrete system. This ensures reusable code retains its redaction intent across all instantiations.

```
class secured_postgres {
    group postgres { gid = 26; system = true }

    user postgres {
        system = true
        uid = 26
        group = postgres
        home = /var/lib/pgsql
        shell = /bin/bash
        password {
            hash = @@var db_credential pg_superuser_pass@@
        }
    }
}

# When applied:
system db01 {
    apply secured_postgres
    # The password hash is generated and injected.
    # The on-system source for the resolved class shows: hash =
}
```

The class itself is stored with `@@` markers intact — it can be shared, inherited, and reused. Redaction happens at the point of concrete system compilation, not at class definition time.

### Auto-Redaction via Function and Class Parameters

When `@@` wraps a parameter in a function or class signature, it becomes part of the interface. The parser does not remove the `@@` markers from the definition — it preserves them. Any value passed to that parameter is automatically redacted on the deployed system. The caller does not need to wrap their argument in `@@`.

#### Function Parameters

```
func setup_service_account(name: string, @@password: string@@) {
    user ${name} {
        system = true
        group = ${name}
        shell = /sbin/nologin
        password {
            hash = ${password}
        }
    }
}
```

When called:

```
embed func.setup_service_account("myapp", var strong_pass myapp_password)
```

The caller passes the password without `@@`. The function's signature declares that the `password` parameter is always redacted. On the deployed system:
- The function definition retains `@@password: string@@` in its signature — the markers stay.
- The argument `var strong_pass myapp_password` is stripped — the content between the markers disappears.
- The compiled system has the account with a working password in `/etc/shadow`.

This lets function authors enforce redaction policy on sensitive inputs. Anyone calling the function gets automatic redaction without needing to remember to wrap their arguments.

#### Class Parameters

The same pattern works on parameterized classes:

```
class db_server(name: string, @@superuser_pass: string@@, @@replication_pass: string@@) {
    group postgres { gid = 26; system = true }

    user postgres {
        system = true
        uid = 26
        group = postgres
        home = /var/lib/pgsql
        shell = /bin/bash
        password {
            hash = ${superuser_pass}
        }
    }

    init systemd {
        service postgresql {
            user = postgres
            group = postgres
            credentials {
                REPLICATION_PASS = ${replication_pass}
            }
            install { wanted_by = [multi-user.target] }
        }
    }
}
```

When applied:

```
system db01 {
    apply db_server("primary", var db_pass pg_super, var db_pass pg_repl)
}
```

The caller writes clean code with no `@@`. The class interface guarantees both password arguments are redacted on the deployed system. The on-system `.ic` files show the class definition with `@@` markers intact (so the interface is inspectable) but the actual argument values are stripped.

#### Standard Library Implications

This is how the standard library enforces redaction policy at scale. A `std.hardened_rhel_base` class can mark its LUKS passphrase and root password parameters as `@@`-wrapped. Every system that inherits from it gets automatic redaction of those values without the operator doing anything special:

```
# Standard library class (shipped with Ironclad)
class std.hardened_rhel_base(@@luks_pass: string@@, @@root_hash: string@@) {
    # ... storage, users, SELinux, etc.
}

# Operator just uses it — redaction is automatic
system web01 {
    apply std.hardened_rhel_base(
        var hex_key my_luks_pass,
        var strong_pass my_root_hash
    )
}
```

### Redaction Inside Conditionals and Loops

`@@` works inside any block:

```
if ${var.env} == "production" {
    @@var strong_pass prod_db_pass@@

    init systemd {
        service myapp {
            credentials {
                DB_PASS = @@secrets.prod_db_pass@@
            }
        }
    }
}

for host in ${var.cluster_hosts} {
    @@var hex_key ${host.name}_node_key@@
}
```

### What `@@` Does NOT Do

- **`@@` does not prevent values from reaching their destinations.** A password still ends up in `/etc/shadow`. A LUKS key still ends up in the LUKS header. A config file credential still ends up in the config file. The system needs these values to function. `@@` prevents the Ironclad *source* from revealing how they got there.
- **`@@` does not encrypt anything.** It strips source text from the deployed `.ic` files. Protecting the compiled artifacts (`/etc/shadow`, LUKS headers, config files) is the responsibility of file permissions, SELinux labels, and the secret keeper.
- **`@@` does not affect the intermediate manifest.** The manifest stores content hashes of all declared files, including those with redacted inputs. The runtime agent can still detect drift on secret-bearing files.
- **`@@` does not affect the secret keeper.** Secrets are stored in the keeper regardless of whether they are redacted. `@@` controls source visibility on the deployed system only.
- **`@@` is not a security boundary on its own.** An attacker with root can read `/etc/shadow` whether or not the Ironclad source mentions the password. `@@` prevents casual discovery from reading the declaration — it does not prevent forensic recovery from the live system's actual files.

### Parser Rules

- `@@` is recognized as a token only outside string literals. The string `"email@@example.com"` is not affected.
- `@@` must appear in matched pairs. An unmatched `@@` is a parse error.
- Inline `@@content@@` and block `@@\n...\n@@` are both valid. The parser distinguishes them by the presence of a newline immediately after the opening `@@`.
- Nested `@@` is not permitted. `@@ foo @@bar@@ baz @@` is a parse error.
- `@@` inside comments (`# @@ ... @@`) is ignored — comments are already stripped.
- `@@` on a function or class parameter wraps the parameter declaration, not just the name: `@@password: string@@`. The markers are preserved in the definition and apply automatically to any argument passed to that parameter at every call site.

---

## Audit Storage

Some environments require that secret values be recoverable for auditing, incident response, or disaster recovery — but not from the running system. The `audit_store` block declares where sealed copies of build-time secret values are stored.

```
secrets {
    audit_store {
        path = /opt/ironclad/audit
        encryption = age
        recipients = [
            "age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p",   # security team
        ]
        selinux = system_u:object_r:classified_audit_t:s12:c0.c255
    }
}
```

### Audit Store Properties

| Property      | Type             | Default  | Description                                                                  |
|---------------|------------------|----------|------------------------------------------------------------------------------|
| `path`        | `path`           | required | Directory where audit bundles are written. On the build host, not the target.|
| `encryption`  | enum             | required | Encryption method: `age`, `gpg`, `pkcs11`.                                  |
| `recipients`  | `list[string]`   | required | Public keys that can decrypt the audit bundle.                               |
| `selinux`     | context expr     | (none)   | SELinux context for the audit store path. Validated against MLS policy.       |

### Audit Store Behavior

After a successful build, the compiler emits an encrypted audit bundle at `<path>/<system_name>-<build_hash>.audit.age` (or `.gpg`, `.p11`). The bundle contains:

- Every build-time secret name and its resolved plaintext value.
- The build timestamp and source tree hash.
- The manifest hash for cross-reference.

The audit bundle is encrypted to the declared recipients. It is stored on the build host, not in the image. The operator is responsible for transferring audit bundles to secure long-term storage.

If `audit_store` is not declared, build-time secret values exist only in the keeper and in the artifacts they were injected into. No centralized recovery path exists. The compiler emits an informational note (not a warning) in this case.

---

## SELinux MLS Integration

Secrets can carry a sensitivity label that the compiler uses for MLS validation.

```
var secret(sensitivity = s8) classified_key
```

### Sensitivity Property

| Property       | Type     | Default | Description                                                                               |
|----------------|----------|---------|-------------------------------------------------------------------------------------------|
| `sensitivity`  | MLS expr | `s0`    | The minimum SELinux sensitivity level for this secret. Files receiving this secret's value must have contexts with ranges that dominate this level. |

### Validation

When a secret with `sensitivity = s8` is injected into a file, the compiler checks:

1. The file's SELinux context has a range that dominates `s8` (e.g., `s8-s15:c0.c1023` dominates `s8`).
2. The user declared in the file's SELinux context has an authorized range that dominates `s8`.
3. Any process (service) that reads this file at runtime has an SELinux type authorized for `s8` access.

If any check fails, the compiler emits a hard error:

```
ERROR: secret "classified_key" (sensitivity s8) injected into /etc/app/secret.conf
       with context system_u:object_r:app_conf_t:s0 — context range s0 does not dominate s8
```

### Audit Store and MLS

The audit store path's `selinux` label must dominate all secrets written to it. If the store receives secrets at `s12` but is labeled `s0`, the compiler emits an error.

---

## Interaction with User Declarations

The `password.hash` property in `USERS_SYNTAX.MD` accepts a secret reference:

```
secret_type strong_pass {
    length = 32
    charset = alphanumeric_special
}

user kit {
    group = kit
    password {
        hash = var strong_pass kit_password
    }
}
```

The compiler:
1. Generates (or retrieves from the keeper) the plaintext password.
2. Runs it through the appropriate password hashing algorithm (`crypt(3)` with SHA-512, or as configured).
3. Injects the hash into `/etc/shadow`.
4. Stores the plaintext in the keeper and optionally the audit store.
5. If the declaration is wrapped in `@@`, the on-system source copy has the redacted content removed.

For `hash = locked` or `hash = "literal_hash_string"`, no secret processing occurs — these pass through unchanged.

---

## Interaction with Storage Declarations

LUKS passphrases reference secrets:

```
var secret luks_key

disk /dev/sda {
    luks2 system {
        passphrase = secrets.luks_key

        clevis {
            keeper = clevis "datacenter"     # runtime: Tang-bound auto-unlock
        }
    }
}
```

**Build-time:** The compiler reads `luks_key` from the build-time keeper and passes it to `cryptsetup luksFormat` in the Kickstart configuration.

**Runtime:** The Clevis binding allows the system to unlock without the passphrase when the Tang server is reachable. The passphrase remains in the keeper for disaster recovery (Tang server down).

---

## Interaction with Service Declarations

Service credentials reference secrets:

```
var secret runtime db_conn_string

init systemd {
    service myapp {
        user = myapp
        group = myapp

        credentials {
            DB_PASSWORD = secrets.db_conn_string
        }
    }
}
```

The compiler emits the appropriate mechanism for the configured runtime keeper:
- **systemd-creds:** `LoadCredentialEncrypted=DB_PASSWORD:<credstore>/db_conn_string.cred`
- **Vault:** Vault Agent template that renders the secret to a credentials file.
- **AWS SSM:** A helper service that fetches from SSM and writes to `/run/credentials/myapp.service/DB_PASSWORD`.

The service's SELinux type must have permission to read the credentials path. The compiler validates this against the SELinux declaration.

---

## Interaction with Firewall Declarations

Firewall rules may reference secrets for WireGuard or IPsec pre-shared keys (future specification). The pattern is the same: a secret reference in the relevant property, routed through the keeper.

---

## Secret Generation Lifecycle

When the compiler encounters a generated secret (`var strong_pass db_pass` or `var secret api_token: generated`):

1. **Check keeper.** Query the configured keeper for a value stored under the secret's name.
2. **If found:** Use the existing value. Generation is idempotent — a secret is generated once and reused on subsequent builds.
3. **If not found:** Generate a value using the type's profile (or the default profile for `var secret`).
4. **Store in keeper.** Write the generated value to the keeper.
5. **Store in audit store.** If an audit store is configured, include the value in the encrypted audit bundle.
6. **Resolve references.** Substitute the value wherever the secret is referenced.

### Default Generation Profile

When `var secret foo: generated` is used without a custom type, the compiler uses:

```
# Implicit default
length = 32
charset = alphanumeric
```

This produces a 32-character alphanumeric string, which is safe for most applications. The operator should define a custom `secret_type` when the consuming application has specific format requirements.

---

## Compiler Validation Summary

### Errors (halt compilation)

- Build-time secret not found in keeper (file missing from directory, key missing from SOPS file, Vault path does not exist)
- `secret_keeper` configuration is invalid (missing required property, unreachable Vault address at build time)
- No default keeper configured and a secret does not specify a keeper
- `secret_type` declares both `charset`/`length` and `algorithm` (mutually exclusive)
- `secret_type` declares both `charset` and `format` (mutually exclusive)
- Secret with MLS `sensitivity` injected into a file with an SELinux context that does not dominate the sensitivity level
- Audit store `selinux` label does not dominate the sensitivity of a secret being written to it
- Two `secret_type` declarations with the same name
- Duplicate secret variable names within the same scope
- `bits` declared on a non-RSA algorithm
- `rsa` algorithm without `bits`

### Warnings

- Build-time keeper `path` is not on an encrypted filesystem
- Build-time keeper uses `directory` type (no encryption at rest — operator responsibility)
- No `audit_store` declared (informational — no centralized recovery path)
- Generated secret uses the default profile (consider defining a `secret_type` for explicit control)
- Runtime secret destination path has world-readable permissions
- Secret variable declared but never referenced
- `ssh-rsa` key generated (deprecated algorithm — prefer `ed25519`)

---

## Security Floor Enforcement

- **Floor: baseline** — At least one `secret_keeper` must be configured. All password hashes must use `secret` or a custom `secret_type` — literal hash strings in source are promoted from acceptable to warning.
- **Floor: high** — Baseline plus: `audit_store` must be configured. Build-time keeper must not be `directory` type (encryption at rest must be explicit — `age`, `sops`, `vault`, `pkcs11`). All generated secrets must use a custom `secret_type` (no default profile). MLS `sensitivity` must be declared on all secrets in systems with `selinux.policy = mls`.
- **Floor: maximum** — High plus: all keepers must use authenticated encryption (no plaintext directory, no GPG without a hardware key). PKCS#11 or Vault required. Audit store must use `pkcs11` encryption. All secrets must have explicit `sensitivity` labels.

---

## Reserved Keywords

`secret`, `secret_type`, `secret_keeper`, `audit_store`, `generated`, `sensitivity`, `keeper`, `scope`, `build`, `runtime`, `both`, `length`, `charset`, `exclude`, `algorithm`, `bits`, `format`, `prefix`, `recipients`, `encryption`, `pin_file`, `token_label`, `module`, `slot`, `auth_method`, `mount`, `namespace`, `base_path`, `pin`, `tang`, `tpm2`, `sss`, `threshold`, `pins`, `vault_path`, `template_dir`, `auto_auth`, `agent_config_path`, `credentials_dir`, `pcr_mask`, `thumbprint`, `delivery_dir`, `read_command`, `write_command`, `generate_command`, `service_unit`, `build_path`, `identity`, `key_type`, `locked`

The `@@` redaction operator is a token, not a keyword. It is reserved at the lexer level.

---

## Grammar Summary (Informative)

```
secret_keeper_decl  = "secret_keeper" keeper_type string "{" keeper_body "}"
keeper_type         = "directory" | "age" | "sops" | "vault" | "systemd_creds"
                    | "clevis" | "aws_ssm" | "aws_secrets_manager"
                    | "gcp_secret_manager" | "azure_keyvault" | "pkcs11" | "custom"
keeper_body         = (property | tls_block | runtime_block | build_block
                    | tang_block | tpm2_block | sss_block)*

secret_type_decl    = "secret_type" identifier "{" secret_type_body "}"
secret_type_body    = (property)*

secret_var_decl     = "var" "secret" ("build" | "runtime")?
                      ("(" keeper_ref ")")? identifier (":" "generated")?
                    | "var" "secret" ("(" "sensitivity" "=" mls_expr ")")?
                      identifier (":" "generated")?
                    | "var" identifier identifier                   # user-defined type

secret_ref          = "secrets" "." identifier

audit_store_decl    = "audit_store" "{" audit_store_body "}"
audit_store_body    = (property)*

secrets_block       = "secrets" "{" secrets_body "}"
secrets_body        = (secret_keeper_decl | secret_type_decl | secret_var_decl
                    | audit_store_decl)*

redaction_inline    = "@@" expression "@@"
redaction_block     = "@@" NEWLINE (any_declaration)* "@@"
redaction_param     = "@@" param "@@"                       # in func/class signatures
```

---

## Full Example: Desktop / Small Team

```
system workstation01 {
    secrets {
        secret_keeper age "local" {
            default = true
            scope = build
            path = "secrets/"
            identity = "/home/kit/.age/key.txt"
            recipients = [
                "age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p",
            ]
        }

        secret_type strong_pass {
            length = 32
            charset = alphanumeric_special
        }

        secret_type luks_passphrase {
            length = 64
            charset = hex
        }

        var luks_passphrase system_disk_key

        audit_store {
            path = "/opt/ironclad/audit"
            encryption = age
            recipients = [
                "age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p",
            ]
        }
    }

    user kit {
        uid = 1000
        group = kit
        groups = [wheel]
        home = /home/kit
        shell = /bin/bash
        selinux_user = staff_u

        password {
            hash = var strong_pass kit_password
        }

        ssh_keys = [
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIG... kit@phone",
        ]
    }

    group kit { gid = 1000 }
    group wheel { gid = 10; system = true; members = [kit] }

    user root {
        password { hash = locked }
    }

    disk /dev/nvme0n1 {
        label = gpt

        fat32 efi {
            size = 1G
            mount = /boot/efi
        }

        luks2 system {
            passphrase = secrets.system_disk_key

            lvm vg0 {
                xfs root {
                    size = 50G
                    mount = /
                }
                xfs home {
                    size = remaining
                    mount = /home
                }
            }
        }
    }
}
```

---

## Full Example: Datacenter with Vault and Clevis

```
system db_primary {
    secrets {
        secret_keeper vault "production" {
            default = true
            scope = both
            address = "https://vault.corp.internal:8200"
            auth_method = approle
            role_id_file = "/opt/ironclad/vault-role-id"
            secret_id_file = "/opt/ironclad/vault-secret-id"
            mount = "secret"
            namespace = "infrastructure"
            base_path = "ironclad/db_primary"
            tls {
                ca_cert = "/etc/pki/tls/certs/corp-ca.crt"
            }
            runtime {
                agent_config_path = /etc/vault-agent.d
                auto_auth {
                    method = approle
                    role_id_file = "/run/vault/role-id"
                    secret_id_file = "/run/vault/secret-id"
                }
            }
        }

        secret_keeper clevis "rack_tang" {
            scope = runtime
            pin = sss
            sss {
                threshold = 2
                pins = [
                    { pin = tang; tang { url = "http://tang1.rack04:8080"; thumbprint = "aaa..." } },
                    { pin = tang; tang { url = "http://tang2.rack04:8080"; thumbprint = "bbb..." } },
                    { pin = tpm2; tpm2 { pcr_mask = "7+11+14" } },
                ]
            }
        }

        secret_keeper systemd_creds "local_tpm" {
            scope = runtime
            tpm2 = true
            pcr_mask = "7+11+14"
        }

        secret_type db_pass {
            length = 48
            charset = alphanumeric
            exclude = "0O1lI"
        }

        secret_type hex_key {
            length = 64
            charset = hex
        }

        var hex_key luks_key
        var db_pass replication_password
        var db_pass app_readonly_password

        audit_store {
            path = "/opt/ironclad/audit"
            encryption = age
            recipients = [
                "age1abc...security-team-key",
                "age1def...ciso-recovery-key",
            ]
            selinux = system_u:object_r:audit_secret_t:s12:c0.c255
        }
    }

    selinux {
        mode = enforcing
        policy = mls
        max_sensitivity = 15
        max_category = 1023

        user system_u {
            roles = [system_r]
            range = s0-s15:c0.c1023
        }

        user sysadm_u {
            roles = [sysadm_r]
            range = s0-s15:c0.c1023
        }
    }

    group postgres { gid = 26; system = true }

    user postgres {
        system = true
        uid = 26
        group = postgres
        home = /var/lib/pgsql
        shell = /bin/bash
    }

    disk /dev/sda {
        label = gpt

        fat32 efi {
            size = 1G
            mount = /boot/efi
        }

        luks2 system {
            passphrase = secrets.luks_key

            clevis {
                keeper = clevis "rack_tang"
            }

            lvm vg0 {
                xfs root {
                    size = 100G
                    mount = /
                }
                xfs pgdata {
                    size = 500G
                    mount = /var/lib/pgsql/data
                }
            }
        }
    }

    init systemd {
        service postgresql {
            type = notify
            user = postgres
            group = postgres
            exec_start = "/usr/bin/postmaster -D /var/lib/pgsql/data"
            selinux = postgresql_t

            credentials {
                REPLICATION_PASSWORD = secrets.replication_password
                APP_READONLY_PASSWORD = secrets.app_readonly_password
            }

            install { wanted_by = [multi-user.target] }
        }
    }
}
```

---

## Full Example: Secret Types in a Reusable Class

```
# --- classes/postgres.ic ---

class postgres_secured {
    secrets {
        secret_type db_pass {
            length = 48
            charset = alphanumeric
            exclude = "0O1lI"
        }

        var db_pass pg_superuser_password
        var db_pass pg_replication_password
    }

    group postgres { gid = 26; system = true }

    user postgres {
        system = true
        uid = 26
        group = postgres
        home = /var/lib/pgsql
        shell = /bin/bash
    }

    init systemd {
        service postgresql {
            type = notify
            user = postgres
            group = postgres
            exec_start = "/usr/bin/postmaster -D /var/lib/pgsql/data"
            selinux = postgresql_t

            credentials {
                SUPERUSER_PASS = secrets.pg_superuser_password
                REPLICATION_PASS = secrets.pg_replication_password
            }

            install { wanted_by = [multi-user.target] }
        }
    }
}

# --- system.ic ---

import "classes/postgres.ic" { postgres_secured }

system db01 {
    secrets {
        # Keeper configured at system level — class secrets route here
        secret_keeper vault "production" {
            default = true
            scope = both
            address = "https://vault.internal:8200"
            auth_method = kubernetes
            kubernetes_role = "ironclad-db01"
            mount = "secret"
            base_path = "ironclad/db01"
        }
    }

    apply postgres_secured
    # pg_superuser_password and pg_replication_password are generated
    # and stored in Vault at ironclad/db01/pg_superuser_password, etc.
}
```

---

## What This Document Does Not Cover

- **Secret rotation** — Automated rotation policies, key rollover, and certificate renewal (future specification — expected to integrate with keeper-native rotation features)
- **Certificate Authority** — Internal CA management, cert issuance, chain validation (future specification)
- **WireGuard / IPsec keys** — Tunnel key management using the secret system (future specification, cross-references FIREWALL_SYNTAX)
- **sudo / privilege escalation** — Using secrets for sudo password requirements (future specification, cross-references USERS_SYNTAX)
- **Multi-system secret sharing** — Topology-level secret distribution where multiple systems share a secret (future specification, cross-references topology declarations)
- **Runtime secret rotation hooks** — Triggering service restarts when a runtime secret changes (future specification, cross-references INIT_SYNTAX)
