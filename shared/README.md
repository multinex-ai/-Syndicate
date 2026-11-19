# shared/

The common implementation and installed dependencies for every worker. Source is tracked; the large installed pieces are not.

| Path | What it is | Kind |
| --- | --- | --- |
| `rust-harness/` | Crate `local-agents-rs`: agent loop, tools, job supervisor, SQLite job store, STDIO MCP bridge, media workers | source (`target/` is build output) |
| `bin/` | Launchers (`run-agent.sh`, `run-image.sh`, `run-video.sh`, `run-mcp.sh`, `local-chat.command`), downloaders, report builder, qualification runner | source |
| `harness/` | Python that remains where only a Python library can do the work (MLX media kernel, MemQ client), plus acceptance scripts and unit tests | source |
| `private-inference/` | Authenticated gateway, client transport, budget ledger and Runpod deployment documentation | source |
| `config/` | Runtime plan, dependency pins (`requirements.in`, `requirements.lock.txt`), MemQ and skills settings | source |
| `skills/` | Guidance packages exposed to local workers (`list_skills` / `read_skill`) | source |
| `vendor/mlx-wan/` | Vendored video runtime source | source |
| `models/` | Model weights (Qwen, FLUX components, Wan) | installed, git-ignored |
| `runtime/` | Virtual environments, job database, locks, wheels | installed, git-ignored |

## Common tasks

Build the harness, then run its tests:

```sh
cargo build --release --manifest-path rust-harness/Cargo.toml
cargo test --release --manifest-path rust-harness/Cargo.toml
```

Run the Python tests and the MCP protocol check from the Agents root:

```sh
shared/runtime/venv/bin/python -m unittest discover -s shared/harness -p 'test_*.py'
shared/runtime/venv/bin/python shared/bin/check_mcp.py
```

Regenerate the qualification report (reads evidence, writes `reports/local-agent-models.html`):

```sh
node shared/bin/build-report.mjs && node shared/bin/check-report.mjs
```

## Rules for this folder

- Do not change anything here to suit one application. Praeco and other projects keep their own state in their own folders and treat `shared/` as a dependency.
- Do not delete `models/` or `runtime/` to free space without checking what is loaded. Only one package-managed heavy model runs at a time.
- Design background: [harness plan](../docs/harness-plan.md), [Rust port status](../docs/rust-port-status.md), [client integration](../docs/client-integration.md).
