use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const ARCHIVES: [(&str, &str); 4] = [
    ("imagegenexpert-linux-x86_64.zip", "imagegen"),
    ("imagegenexpert-macos-x86_64.zip", "imagegen"),
    ("imagegenexpert-macos-aarch64.zip", "imagegen"),
    ("imagegenexpert-windows-x86_64.zip", "imagegen.exe"),
];

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("imagegenexpert-{label}-{unique}"))
}

fn run(command: &mut Command) -> Output {
    command.output().expect("failed to execute test command")
}

fn create_asset(root: &Path, assets: &Path, archive: &str, binary: &str) {
    let skill = root.join("imagegenexpert");
    fs::create_dir_all(skill.join("agents")).unwrap();
    fs::create_dir_all(skill.join("bin")).unwrap();
    fs::write(skill.join("SKILL.md"), "---\nname: imagegenexpert\n---\n").unwrap();
    fs::write(skill.join("agents/openai.yaml"), "interface: test\n").unwrap();
    let binary_path = skill.join("bin").join(binary);
    fs::write(&binary_path, b"fixture binary").unwrap();
    #[cfg(unix)]
    if binary != "imagegen.exe" {
        fs::set_permissions(&binary_path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let output = run(Command::new("zip")
        .args(["-qr"])
        .arg(assets.join(archive))
        .arg("imagegenexpert")
        .current_dir(root));
    assert!(
        output.status.success(),
        "zip stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(skill).unwrap();
}

fn create_valid_assets(root: &Path) -> PathBuf {
    let assets = root.join("assets");
    fs::create_dir_all(&assets).unwrap();
    for (archive, binary) in ARCHIVES {
        create_asset(root, &assets, archive, binary);
    }
    assets
}

#[test]
fn release_asset_validator_accepts_directly_installable_skill_archives() {
    let root = temp_root("valid-release-assets");
    fs::create_dir_all(&root).unwrap();
    let assets = create_valid_assets(&root);

    let output = run(Command::new("bash")
        .arg("scripts/verify_release_assets.sh")
        .arg(&assets));
    assert!(
        output.status.success(),
        "validator stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn release_asset_validator_rejects_an_actions_artifact_double_zip() {
    let root = temp_root("double-zip-release-assets");
    fs::create_dir_all(&root).unwrap();
    let assets = create_valid_assets(&root);
    let archive_name = "imagegenexpert-linux-x86_64.zip";
    let wrapper = root.join("wrapper");
    fs::create_dir_all(&wrapper).unwrap();
    fs::rename(assets.join(archive_name), wrapper.join(archive_name)).unwrap();

    let output = run(Command::new("zip")
        .arg("-q")
        .arg(assets.join(archive_name))
        .arg(archive_name)
        .current_dir(&wrapper));
    assert!(
        output.status.success(),
        "zip stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = run(Command::new("bash")
        .arg("scripts/verify_release_assets.sh")
        .arg(&assets));
    assert!(
        !output.status.success(),
        "double-wrapped archive must be rejected"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("unexpected archive layout"));
    fs::remove_dir_all(root).ok();
}
