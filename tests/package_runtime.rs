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
