---
name: zero-day-triage
description: Detect, safely reproduce and rate previously-unknown vulnerabilities in code and systems you are authorized to test. Defensive focus — find it first so it can be fixed. Reproduce only in a lab; never weaponize or retain exploits against third parties.
---

# Zero-day detection and triage

A zero-day is a flaw nobody has a patch for yet. This skill is for finding one in **your own or authorized** code and systems before an attacker does, reproducing it safely, and handing it to remediation. It is guidance only and grants no tools or permissions.

## Authorization

Only against targets named in a signed scope (see `authorized-pentest`). Analysis of your own source or a lab build is always fair game; touching a running system is not, unless scope allows it. If scope is missing, do static analysis of code you own and stop at anything live.

## How to look

1. **Pick the surface.** Where does untrusted input meet privileged action? Parsers, deserializers, file and path handling, auth and session logic, memory management in native code, template and query construction, update and signing paths, inter-process and network message handling.
2. **Reason about invariants.** For each surface, what must always be true for it to be safe (bounds, type, ownership, state order, trust level)? A vulnerability is where an input can break one.
3. **Combine signals.** Static review, fuzzing an isolated build, differential testing against a reference, and diffing versions for silently-fixed bugs. No single signal is proof.
4. **Reproduce in a lab.** Build the smallest input or sequence that triggers the flaw, in an isolated environment. Capture the crash, the wrong result, or the policy bypass as a receipt.
5. **Rate it.** Use the dimensions in [the triage rubric](references/TRIAGE.md): what is reachable, what an attacker needs, what they gain, and how hard it is to exploit reliably.

## Discipline

- Confirm in a lab, never on production or a third party.
- The artifact you keep is a **regression test and a report**, not a packaged exploit. Keep proof minimal — enough to reproduce and fix.
- No destructive testing, no persistence, no data exfiltration.
- Report responsibly: the finding goes to the owner and to `vuln-remediation`, not outward.
- Record it in `vuln-episodic-memory` with its downstream effects, and watch for the same class elsewhere.

Hand confirmed findings to `vuln-remediation` for a fix plus a test, and to detection engineering for a way to spot exploitation.

See [the triage rubric](references/TRIAGE.md) and [safe reproduction](references/SAFE_REPRO.md).
