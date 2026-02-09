# Security Policy

## Security Model

k-Terminus uses **Tailscale as its sole authentication and authorization mechanism**. Being on the same Tailscale network is the trust boundary.

### Authentication Flow

```
Agent connects to Orchestrator
         │
         ▼
┌─────────────────────────────────┐
│ Is connection from localhost    │
│ (127.0.0.1)?                    │
└─────────────────────────────────┘
         │
    ┌────┴────┐
    │         │
   Yes        No
    │         │
    ▼         ▼
┌────────┐  ┌─────────────────────┐
│ ACCEPT │  │ Is peer IP in our   │
│        │  │ Tailscale network?  │
└────────┘  └─────────────────────┘
                    │
               ┌────┴────┐
               │         │
              Yes        No
               │         │
               ▼         ▼
          ┌────────┐  ┌────────┐
          │ ACCEPT │  │ REJECT │
          └────────┘  └────────┘
```

### Security Layers

| Layer | Mechanism | Purpose |
|-------|-----------|---------|
| **Network** | Tailscale WireGuard | Encrypted tunnel between all devices |
| **Identity** | Tailscale device verification | Peer must be in same tailnet |
| **Transport** | SSH protocol | Additional encryption layer |
| **Session** | Per-session isolation | Each PTY runs in separate process |

### Trust Model

**Tailscale Authentication:**
- You authenticated to Tailscale via SSO (Google, GitHub, Microsoft, etc.)
- Your devices are verified members of your tailnet
- The orchestrator queries `tailscale status --json` to verify peer IPs
- Same tailnet = same identity = trusted

**Loopback Exception:**
- Connections from 127.0.0.1 (localhost) are always accepted
- This enables local CLI commands and development/testing
- Loopback connections can only originate from the same machine
- This is standard security practice for local services

### What This Means

1. **No manual key copying needed** - Tailscale handles identity
2. **No OAuth flows** - Tailscale SSO is the authentication
3. **No fallback mechanisms** - Tailscale-only reduces attack surface
4. **Revocation is simple** - Remove device from Tailscale admin console
5. **All traffic encrypted** - WireGuard + SSH (double encryption)

## Input Validation

k-Terminus validates all input to prevent denial-of-service and memory exhaustion attacks.

### Size Limits

| Input Type | Maximum Size | Enforcement |
|------------|--------------|-------------|
| Session input (terminal data) | 64 KB | Validated in IPC server before forwarding |
| Protocol frame payload | 16 MB | Enforced by 24-bit length field in frame header |
| IPC JSON requests | Reasonable limits | Validated before JSON parsing |

### Why These Limits?

- **64 KB for session input**: Sufficient for any realistic terminal interaction (paste operations, command execution). Larger inputs are likely malicious or accidental.
- **16 MB for protocol frames**: Allows for large outputs (build logs, data dumps) while preventing unbounded memory allocation.

### Validation Behavior

When input exceeds limits:
1. The request is rejected with a clear error message
2. No partial data is processed
3. The connection remains open for subsequent valid requests
4. The event is logged for monitoring

## Session Isolation

Sessions are isolated to prevent unauthorized access and ensure proper resource management.

### Ownership Model

Each session is bound to a specific machine:

```
Session {
    id: SessionId,
    machine_id: MachineId,  // Owner - immutable after creation
    shell: Option<String>,
    pid: Option<u32>,
    created_at: Instant,
}
```

### Ownership Enforcement

- **Create**: Sessions can only be created on connected machines
- **Input**: Terminal input is only forwarded to sessions owned by connected machines
- **Resize**: Window resize events verify machine ownership
- **Close**: Sessions can be closed by the owning machine or the orchestrator

### Benefits

1. **Prevents session hijacking**: Cannot send input to another machine's sessions
2. **Clean disconnect handling**: All sessions are properly cleaned up when a machine disconnects
3. **Resource accounting**: Session count is tracked per machine for limit enforcement

## Resource Limits

Configurable limits prevent resource exhaustion attacks.

### Connection Limits

```toml
[orchestrator]
max_connections = 100  # Maximum concurrent agent connections
```

- Limits total number of connected agents
- Protects orchestrator from connection flooding
- Set based on expected machine count with headroom

### Session Limits

```toml
[orchestrator]
max_sessions_per_machine = 10  # Maximum sessions per machine
```

- Limits PTY processes per machine
- Prevents runaway session creation
- Error code `SessionLimitExceeded` returned when exceeded

### Recommended Values

| Deployment | max_connections | max_sessions_per_machine |
|------------|-----------------|--------------------------|
| Personal | 10 | 5 |
| Small team | 50 | 10 |
| Enterprise | 500 | 20 |

## Threat Model

### In Scope

| Threat | Mitigation |
|--------|------------|
| Eavesdropping | WireGuard + SSH encryption |
| MITM attacks | Tailscale key exchange, SSH host keys |
| Unauthorized access | Tailnet membership verification |
| Session hijacking | Per-session process isolation |
| Credential theft | No passwords stored; key-based only |

