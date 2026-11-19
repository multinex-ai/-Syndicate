---
name: vuln-episodic-memory
description: Keep durable episodic records of vulnerabilities, findings and fixes in MemQ, and reason about their second, third and fourth-order and long-horizon effects. The team's long memory of what was found, what it touched, and what it leads to over time.
---

# Episodic vulnerability memory and higher-order effects

Vulnerabilities are not isolated events. The same flaw class recurs, one finding implies others, and a fix has its own ripples. This skill keeps a durable, episodic record of findings and fixes in MemQ and reasons forward about their effects. Guidance only; it grants no tools and does not itself prove a live connection.

## MemQ first: identity and scope

Before any read or write: check `memq_status`, discover the actual MemQ tools, and verify the authenticated account and namespace against the expected binding (see `memq-operations`). A configured connection or a tag is not proof of identity, and never a permission. If identity is unavailable, keep a local record in the workspace and persist later; do not claim recall from configuration alone. Keep this work in its correct realm; do not mix project memory.

## What an episode records

One episode per finding or fix, stored with provenance:

```
id, title, class (CWE), target, severity
discovered: when, by which role, how (triage / review / surface / advisory)
status timeline: suspected -> confirmed -> fix-proposed -> resolved (with receipts)
evidence + fix pointers (by workspace path)
components_touched: what used or depended on the vulnerable thing
```

Store concise, evidence-linked facts — not raw dumps, not secrets, not real user data. Repeated observation is not extra proof; reinforce only on verified outcomes.

## Reasoning about effects, in orders

For each finding and each fix, think outward in orders of effect, and record them:

- **First order:** the direct impact of the vulnerability or the fix itself.
- **Second order:** what else used the vulnerable component; what the fix changed downstream (a dependency bump, a behaviour or performance change, a broken integration).
- **Third order:** how those changes affect other systems, teams, or detections — a fix that shifts an interface, a mitigation that moves attacker behaviour elsewhere.
- **Fourth order and beyond:** slow, compounding consequences — technical debt created or retired, monitoring blind spots opened, patterns an adversary could learn from the sequence of changes.

See [the effects worksheet](references/EFFECTS.md) for how to hold this without runaway speculation: bound it to plausible, evidence-linked chains and mark confidence.

## Long-horizon tracking

- **Recurrence.** When a class is found once, record it so the next appearance is recognized fast. Sweep siblings now; watch for the class later.
- **Drift.** Track the external surface and component inventory over time (with `external-attack-surface`) so a newly-published advisory can be matched to what you actually run.
- **Decay.** Mark findings resolved only on receipts, and revisit "resolved" items when their component changes — a fix can regress.
- **Expiry.** Give forward-looking predictions a review date; prune ones that did not pan out, and keep the reasoning that did.

## Discipline

- Defensive record-keeping and planning only. The output is awareness and prioritization, not targeting.
- Evidence over narrative: every episode and every predicted effect links to something observed, with a confidence level.
- Private and project memory stay in their own realm; no cross-realm copy without authorization.

See [the effects worksheet](references/EFFECTS.md).
