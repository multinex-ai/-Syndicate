# Triage rubric

Rate a confirmed finding on each axis, then combine into a severity. Record the reasoning, not just the label.

## Axes

- **Reachability** — can an attacker reach the vulnerable code at all? remote-unauthenticated > remote-authenticated > local > physical.
- **Preconditions** — what must be true first? none > a common config > a rare config > a second bug.
- **Impact** — what do they gain? full control / code execution > data disclosure > integrity change > denial of service > information leak.
- **Reliability** — how consistently does it work? deterministic > usually > rarely / race-dependent.
- **Blast radius** — one record, one tenant, or everyone? (see higher-order effects in `vuln-episodic-memory`).

## Combining

- **Critical:** remote, no preconditions, code execution or full data disclosure, reliable.
- **High:** serious impact but needs auth, a precondition, or is less reliable.
- **Medium:** limited impact, or needs significant preconditions.
- **Low:** minor leak, hard to reach, or low reliability.

State what would raise or lower the rating, so a defender can prioritise.
