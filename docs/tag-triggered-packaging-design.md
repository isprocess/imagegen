# Tag-Triggered Release Packaging Design

## Goal

Publish one directly installable skill ZIP per supported operating system on a
GitHub Release created from each `v*` tag. Extracting an asset into a Codex or
Claude Code skills directory must produce `imagegenexpert/SKILL.md` and a
working bundled binary without requiring a second unzip operation or a local
Rust toolchain.

## Current Failure

The existing matrix jobs create valid platform ZIP files, then
`actions/upload-artifact` wraps each ZIP in another Actions artifact ZIP. Users
must download from a workflow run and unzip twice. The workflow never creates a
GitHub Release.

The Linux binary is built for `x86_64-unknown-linux-gnu` on `ubuntu-latest` and
requires GLIBC 2.38. It does not run on otherwise supported environments with
an older glibc, so the current Linux package is not portable enough for the
"extract and use" contract.

## Release Asset Contract

Each GitHub Release contains exactly these platform assets:

- `imagegenexpert-linux-x86_64.zip`
- `imagegenexpert-macos-x86_64.zip`
- `imagegenexpert-macos-aarch64.zip`
- `imagegenexpert-windows-x86_64.zip`

Every ZIP has one top-level directory and the following runtime-only layout:

```text
imagegenexpert/
  SKILL.md
  agents/openai.yaml
  bin/imagegen        # imagegen.exe in the Windows asset
```

Extracting a ZIP into `~/.codex/skills/` or `~/.claude/skills/` therefore
creates `~/.codex/skills/imagegenexpert/SKILL.md` or
`~/.claude/skills/imagegenexpert/SKILL.md`. Development files such as
`Cargo.toml`, `src/`, `tests/`, `target/`, `temp/`, and `docs/` remain excluded.

## Workflow Architecture

A pushed `v*` tag runs the existing test job and four-platform package matrix.
The matrix continues using Actions artifacts only as internal job-to-job
transport. A final `release` job depends on the complete package matrix,
downloads and unwraps all four internal artifacts, validates their contents,
and publishes the inner ZIP files as direct GitHub Release assets.

The release job runs only for a tag `push`. `workflow_dispatch` still exercises
the build and package jobs but does not create a release. The workflow never
deletes or moves a tag.

The release job uses job-level `contents: write`; all test and package jobs
retain the workflow-level `contents: read`. GitHub CLI creates a release with
generated notes when it does not exist. On a rerun, it uploads the four assets
with replacement semantics instead of racing to create a second release.

## Linux Portability

The Linux matrix entry changes to `x86_64-unknown-linux-musl` while retaining
the public asset name `imagegenexpert-linux-x86_64.zip`. The Ubuntu runner
installs `musl-tools`, Rust installs the musl target, and the package job checks
that the resulting ELF has no dynamic program interpreter before staging it.
This prevents a future regression back to a glibc-dependent Linux asset.

## Validation And Failure Behavior

Before publication, the release job requires exactly four ZIP files and checks
that each contains the common skill files plus the correct platform binary
name. A missing, extra, malformed, or incorrectly rooted archive fails the job
before release creation or asset replacement.

If tests, any matrix build, archive validation, or release publication fails,
the workflow fails and the source tag remains available for investigation. A
rerun is safe: an existing release is reused and same-named assets are replaced.

Repository tests lock the following contracts:

- automatic triggers remain `v*` tag pushes plus manual dispatch;
- the Linux target is musl and a static-link check exists;
- the release job depends on the package matrix and has only job-level write
  permission;
- manual runs do not publish releases;
- staged and archived skill layouts match the runtime-only asset contract.

Verification includes formatting, the complete locked test suite, a locked
release build, local archive inspection, and workflow source review.

## Version And Rollout

The crate and CLI version advance from `0.1.0` to `0.1.1`. The existing
`v0.1.0` tag remains unchanged. After the implementation reaches `main`, a new
annotated `v0.1.1` tag triggers the first Release-producing run, keeping the tag
and binary versions aligned.

## Documentation

The README replaces the Actions-artifact download path with GitHub Releases and
documents direct extraction into Codex and Claude Code skill roots. It retains
local packaging instructions, the version/tag consistency rule, the four asset
names, manual build behavior, failure retention, and the fact that old tag push
events are not replayed.

## Non-Goals

- Deleting or moving release tags.
- Publishing the crate to crates.io.
- Bundling source or development-only files in release assets.
- Adding architectures beyond the existing four public platform variants.
- Creating a release from `workflow_dispatch`.
