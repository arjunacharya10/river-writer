# River Writer

An offline writing app for screenwriters, playwrights and novelists. You write plain
text, and River turns it into perfectly formatted pages. No cloud, no accounts, no
subscription.

Status: **engine CLI and first Mac workbench slice implemented** (38 decisions).
The native app now opens/edits/saves story files, shows Rust page previews and exports
PDF, using the approved Kobalte/Corvu foundation. macOS is the sole current/v1 target.
See [workbench usage and boundaries](docs/10-mac-workbench.md) and
[engine validation](docs/09-engine-cli.md).

```sh
pnpm --dir app install
pnpm --dir app tauri dev
cargo test --workspace
```

Next: finish the shell/preview phases—incremental updates, watching, navigation and
preview polish—then extras/settings and versioning. The editor under `spikes/`
remains an experiment.

## Documents

| Doc | Contents |
|---|---|
| [docs/01-product-brief.md](docs/01-product-brief.md) | Vision, audience, principles, what's in and what's cut |
| [docs/02-files-and-projects.md](docs/02-files-and-projects.md) | File formats, project folder layout, `_river/`, extras |
| [docs/03-editor.md](docs/03-editor.md) | UI exactly like VS Code: activity bar, side-bar extras, tabs for story files, template dropdown + preview icon |
| [docs/04-versioning-git.md](docs/04-versioning-git.md) | Git integration, the vocabulary subset, autosave, diff/merge, remotes |
| [docs/05-architecture.md](docs/05-architecture.md) | Detailed Tauri architecture: process model, crates, typesetting, IPC, git internals, security, testing, build phases |
| [docs/06-roadmap.md](docs/06-roadmap.md) | Version 1 / 1.1 / never, plus open decisions |
| [docs/07-prior-art.md](docs/07-prior-art.md) | Technical lessons from Obsidian and open-source projects (what we reuse instead of re-solving) |
| [docs/08-release-and-distribution.md](docs/08-release-and-distribution.md) | CI/CD (GitHub Actions + tauri-action), signing, release flow, manual-update policy, newsletter |
| [docs/09-engine-cli.md](docs/09-engine-cli.md) | Implemented CLI, crate responsibilities, supported formatting, verification and limits |
| [docs/10-mac-workbench.md](docs/10-mac-workbench.md) | Native workbench, setup, verification and remaining shell/preview work |
| [docs/decision-log.md](docs/decision-log.md) | Every product decision, with the reasoning and any reversals |

