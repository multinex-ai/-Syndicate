# Review checklist

Per audited component. Mark each: ok / finding / not-applicable.

## Input and output
- [ ] All external input validated or encoded at the boundary.
- [ ] Queries parameterized; no string-built SQL/commands.
- [ ] Output encoded for its sink (HTML, shell, template, log).
- [ ] File paths and URLs constrained; no traversal or SSRF.

## Identity and access
- [ ] Every privileged action checks authorization server-side.
- [ ] Object references are checked against the caller's rights.
- [ ] Authentication, session lifetime and token generation are sound.

## Data and crypto
- [ ] Standard, current crypto; good randomness; no home-grown schemes.
- [ ] No secrets in source, config-in-repo, or logs.
- [ ] Sensitive data minimized and protected at rest and in transit.

## Handling
- [ ] Untrusted data is not deserialized into objects unsafely.
- [ ] Errors do not leak secrets or internal detail.
- [ ] Native code: bounds, lifetimes, integer overflow checked.

## Each finding
- [ ] CWE class, location, exploitability note, concrete fix, sibling sweep.
