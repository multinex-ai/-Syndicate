---
name: documentary-storycraft
description: Plot, structure, narration and edit craft for narrated documentaries built from public footage. Use when choosing a documentary's question and structure, outlining chapters, writing narration for a synthetic voice, planning shots and transitions over real footage, or reviewing a cut for story, pacing and honesty.
---

# Documentary storycraft

A documentary is an argument told as a story. The story keeps people watching; the evidence earns their trust. Lose either and the film fails. This skill covers both, for films assembled from public footage and narrated by a synthetic voice with the Praeco documentary tools (`Praeco/scripts/documentary/doc.mjs guide`).

References (read the one you need, not all):
- `references/structures.md`: seven proven documentary structures, when each fits, and their beat sheets.
- `references/narration.md`: writing for the ear and for Kokoro: sentence rhythm, numbers, names, what never to put in narration.
- `references/picture-and-cut.md`: choosing footage, shot order, cutting to narration, transitions, pacing by runtime.
- `references/honesty.md`: evidence classes, illustrative versus depicting footage, claims and hedges, the fact-check pass.

## The six decisions, in order

1. **The question.** One sentence a curious viewer would actually ask, that the film can answer with evidence. "What happened to the Aral Sea?" beats "The Aral Sea." If you cannot state the question, you do not have a film yet.
2. **The answer's shape.** Write the answer in two sentences before any outline. That answer is the spine. Every chapter either sets up the question, complicates it, or moves toward the answer. A chapter that does none of those is cut.
3. **The structure.** Pick one from `references/structures.md` and name it in `outline.structure`. Do not blend more than two.
4. **The turns.** For every beat, write the `turn`: what the viewer believes or feels at the end that they did not at the start. If the turn is "they learn more facts", it is not a turn. Turns are reversals, revelations, raised stakes, or a question answered that opens a bigger one.
5. **The evidence map.** Before writing narration, list which source supports each factual beat. Beats with no source become either `hypothetical scenario` or `editorial analysis` (and are written that way), or they are cut.
6. **The picture plan.** For each chapter, what can the viewer actually see? Public footage is limited. Write to the footage you can get, not to footage you wish existed.

## What makes the plot work

- **Open on a concrete image and an unanswered question, not a thesis.** Show one specific thing, then raise the question it implies, all in the first 20 seconds.
- **Make a promise, then keep paying it.** The opening promises an answer. Every 60 to 90 seconds give a partial payoff: a fact that surprises, a reversal, a face, a number with a scale the viewer can feel.
- **Escalate.** Each chapter should raise the stakes or deepen the mystery relative to the last. Order chapters by rising consequence, not by chronology, unless the structure is chronological.
- **One complication the viewer did not expect.** The best documentaries have a midpoint turn: the obvious explanation is incomplete, or the villain is a system, or the solution created the next problem.
- **Characters are how people care.** A real person (a named scientist, an archive photographer, a town) gives the viewer someone to follow. Only use people who are on the record in your sources, and describe them only as the sources do.
- **Give the counter-argument its strongest form.** Name what a fair critic would say and answer it with evidence, or concede it. Viewers trust a film that has argued with itself.
- **End on meaning, not summary.** The last chapter returns to the opening image or question, now seen differently. One line of narration, then let the picture hold.

## Runtime budgets

| Runtime | Chapters | Words of narration | Shots per minute |
|---|---|---|---|
| 2 to 4 min | 3 to 4 | 300 to 550 | 8 to 12 |
| 8 to 12 min | 5 to 7 | 1,100 to 1,700 | 6 to 10 |
| 15 to 25 min | 7 to 10 | 2,000 to 3,500 | 5 to 8 |

Kokoro reads about 145 to 155 words a minute at speed 0.94. Leave room: narration should cover about 80 percent of runtime. The rest is picture breathing (openings, chapter cards, pauses after a reveal).

## Handing work on

- Outline goes to `put <slug> outline`. Script to `put <slug> script`. The tools reject a file that breaks the contract and say why.
- The fact-checker reads the script against `sources.json` before narration. Narration is regenerated whenever the script changes (the receipts catch stale audio).
- Draft-stitch first (`stitch --draft`), watch the timeline in the manifest, then stitch the final.
- Nothing is published by the team. A finished film is a local draft until a person approves it.
