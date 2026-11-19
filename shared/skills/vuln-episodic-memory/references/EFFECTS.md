# Effects worksheet

A bounded way to reason about higher-order effects without drifting into speculation.

## The chain

Start from a finding or a fix and extend one step at a time. Each step must name a concrete mechanism, not just "and then something bad."

```
order 1 (direct):     <the vulnerability's impact, or the fix's immediate change>
order 2 (downstream):  because <mechanism>, this affects <what used it>
order 3 (systemic):    which, because <mechanism>, affects <other systems/teams/detections>
order 4+ (long):       over <time>, this compounds into <debt / blind spot / pattern>
```

For each step record:

- **mechanism** — why one leads to the next (a dependency, an interface, a shared assumption).
- **confidence** — measured / likely / speculative.
- **evidence** — what observation supports it (path or link); speculative steps say so.
- **review_by** — when to check whether it happened; prune if it did not.

## Keeping it honest

- Stop extending when the next step is only "something could go wrong" with no mechanism.
- Mark speculative branches clearly; they inform watching, not action.
- A predicted effect that recurs and is confirmed graduates to a recorded pattern; one that never materializes is pruned with a note.
- Effects analysis is for prioritizing defence and anticipating fix fallout — not for planning action against anyone.
