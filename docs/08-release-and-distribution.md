# 08 — Release, distribution and updates

Goal: **releasing should take no thought.** Bump the version, push a tag, and CI builds,
signs and publishes. Users update manually; River itself never contacts a server for
updates.

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

## CI/CD pipeline (GitHub Actions)

```mermaid
flowchart LR
  PR[Pull request] --> CI[CI: fmt, clippy, unit tests,<br/>golden pagination, visual regression,<br/>E2E smoke on macOS + Windows]
  TAG[Push tag vX.Y.Z] --> BUILD[tauri-action build matrix]
  BUILD --> MAC[macOS universal .dmg<br/>codesign + notarize + staple]
  BUILD --> WIN[Windows .msi / NSIS .exe<br/>Authenticode via Azure Trusted Signing]
  MAC --> REL[GitHub Release draft<br/>+ SHA-256 checksums + notes]
  WIN --> REL
  REL --> PUB[Publish release → website download links update]
```

### On every pull request / push to `main`
* Rust: `cargo fmt --check`, `cargo clippy -D warnings`, unit + golden tests
  (pagination, parser corpus), `cargo deny`/audit (licenses, advisories, and the
  **no-networking-crates check** for the network promise).
* UI: typecheck, lint, unit tests.
* Visual regression renders (pages → PNG → perceptual diff).
* E2E smoke tests of the built app on **macOS and Windows** runners.
* Performance benchmarks against the budgets in doc 05 §1 (fail on regression).

### On a version tag (`vX.Y.Z`)
* **`tauri-apps/tauri-action`** builds the installers in a matrix:
  * **macOS:** universal binary (Apple Silicon + Intel) `.dmg`; **codesign with a
    Developer ID certificate, notarize, staple** (Apple credentials as GitHub secrets).
  * **Windows:** `.msi` and/or NSIS `.exe`; **Authenticode signing via Azure Trusted
    Signing** (or an OV certificate); WebView2 bootstrapper included.
  * **Linux (when enabled):** AppImage + `.deb` (+ `.rpm`), added as one more matrix row.
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
| Windows code signing (Azure Trusted Signing, or an OV certificate) | Avoid "unknown publisher" / SmartScreen warnings | ~$10/month (Azure) |
| GitHub repository secrets | Apple ID/app-specific password or API key, team ID, certificates; Azure signing credentials | — |
| Website + newsletter service | Download page, changelog, sign-up (any static site + a newsletter provider) | provider-dependent |

## Versioning
* **Semantic versioning:** `MAJOR.MINOR.PATCH`. Patch = fixes; minor = features (slow,
  deliberate); major = rare, only for unavoidable format changes (with migration).
* `CHANGELOG.md` in the repository is the source for release notes and the newsletter.

## Not doing
* Auto-update, update checks, in-app newsletter sign-up, telemetry, crash upload.
* App stores for v1 (Mac App Store sandboxing complicates git over SSH and project
  folder access). Possible later as an extra channel.
