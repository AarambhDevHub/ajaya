# Security Policy

Ajaya is a web framework — security vulnerabilities in it can affect every
application built on top of it. We take security seriously and appreciate
responsible disclosure from the community.

---

## Supported Versions

Only the latest published version of each crate receives security fixes.
We do not backport patches to older versions unless the severity is critical
and the upgrade path is blocked.

| Version    | Supported          |
|------------|--------------------|
| Latest     | ✅ Active support  |
| Older      | ❌ Upgrade to latest |

Once Ajaya reaches `1.0.0`, an LTS policy will be defined here.

---

## Scope

The following are **in scope** for this security policy:

- All crates in the `ajaya` workspace:
  `ajaya`, `ajaya-core`, `ajaya-router`, `ajaya-hyper`, `ajaya-extract`,
  `ajaya-middleware`, `ajaya-ws`, `ajaya-sse`, `ajaya-static`, `ajaya-tls`,
  `ajaya-macros`, `ajaya-test`
- Security issues caused by incorrect documentation that leads users to
  write insecure code
- Vulnerabilities introduced via our direct dependencies that we can
  mitigate at the framework level

The following are **out of scope**:

- Vulnerabilities in your application code (we can help, but it's not a
  framework bug)
- Vulnerabilities in transitive dependencies that have no available fix
  (report to those upstream maintainers directly)
- Issues requiring physical access to the server
- Social engineering attacks

---

## Reporting a Vulnerability

**Please do NOT open a public GitHub issue for security vulnerabilities.**
Public disclosure before a patch is ready puts every Ajaya user at risk.

### How to Report

**Option 1 — GitHub Private Advisory (Preferred)**

1. Go to the [Ajaya Security Advisories](https://github.com/aarambh-darshan/ajaya/security/advisories/new)
2. Click **"Report a vulnerability"**
3. Fill in the details — see the template below

**Option 2 — Discord (for sensitive discussion)**

Join the [Aarambh Dev Hub Discord](https://discord.gg/HDth6PfCnp) and send a direct
message to the maintainer. Do not post in public channels.

---

## What to Include in Your Report

Please provide as much of the following as possible:

```
Vulnerability Type:
  (e.g. SQL injection, path traversal, denial of service, memory unsafety, etc.)

Affected Crate(s):
  (e.g. ajaya-router 0.1.2)

Affected Component:
  (e.g. Path extractor, ServeDir, WebSocket upgrade)

Description:
  A clear description of the vulnerability and its potential impact.

Steps to Reproduce:
  1. ...
  2. ...
  3. ...

Proof of Concept (if available):
  Minimal Rust code or curl commands that demonstrate the issue.

Expected Behavior:
  What should happen.

Actual Behavior:
  What actually happens.

Suggested Fix (optional):
  If you have ideas on how to fix it.

Environment:
  - Ajaya version:
  - Rust version (rustc --version):
  - OS:
```

---

## Our Response Process

| Timeline | Action |
|----------|--------|
| **Within 48 hours** | Acknowledge receipt of your report |
| **Within 7 days** | Confirm whether the issue is valid and in scope |
| **Within 30 days** | Release a patch (or provide a timeline if more complex) |
| **After patch ships** | Coordinate public disclosure with you |

We follow **coordinated disclosure** — we will work with you on timing before
any public announcement. We will credit you in the release notes and security
advisory unless you prefer to remain anonymous.

---

## Severity Classification

We use the following classification to prioritize fixes:

| Severity | Description | Examples |
|----------|-------------|---------|
| **Critical** | Remote code execution, arbitrary file read/write, authentication bypass | Path traversal in `ServeDir`, memory corruption in parser |
| **High** | Denial of service, significant data exposure, privilege escalation | Unbounded memory growth, header injection |
| **Medium** | Limited impact exploits, requires specific configuration | CORS misconfiguration bypass, request smuggling |
| **Low** | Minimal impact, theoretical attacks | Information leakage in error messages |

Critical and High severity issues will receive patches within 7 days where
possible.

---

## Security Best Practices for Ajaya Users

While using Ajaya, we recommend:

- **Always set a body size limit:**
  ```rust
  .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024)) // 10MB
  ```

- **Always set a request timeout:**
  ```rust
  .layer(TimeoutLayer::new(Duration::from_secs(30)))
  ```

- **Use `CatchPanicLayer` in production:**
  ```rust
  .layer(CatchPanicLayer::new())
  ```

- **Sanitize path parameters before using in filesystem operations.**
  Never pass `Path<String>` directly to `fs::read()` or similar.

- **Enable `SensitiveHeadersLayer`** to prevent tokens leaking into logs:
  ```rust
  .layer(SensitiveHeadersLayer::new([AUTHORIZATION, COOKIE]))
  ```

- **Use `PrivateCookieJar`** for session data, not plain `CookieJar`.

- **Keep Ajaya and all dependencies up to date.** Run:
  ```bash
  cargo update
  cargo audit  # requires cargo-audit
  ```

---

## Known Security Advisories

Security advisories will be published at:
[https://github.com/aarambh-darshan/ajaya/security/advisories](https://github.com/aarambh-darshan/ajaya/security/advisories)

No advisories have been issued yet.

---

## Hall of Fame

We gratefully acknowledge security researchers who responsibly disclose
vulnerabilities to us. Contributors will be listed here with their permission.

*No entries yet — Ajaya is young. Be the first!*

---

*Ajaya (अजय) — Unconquerable. Built by Aarambh Dev Hub.*