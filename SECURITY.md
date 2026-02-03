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
