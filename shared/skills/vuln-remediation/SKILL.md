---
name: vuln-remediation
description: Turn a security finding into a fix with a regression test and a detection rule, and prove the fix actually closes the hole. The "prove the fix" discipline — a finding is not resolved until a replayed attack is caught or a test that failed now passes.
---

# Remediation and detection engineering

A finding is only closed when there is proof it is closed. This skill turns a confirmed vulnerability into three things: a fix, a test that fails without the fix, and a way to detect the attack if it happens again. Guidance only; grants no tools.

## From finding to closed

1. **Understand the root cause**, not just the symptom. The same class often appears in several places — fix the cause and sweep for siblings.
2. **Write the failing test first.** Using the safe reproduction from `zero-day-triage`, write a regression test that fails on the current code. This is the proof the bug exists.
3. **Fix the root cause.** Prefer the change that removes the whole class (validate at the boundary, use a safe API, enforce the invariant) over patching one path.
4. **Prove it.** The regression test now passes. Re-run the original reproduction and confirm it no longer works. Keep both receipts.
5. **Check for regressions.** The fix must not break working behaviour. Run the surrounding tests.
6. **Write a detection.** A log signature, rule, or alert that fires if someone tries this in the wild. A fix stops the known path; detection catches the next attempt.
7. **Re-map exposure.** Confirm the previously-exposed surface now reads as closed (the Purple "re-map as closed" loop with recon).

## The rule

**A vulnerability is resolved only when a replayed attack is caught, or a test that failed now passes — never because someone says it is done.** Record both the failing-before and passing-after receipts.

## Detection quality

- Detect the technique, not just the exact test string, or the detection is worthless against a small variation.
- Track false-positive rate; a noisy rule gets ignored.
- Prefer detections tied to the invariant that was broken.

## Memory

Record the fix, its tests, and its downstream effects in `vuln-episodic-memory`: what else used the vulnerable component, what the fix changed, and what to watch next. A fix can have its own second-order effects (a dependency bump, a behaviour change) — note them.

See [the fix record](references/FIX_RECORD.md).
