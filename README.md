# River Writer

An offline writing app for screenwriters, playwrights and novelists. You write plain
text, and River turns it into perfectly formatted pages. No cloud, no accounts, no
subscription.

Status: **engine CLI and first Mac workbench slice implemented** (66 decisions).
River now starts with New Project, Open Project and Recent Projects, restores saved
project tabs/layout, and opens/edits/saves story files, shows Rust page previews and exports
PDF, using the approved Kobalte/Corvu foundation. macOS is the sole current/v1 target.
See [workbench usage and boundaries](docs/10-mac-workbench.md) and
[engine validation](docs/09-engine-cli.md).

## Install (build it yourself)

River is distributed as source (#65). Building takes a few minutes the first time and
needs these free tools on a Mac running macOS 13 or later:

1. Xcode Command Line Tools: `xcode-select --install`
2. Rust 1.96 or newer: install from [rustup.rs](https://rustup.rs)
3. Node.js 22.12 or newer: from [nodejs.org](https://nodejs.org) or Homebrew
4. pnpm 9: `corepack enable` (comes with Node)

Then:

```sh
git clone <repository URL> river-writer
cd river-writer
pnpm --dir app install --frozen-lockfile
pnpm --dir app tauri build --bundles app
cp -R "app/src-tauri/target/release/bundle/macos/River Writer.app" /Applications/
```

The tools are only needed to build. River itself needs nothing else installed, makes
no network connections, and opens without Gatekeeper warnings because you built it.
To update, pull and run the last two commands again; your projects and settings are
untouched.

## Develop

```sh
pnpm --dir app tauri dev
cargo test --workspace
```

See [workbench usage, checks and boundaries](docs/10-mac-workbench.md).

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
| [docs/08-release-and-distribution.md](docs/08-release-and-distribution.md) | Build-from-source distribution for v1, manual-update policy, possible later CI/signing |
| [docs/09-engine-cli.md](docs/09-engine-cli.md) | Implemented CLI, crate responsibilities, supported formatting, verification and limits |
| [docs/10-mac-workbench.md](docs/10-mac-workbench.md) | Native workbench, setup, verification and remaining shell/preview work |
| [docs/decision-log.md](docs/decision-log.md) | Every product decision, with the reasoning and any reversals |

## License

[MIT](LICENSE). No business model or monetization planned. Bundled Courier Prime is
licensed separately under the SIL Open Font License; third-party components retain
their own licenses.
