# Safe reproduction

The goal is a receipt that proves the flaw and a test that fails until it is fixed — not a weapon.

## Rules

- **Isolated only.** A lab VM, container or local build that is disconnected from production and from any third party. Snapshot before, restore after.
- **Minimal trigger.** Reduce to the smallest input or step sequence that still triggers the flaw. Smaller is easier to understand and to fix.
- **Non-destructive proof.** Demonstrate control or disclosure with a benign marker (a canary file, a harmless computed value), never real data, never damage.
- **No persistence.** Do not install implants, backdoors or accounts. Remove anything the repro created.
- **Capture, then stop.** Once you have the receipt, stop escalating. You do not need full exploitation to justify a fix.

## What to keep

- The minimal trigger input or script.
- The observed effect (crash log, wrong output, bypassed check), by workspace path.
- A regression test that fails on the vulnerable build and passes once fixed.

## What not to keep

- A reliable, packaged, redistributable exploit.
- Any real data encountered during reproduction.
- Anything that would let someone else run the attack without doing the analysis.
