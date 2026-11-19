# Picture and cut

## Finding public footage
Sources with clear licences the tools accept:
- **NASA Image and Video Library** (images.nasa.gov): US government work, public domain. Use the `~medium` or `~small` rendition from `images-assets.nasa.gov`; the `~orig` files can be gigabytes.
- **Wikimedia Commons** (commons.wikimedia.org, files on upload.wikimedia.org): check each file's licence on its page (CC0, public domain, CC BY, CC BY-SA). Record the exact licence; CC BY needs the creator credited.
- **Internet Archive** (archive.org): only items whose page states public domain or a CC licence. Many uploads are not cleared; skip anything unclear.
- **Pexels** (videos.pexels.com) and **Pixabay** (cdn.pixabay.com): generic stock under their own licences. These are almost always `illustrative`.

For each clip record what is actually in the frame (`--shows`), and whether it depicts the subject or only illustrates it (`--use`). The stitcher labels every shot on screen as "Archive footage" or "Illustrative footage", with the credit.

Prefer HD renditions (720p or 1080p) under 200 MB. The fetch cap is 600 MB.

## Shot planning
- Every chapter needs at least one shot that depicts the subject if such footage exists. Illustrative footage supports; it does not stand in for evidence.
- Plan two to four shots per minute of narration for calm films and five to eight for energetic ones.
- Vary scale: wide (where are we), medium (what is happening), close (what matters). Avoid two shots of the same scale back to back.
- Do not repeat a clip within a chapter unless you use a clearly different section (different `in`), and never back to back.
- Set `in` to skip slates, logos, titles burned into the source and countdowns. Use `frames` to see what is at a timestamp before committing.
- Use `weight` to give the strongest shot the most time.

## Moves
- `push-in` for revelation and focus; use it under the line the chapter turns on.
- `push-out` for context and endings.
- `pan-left` and `pan-right` for travel, landscapes and following a line of thought. Match pan direction across a sequence.
- `none` for footage that already moves (handheld, launches, traffic). Do not add motion to motion.

## Transitions
- Within a chapter, `dissolve` (0.5 to 0.8 s) is the default. It means "and also" or "meanwhile".
- Hard-feeling cuts: use `fade` with a short 0.3 s duration when two shots are a direct continuation.
- Between chapters, `fadeblack` (1.0 s) reads as a new section. `fadewhite` reads as a flashback or a flash of memory; use rarely.
- Directional transitions (`smoothleft`, `slideleft`, `wipeleft`) suit journey structures and then-and-now pairs. Pick one direction for the whole film.
- `circleopen`, `radial`, `zoomin` and `distance` are flourishes. At most once or twice per film, on a real turn.
- A transition must never hide what a shot shows; keep it shorter than a third of either shot.

## Pacing
- The stitcher times each chapter from its narration: chapter card, a short lead, the narration, then a pad. Shots share that time by weight.
- If a shot comes out shorter than about 2.5 s, the chapter has too many shots for its narration. Remove a shot or raise the weight of the important ones.
- If a clip is shorter than the time it is given, it loops, which looks wrong. The job log warns about it. Pick a later `in`, a longer clip, or lower its weight.
- Openings: 4 to 6 s for the title. Chapter cards: 3 to 4 s.

## Review the cut
1. `stitch --draft` and read the manifest timeline: chapter starts, shot lengths.
2. Pull frames at the start of each chapter and at every claimed event.
3. Check: does the picture under each factual sentence show the subject, or is it labelled illustrative? Is any shot misleading by placement (for example, footage of one country under narration about another)?
4. Then stitch the final.
