---
name: secure-code-review
description: Review source code you are authorized to audit for security defects, driven by OWASP and CWE classes. Find injection, auth, crypto, deserialization, access-control and secrets issues, each with a concrete fix. Defensive, remediation-focused.
---

# Secure code review

A structured read of code **you are authorized to audit** (your own, or an engagement's source) to find security defects before they ship. Guidance only; grants no tools or permissions.

## Where to look, by class

Walk the code against the common classes rather than reading top to bottom:

- **Injection** (CWE-89/78/79/94): untrusted input reaching a query, shell, template, or interpreter without parameterization or escaping.
- **Broken access control** (CWE-285/639): missing authorization checks, object references the caller shouldn't reach, trust in client-supplied role or id.
- **Authentication & session** (CWE-287/384): weak password handling, predictable tokens, missing expiry, fixation.
- **Cryptographic failures** (CWE-327/330): home-grown crypto, weak algorithms, static IVs, bad randomness, secrets in code.
- **Deserialization & parsing** (CWE-502): untrusted data into object construction or an unsafe parser.
- **SSRF & path handling** (CWE-918/22): server fetching attacker-controlled URLs, path traversal in file access.
- **Secrets & config** (CWE-798): credentials, keys or tokens committed to the repo or logged.
- **Memory safety** (CWE-119/416) in native code: bounds, use-after-free, integer overflow.

## Method

1. **Map the trust boundaries.** Where does external input enter, and what privilege does it reach? Review those paths first.
2. **Follow the data, not the function.** Trace a tainted value from entry to a dangerous sink.
3. **Check the invariant at the boundary.** Is input validated/encoded at the edge, or deep inside where it is easy to miss one path?
4. **One finding, one fix.** Each issue gets a CWE class, the vulnerable location, why it is exploitable, and a concrete remediation.
5. **Sweep for siblings.** A pattern in one place is usually in several.

## Discipline

- Defensive output: findings and fixes for the owner, routed to `vuln-remediation`.
- Confirm exploitability where you safely can (via `zero-day-triage`); mark the rest as suspected.
- Record findings and their reach in `vuln-episodic-memory`.
- Secrets found in code: report the location and that it needs rotation; do not reuse or exfiltrate the secret.

See [the review checklist](references/CHECKLIST.md).
