# 08 — Release, distribution and updates

Goal: **releasing should take no thought.** Bump the version, push a tag, and CI builds,
signs and publishes. Users update manually; River itself never contacts a server for
updates.

Current release target: **macOS only** (decision #36). Windows distribution and
signing are deferred; they do not block release.

## License and distribution (#40)
River Writer is **MIT-licensed**, with **no business model or monetization planned**.
Include River's `LICENSE` in distributed builds alongside the bundled font licenses
and third-party notices. The license/pricing/business-model decisions are closed;
signing and release preparation remain implementation work.

## Update policy (decision #30)
* **No auto-update and no update checks.** River makes **no network connections except
  git push/pull/fetch/clone** (the network promise stays strict).
* **Help → Check for Updates…** opens the River website's download page **in the default
  browser** (the browser connects, not River).
* **Help → Subscribe to Updates…** opens the newsletter sign-up page in the browser.
  Email addresses are collected **on the website only**, never in the app.
* **Help → About River** shows the installed version and build.
* Release notes: newsletter, website changelog, GitHub Releases.
* **Updating:** download the new installer and install over the old version. Settings
  (app config dir) and projects (plain folders) are unaffected. Settings and project
  formats stay backward-compatible; a newer River never silently rewrites `_river/` in a
  way an older River can't read (version field in `river.json`, additive changes only).

## Keeping the door open for a future updater (decision #34)
v1 has **no updater, no update checks, and no package-manager channels** (no Homebrew,
no winget). Nothing in v1 may make adding an **opt-in** updater later expensive:

| Keep stable from v1 | Why it matters later |
|---|---|
| **App identity**: macOS bundle identifier | An updater (or a newer installer) can only replace an install it recognises |
| **Signing identities**: same Apple Developer ID team on every release | Updates must be signed by the same identity, or the OS treats the app as different/untrusted |
| **Install locations** (`/Applications/River Writer.app`) | The updater swaps files in place |
| **Semantic versioning** on every release | The updater compares versions |
| **GitHub Releases as the canonical artifact host**, with stable asset naming | A future `latest.json` manifest can point at the same assets |
| **Backward-compatible settings and `_river/` formats** (additive changes, version field) | Updating must never break existing projects |

Prepared but **not shipped** in v1:
* An **updater signing keypair** (Tauri updater, Ed25519) generated once and stored
  offline (hardware-backed or password-protected, never in CI), so the first
  updater-capable release can be signed without a scramble.
* The release workflow has `createUpdaterArtifacts` **off**; turning it on later produces
  signed update bundles from the same pipeline.
* Settings schema reserves `update.mode` (only `"off"` in v1).
* The network-promise audit (dependency check + no-connection test) is written as an
  **allow-list** (today: git only), so adding the updater later is one reviewed line,
  not a rewrite.

If an updater is ever added, it must follow the guards already agreed: **opt-in** (asked
once, default off), signed (Ed25519 + OS code signing), HTTPS to one fixed static URL, no
identifiers sent, never silent installs, never during unsaved work, and the network
promise is publicly reworded at that time.

## CI/CD pipeline (GitHub Actions)

```mermaid
flowchart LR
  PR[Pull request] --> CI[CI: fmt, clippy, unit tests,<br/>golden pagination, visual regression,<br/>E2E smoke on macOS]
  TAG[Push tag vX.Y.Z] --> BUILD[tauri-action macOS build]
  BUILD --> MAC[macOS universal .dmg<br/>codesign + notarize + staple]
  MAC --> REL[GitHub Release draft<br/>+ SHA-256 checksums + notes]
  REL --> PUB[Publish release → website download links update]
```

### On every pull request / push to `main`
* Rust: `cargo fmt --check`, `cargo clippy -D warnings`, unit + golden tests
  (pagination, parser corpus), `cargo deny`/audit (licenses, advisories, and the
  **no-networking-crates check** for the network promise).
* UI: typecheck, lint, unit tests.
* Visual regression renders (pages → PNG → perceptual diff).
* E2E smoke tests of the built app on **macOS** runners.
* Performance benchmarks against the budgets in doc 05 §1 (fail on regression).

### On a version tag (`vX.Y.Z`)
* **`tauri-apps/tauri-action`** builds the macOS release:
  * **macOS:** universal binary (Apple Silicon + Intel) `.dmg`; **codesign with a
    Developer ID certificate, notarize, staple** (Apple credentials as GitHub secrets).
  * **Windows:** deferred; no installer, signing credentials or CI job required now.
  * **Linux:** not built (later or never; community port welcome).
* Creates a **draft GitHub Release** with the installers, SHA-256 checksums and release notes
  generated from the changelog.
* Publishing the release is the only manual click. The website's download links point to
  the latest GitHub Release assets (or are updated by a small website deploy step).
* No updater manifest is generated (no auto-update).

### Release flow (the whole thing)
1. Update `CHANGELOG.md`.
2. `git tag vX.Y.Z && git push --tags`.
3. Wait for CI, review the draft release, click **Publish**.
4. Send the newsletter.

## One-time setup (the only real chores)
| Item | Why | Cost (approx.) |
|---|---|---|
| Apple Developer Program | Developer ID signing + notarization so macOS doesn't block the app | $99/year |
| GitHub repository secrets | Apple ID/app-specific password or API key, team ID, certificates | — |
| Website + newsletter service | Download page, changelog, sign-up (any static site + a newsletter provider) | provider-dependent |

## Local release build (implemented, #56/#57)
Identity: bundle ID **`app.riverwriter.mac`**, product name River Writer, category
Productivity, macOS 13+, hardened runtime, original icon from `app/assets/river-icon.svg`
(`pnpm --dir app tauri icon assets/river-icon.svg -o src-tauri/icons`, keeping only the
macOS sizes). Bundles: `.app` and `.dmg`. The MIT license and Courier Prime OFL ship inside.

File types: `.fountain` by extension (Default rank; the writer can still choose another
app), `.md`/`.markdown` via `net.daringfireball.markdown` at Alternate rank (River is
offered in Open With, never forced as the Markdown default). Do not add a MIME type to
the Fountain entry: Tauri turns `text/plain` into `public.plain-text`, which would
register River for every text file.

One-time setup, done by the writer, not River:
```sh
rustup target add x86_64-apple-darwin
xcrun notarytool store-credentials river-notary --apple-id YOU@EXAMPLE.COM --team-id TEAMID
```
Then `scripts/release-mac.sh` builds a universal app and DMG, signs with the first
Developer ID Application identity (override with `APPLE_SIGNING_IDENTITY`), verifies the
signature, notarizes with the `river-notary` profile (`RIVER_NOTARY_PROFILE` to change),
staples both, runs `spctl` and writes `<dmg>.sha256`. `--arm64-only` and `--no-notarize`
help local trials. The CI release workflow above is still to be written.

## Versioning
* **Semantic versioning:** `MAJOR.MINOR.PATCH`. Patch = fixes; minor = features (slow,
  deliberate); major = rare, only for unavoidable format changes (with migration).
* `CHANGELOG.md` in the repository is the source for release notes and the newsletter.

## Not doing
* Auto-update, update checks, in-app newsletter sign-up, telemetry, crash upload.
* App stores for v1 (Mac App Store sandboxing complicates git over SSH and project
  folder access). Possible later as an extra channel.
