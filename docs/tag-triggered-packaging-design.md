# Tag-Triggered Packaging Design

## Goal

Make GitHub Actions package the project when a `v*` Git tag is pushed, while
keeping the tag available for release traceability and failure investigation.

## Scope

- Replace the existing `push.branches: ["v*"]` filter with
  `push.tags: ["v*"]`.
- Keep `workflow_dispatch` for manual packaging runs.
- Keep the existing test, four-platform build matrix, runtime staging, ZIP
  creation, and Actions artifact upload behavior.
- Document the automated packaging workflow in the committed `README.md`.
- Add a regression test that distinguishes tag filters from branch filters.

## Workflow Behavior

A push of a tag whose name starts with `v` starts the `package` workflow. The
workflow runs formatting checks, tests, and a release build before building and
uploading Linux x86_64, macOS x86_64, macOS aarch64, and Windows x86_64 ZIP
artifacts.

The workflow never deletes or moves the source tag. If any job fails, the tag
remains available for diagnosis. A manual `workflow_dispatch` run uses the
selected ref and follows the same build and upload pipeline.

## Documentation

The README will explain:

- the required `v*` tag naming convention;
- the version consistency check between the tag and `Cargo.toml`;
- commands for creating and pushing an annotated tag;
- the four ZIP artifact names and where to download them;
- that tags are retained after success and failure;
- that existing tag push events are not replayed after a workflow change;
- that the workflow uploads Actions artifacts and does not create a GitHub
  Release.

## Testing

Extend `tests/package_runtime.rs` with a source-level workflow contract test.
The test must require a `tags` filter containing `v*` and reject the former
`branches` filter. Run the focused test first to demonstrate the regression,
then run formatting checks, the complete locked test suite, and the locked
release build.

## Non-Goals

- Deleting tags after packaging.
- Creating or updating GitHub Releases.
- Publishing the crate to crates.io.
- Changing archive contents or platform targets.
- Supporting version-like branches as packaging triggers.
