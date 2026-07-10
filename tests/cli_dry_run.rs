use std::process::Command;

#[test]
fn friendly_make_pro_wide_dry_run_outputs_agent_json() {
    let binary = env!("CARGO_BIN_EXE_imagegen");
    let output = Command::new(binary)
        .args([
            "make",
            "A premium product poster",
            "--pro",
            "--wide",
            "--dry-run",
            "--output",
            "poster.png",
        ])
        .output()
        .expect("failed to run imagegen binary");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["operation"], "generate");
    assert_eq!(json["provider"], "google");
    assert_eq!(json["model"], "nano-banana-pro");
    assert_eq!(json["resolution"], "4K");
    assert_eq!(json["aspect_ratio"], "16:9");
    assert_eq!(json["dry_run"], true);
    assert_eq!(json["timeout_seconds"], 900);
    assert_eq!(json["retries"], 0);
}

#[test]
fn expert_generate_dry_run_accepts_explicit_parameters() {
    let binary = env!("CARGO_BIN_EXE_imagegen");
    let output = Command::new(binary)
        .args([
            "generate",
            "--model",
            "gpt-image-2",
            "--prompt",
            "Square logo",
            "--resolution",
            "2K",
            "--aspect-ratio",
            "1:1",
            "--output-format",
            "png",
            "--output",
            "logo.png",
            "--dry-run",
        ])
        .output()
        .expect("failed to run imagegen binary");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(json["operation"], "generate");
    assert_eq!(json["provider"], "openai");
    assert_eq!(json["model"], "gpt-image-2");
    assert_eq!(json["resolution"], "2K");
    assert_eq!(json["aspect_ratio"], "1:1");
}

#[test]
fn config_write_accepts_api_key_from_stdin() {
    use std::io::Write;
    use std::process::Stdio;
    use std::time::{SystemTime, UNIX_EPOCH};

    let binary = env!("CARGO_BIN_EXE_imagegen");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let config_path = std::env::temp_dir().join(format!("imagegen-config-{unique}.json"));

    let mut child = Command::new(binary)
        .args([
            "config",
            "write",
            "--provider",
            "google",
            "--base-url",
            "https://example.com",
            "--api-key-stdin",
            "--config",
        ])
        .arg(&config_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn imagegen");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"secret-key\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::fs::read_to_string(&config_path).unwrap();
    assert!(text.contains("\"apiKey\": \"secret-key\""));
    std::fs::remove_file(config_path).ok();
}
