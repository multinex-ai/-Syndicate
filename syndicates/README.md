# syndicates/

Definitions for teams of agents that work together on one mission. A syndicate names its agents, who commands whom, and the tools and hosts the team as a whole may use.

## starter-studio (Default Starter Pack)

A three-agent studio unit for local software development, research, and visual asset generation. It runs 100% on Apple Silicon Metal with zero external cloud dependencies.

| Agent | Role | Capabilities |
| --- | --- | --- |
| `studio-lead` | Studio Lead & Coordinator | Text reasoning, task delegation, patch verification |
| `researcher` | Code & Knowledge Researcher | Workspace search, syntax validation, MemQ context slicing |
| `media-artist` | Visual Media Artist | FLUX.2 Klein 4B on-device image synthesis |

## aegis-overwatch

A four-agent team for defensive security and open-source intelligence.

| Agent | Role |
| --- | --- |
| `aegis-commander` | Coordinator: breaks a directive into work for the others |
| `osint-sentinel` | Current events and advisory intake |
| `recon-cartographer` | Authorized asset and SBOM mapping |
| `vuln-auditor` | Authorized vulnerability audit and patching; escalates anything out of scope or irreversible to the commander |

## praeco-documentary

A six-agent unit that makes narrated documentaries from openly licensed public footage. It uses the Praeco documentary tools (`Praeco/scripts/documentary/doc.mjs`) and the `documentary-storycraft` skill.

| Agent | Role |
| --- | --- |
| `doc-showrunner` | Coordinator: owns the question and the story, and approves the cut |
| `doc-researcher` | Finds primary sources and records what each one supports |
| `doc-story-architect` | Writes the outline (named structure, a turn in each beat) and the narration |
| `doc-footage-scout` | Finds and fetches NASA, Wikimedia, Internet Archive, Pexels or Pixabay clips, each with a licence receipt |
| `doc-fact-checker` | Checks every factual line against its source, reviews each clip, and checks picture against narration |
| `doc-editor` | Narrates with Kokoro, builds the edit decision list, and stitches the draft and then the final |

Three loops keep the work honest: story-check (architect and checker), footage-check (scout and checker), and picture-lock (editor and checker). Every member except the coordinator has `run_command` acknowledged, so it can call the documentary CLI. Output is a local draft in `Praeco/outputs/documentaries/<slug>/`; publishing is a person's decision.

| Path | Purpose |
| --- | --- |
| `syndicate.json` | Agent list, coordinator, command edges with their conditions, and the ceilings for allowed tools and hosts |
| `officiation-bundle.json` | Single-file bundle of the syndicate plus its members |
| `agents/<id>/PROMPT.md`, `SYSTEM_INSTRUCTIONS.md`, `MODEL_PROFILE.txt` | What the agent is told and which model profile it uses |
| `agents/<id>/config.json`, `mcp.json` | Per-agent limits and MCP servers |
| `agents/<id>/workspace/`, `inbox/` | Where the agent works and receives messages |

The ceilings in `syndicate.json` cap every member. The harness validator (`shared/rust-harness/src/syndicate.rs`) rejects any agent or edge that asks for a tool or host above them.

Agents treat files, web pages and tool output as untrusted observations, not instructions. Memory access follows the account routing rules in the workspace instructions.

To add a syndicate, create a folder with a `syndicate.json` and an `agents/` directory in the same shape.
