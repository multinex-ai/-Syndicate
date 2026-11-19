# Folder map

[Home](../README.md) · [First-run setup](getting-started/README.md)

Checked November 16, 2026. The Git repository is named `local-agents`; this checkout's folder is named `Agents`. Commands use the repository root.

| Area | Location | Contents |
| --- | --- | --- |
| Onboarding | `README.md`, `docs/getting-started/` | First-run instructions |
| Usage | `docs/guides/` | Chat, client connections and commands |
| Reference | Remaining `docs/` files | Design history and dated qualification notes |
| Launchers | `bin/`, root `harness`, `shared/bin/` | Existing command entry points |
| Tooling | `scripts/` | Installation and report builders |
| Configuration | `agents/`, `syndicates/`, `models/` | Workers, teams and small model profiles |
| Contracts | `catalog/`, `schemas/` | Catalog manifests and validation schemas |
| Rust source | `shared/rust-harness/` | Orchestration, resources, tools and transport |
| ML source | `shared/harness/` | Specialized ML workers, including `runpod/` |
| Private inference | `local-inference-gateway/` | Gateway, clients and deploy examples |
| Interfaces | `shared/local-chat/`, `shared/forge-console/`, `shared/menubar/` | Existing interfaces |
| Runtime configuration | `shared/config/`, `shared/skills/` | Dependency locks, plans and worker skills |
| Installed data | `shared/models/`, `shared/runtime/` | Ignored weights, environments, credentials and databases |
| Build/run output | `target/`, `runs/`, `packages/` | Binaries, run records and packages |
| Evidence and reporting | `evidence/`, `reports/`, `project-state.json`, `realm.md` | Receipts, reports and current state |
| Host integration | `.agents/`, `.codex/` | Host configuration and guidance, distinct from worker roles |
| Separate application | `Praeco/`, when present | Own repository, instructions and state |

Add new how-to documents under `docs/guides/` and link them from its index. Keep operational contracts beside their implementation. Keep credentials and weights out of tracked source.

Runtime paths remain stable: no weights, environments, run history or Praeco files were moved. The older [folder inventory](folder-hierarchy.md) and [HTML snapshot](folder-hierarchy.html) describe an earlier filesystem state, not today's repository boundaries or live readiness.
