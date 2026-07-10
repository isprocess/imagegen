use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn package_script_stages_only_runtime_skill_files() {
    let binary = env!("CARGO_BIN_EXE_imagegen");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let out = std::env::temp_dir().join(format!("imagegenexpert-package-{unique}"));

    let output = Command::new("bash")
        .arg("scripts/package_skill.sh")
        .arg(&out)
        .env("IMAGEGEN_BIN_OVERRIDE", binary)
        .output()
        .expect("failed to run packaging script");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out.join("SKILL.md").is_file());
    assert!(out.join("agents/openai.yaml").is_file());
    assert!(out.join("bin/imagegen").is_file());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(out.join("bin/imagegen"))
            .unwrap()
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0, "packaged Unix binary must be executable");
    }

    for dev_path in ["Cargo.toml", "src", "tests", "target", "temp", "docs"] {
        assert!(
            !out.join(dev_path).exists(),
            "runtime package should not include {dev_path}"
        );
    }

    fs::remove_dir_all(out).ok();
}

#[test]
fn release_workflow_uses_runtime_package_script() {
    let workflow = include_str!("../.github/workflows/build.yml");
    assert!(
        workflow.contains("scripts/package_skill.sh"),
        "release workflow should use the same runtime-only packaging script as local builds"
    );
    assert!(
        !workflow.contains("cp SKILL.md README.md"),
        "release workflow should not hand-copy README into the skill runtime package"
    );
    assert!(
        !workflow.contains(".claude/commands"),
        "release workflow should not hand-copy development command files into the runtime package"
    );
}

#[test]
fn release_workflow_triggers_on_version_tags_not_version_branches() {
    let workflow = include_str!("../.github/workflows/build.yml");
    let (_, after_on) = workflow
        .split_once("\non:\n")
        .expect("release workflow should define an on section");
    let (triggers, _) = after_on
        .split_once("\npermissions:\n")
        .expect("release workflow should define permissions after its triggers");

    assert_eq!(
        triggers, "  push:\n    tags:\n      - \"v*\"\n  workflow_dispatch:\n",
        "release workflow should only trigger automatically for v* tags"
    );
}

#[test]
fn release_workflow_builds_static_linux_and_publishes_direct_assets() {
    let workflow = include_str!("../.github/workflows/build.yml");
    assert!(workflow.contains("target: x86_64-unknown-linux-musl"));
    assert!(!workflow.contains("target: x86_64-unknown-linux-gnu"));
    assert!(workflow.contains("sudo apt-get install --yes musl-tools"));
    assert!(workflow.contains("Requesting program interpreter"));

    let (_, after_permissions) = workflow
        .split_once("\npermissions:\n")
        .expect("release workflow should define top-level permissions");
    let (permissions, _) = after_permissions
        .split_once("\njobs:\n")
        .expect("release workflow should define jobs after permissions");
    assert_eq!(
        permissions, "  contents: read\n",
        "workflow-level permissions must remain read-only"
    );

    let (before_release, release) = workflow
        .split_once("\n  release:\n")
        .expect("release workflow should define a release job");
    for required in [
        "needs: package",
        "if: github.event_name == 'push' && github.ref_type == 'tag'",
        "contents: write",
        "actions/download-artifact@v4",
        "merge-multiple: true",
        "scripts/verify_release_assets.sh release-assets",
        "gh release create",
        "gh release upload",
        "--clobber",
    ] {
        assert!(
            release.contains(required),
            "release job is missing {required}"
        );
    }
    assert!(!before_release.contains("contents: write"));
    assert_eq!(
        workflow.matches("contents: write").count(),
        1,
        "write permission must be granted only to the release job"
    );
}

#[test]
fn readme_documents_direct_release_installation() {
    let readme = include_str!("../README.md");
    for required in [
        "GitHub Releases",
        "v0.1.1",
        "~/.codex/skills/",
        "~/.claude/skills/",
        "unzip imagegenexpert-linux-x86_64.zip -d",
        "不需要二次解压",
        "workflow_dispatch",
    ] {
        assert!(readme.contains(required), "README is missing {required}");
    }
    assert!(!readme.contains("Actions → package → 对应运行 → Artifacts"));
}