### Out of Scope

| Threat | Reason |
|--------|--------|
| Compromised Tailscale account | Upstream dependency |
| Malicious tailnet admin | Trust boundary is the tailnet |
| Local privilege escalation | OS-level security |
| Physical access attacks | Out of scope for remote access tool |

### Attack Surface

1. **SSH Server (port 2222)** - Accepts connections, validates via Tailscale
2. **IPC Server (port 22230, localhost only)** - CLI/GUI communication
3. **Tailscale daemon** - Queried for peer verification

## Security Best Practices

### For Users

1. **Use Tailscale ACLs** - Restrict which devices can communicate
   ```json
   // In Tailscale admin console
   {
     "acls": [
       {"action": "accept", "src": ["tag:k-terminus"], "dst": ["tag:k-terminus:2222"]}
     ]
   }
   ```

2. **Monitor your tailnet** - Only devices you control should be members

3. **Review connections** - Run `k-terminus list` regularly

4. **Keep software updated** - Apply security updates promptly

5. **Use device approval** - Enable in Tailscale admin to require approval for new devices

### For Operators

1. **Limit IPC exposure** - IPC server binds to localhost only by default

2. **Rotate host keys** - Delete and regenerate if compromise suspected:
   ```bash
   rm ~/.config/k-terminus/host_key
   k-terminus serve  # Regenerates automatically
   ```

3. **Monitor logs** - Run with `-v` for connection logging

## Reporting a Vulnerability

If you discover a security vulnerability in k-Terminus:

1. **Do NOT** open a public GitHub issue
2. Use GitHub's [private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability)
3. Or email the maintainer directly
4. Include:
   - Detailed steps to reproduce
   - Potential impact assessment
   - Suggested fix if available
5. Allow reasonable time (90 days) for a fix before public disclosure

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes |

## Security Considerations for Development

When contributing:

1. **Never log secrets** - No keys, tokens, or credentials in logs
2. **Validate all input** - Especially from network sources
3. **Use safe defaults** - Fail closed, not open
4. **Review crypto usage** - Don't roll your own; use established libraries
5. **Consider timing attacks** - Use constant-time comparison for secrets

## Cryptographic Choices

### Ed25519 Only

k-Terminus exclusively uses **Ed25519** keys for all SSH operations:

- Host keys: Ed25519
- Agent keys: Ed25519
- RSA keys are **not supported**

This is enforced in `setup.rs` which explicitly passes `-t ed25519` to ssh-keygen.

### Known Advisory: RUSTSEC-2023-0071

The `rsa` crate appears in our dependency tree:
```
russh → ssh-key → rsa
```

This advisory describes a timing side-channel attack on RSA decryption (Marvin Attack). **This does not affect k-Terminus** because:

1. We never generate RSA keys
2. We never accept RSA keys for authentication
3. The RSA code paths are never executed
4. The `ssh-key` crate includes RSA support for compatibility, but we don't use it

This advisory is explicitly ignored in `.cargo/audit.toml` with documentation.

## Dependencies

Security-critical dependencies:

| Crate | Purpose | Notes |
|-------|---------|-------|
| `russh` | SSH protocol | Pure Rust implementation |
| `russh-keys` | SSH key handling | Ed25519 keys only |
| `ssh-key` | Key parsing | Multi-algorithm support (we use Ed25519 only) |
| Tailscale | Network/identity | External dependency |

All dependencies are regularly audited via `cargo audit`.

## Features Not Implemented (Security Decisions)

Certain features were intentionally excluded from k-Terminus due to security concerns:

| Feature | Decision | Reason |
|---------|----------|--------|
| **Session recording** | Not implementing | Creates persistent artifacts containing passwords, secrets, and sensitive data. Playback files are difficult to secure and represent a high-value target for attackers. |
| **Port forwarding** | Not implementing | Tailscale already provides secure port forwarding via `tailscale serve` and `tailscale funnel`. Adding our own would increase attack surface with no benefit. |
| **Full file transfer UI** | Minimal implementation | Drag-to-upload for convenience; browsing/download deferred. Standard tools (scp, rsync) work seamlessly over Tailscale. |

### Session Recording Risks

Session recording would capture:
- Passwords typed at prompts (sudo, ssh, etc.)
- API keys and tokens in environment variables
- Database credentials in config files
- Private keys displayed or edited
- Sensitive business data

Even with encryption at rest, these artifacts persist and could be:
- Accessed by other processes on the machine
- Exfiltrated if the machine is compromised
- Subpoenaed or seized
- Accidentally shared or backed up insecurely

### Port Forwarding Rationale

Tailscale provides equivalent functionality:
- `tailscale serve` exposes local services to your tailnet
- `tailscale funnel` exposes services to the internet (with auth)
- Both integrate with Tailscale's identity and ACL system

Implementing our own port forwarding would:
- Duplicate existing Tailscale functionality
- Add code paths that could have security vulnerabilities
- Create confusion about which method to use
- Not benefit from Tailscale's security review process
