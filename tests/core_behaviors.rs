use imagegen::config::{Config, ProviderConfig};
use imagegen::image_io::{
    decode_base64_image, download_remote_image, output_path_for_index, validate_remote_image_url,
};
use imagegen::provider::google::{
    build_gemini_generate_content_request, build_interactions_request, parse_google_images,
};
use imagegen::provider::openai::{build_generation_request, parse_openai_images};
use imagegen::provider::{
    default_base_url, is_retryable_http_status, resolve_model, AspectRatio, ImageRequest, Model,
    Operation, OutputFormat, ProviderKind, Resolution,
};
use std::collections::BTreeMap;
use std::path::Path;

#[test]
fn resolves_known_models_to_provider_and_wire_model() {
    let google_pro = resolve_model("nano-banana-pro").unwrap();
    assert_eq!(google_pro.model, Model::NanoBananaPro);
    assert_eq!(google_pro.provider, ProviderKind::Google);
    assert_eq!(google_pro.wire_model, "gemini-3-pro-image");

    let google_fast = resolve_model("nano-banana-2").unwrap();
    assert_eq!(google_fast.model, Model::NanoBanana2);
    assert_eq!(google_fast.provider, ProviderKind::Google);
    assert_eq!(google_fast.wire_model, "gemini-3.1-flash-image");

    let openai = resolve_model("gpt-image-2").unwrap();
    assert_eq!(openai.model, Model::GptImage2);
    assert_eq!(openai.provider, ProviderKind::OpenAi);
    assert_eq!(openai.wire_model, "gpt-image-2");
}

#[test]
fn builds_google_interactions_request_with_resolution_and_aspect_ratio() {
    let request = ImageRequest {
        operation: Operation::Generate,
        prompt: "A 4K wide product poster".to_string(),
        input_images: vec![],
        mask: None,
        model: resolve_model("nano-banana-pro").unwrap(),
        resolution: Resolution::Uhd4K,
        aspect_ratio: AspectRatio::Wide16x9,
        output_format: OutputFormat::Png,
        count: 1,
    };

    let payload = build_interactions_request(&request).unwrap();
    assert_eq!(payload["model"], "gemini-3-pro-image");
    assert_eq!(payload["input"][0]["type"], "text");
    assert_eq!(payload["input"][0]["text"], "A 4K wide product poster");
    assert_eq!(payload["response_format"]["type"], "image");
    assert_eq!(payload["response_format"]["mime_type"], "image/png");
    assert_eq!(payload["response_format"]["image_size"], "4K");
    assert_eq!(payload["response_format"]["aspect_ratio"], "16:9");
}

#[test]
fn google_api_key_header_matches_interactions_rest_api() {
    assert_eq!(
        imagegen::provider::google::api_key_header_name(),
        "x-goog-api-key"
    );
}

#[test]
fn builds_google_gemini_generate_content_request_with_images() {
    let request = ImageRequest {
        operation: Operation::Compose,
        prompt: "Create a polished ecommerce product detail image".to_string(),
        input_images: vec![imagegen::provider::ImageInput {
            source: "reference.jpg".to_string(),
            data: b"image-bytes".to_vec(),
            mime_type: "image/jpeg".to_string(),
            filename: "reference.jpg".to_string(),
        }],
        mask: None,
        model: resolve_model("nano-banana-2").unwrap(),
        resolution: Resolution::Hd2K,
        aspect_ratio: AspectRatio::Square1x1,
        output_format: OutputFormat::Png,
        count: 1,
    };

    let payload = build_gemini_generate_content_request(&request).unwrap();
    assert_eq!(payload["contents"][0]["parts"][0]["text"], request.prompt);
    assert_eq!(
        payload["contents"][0]["parts"][1]["inline_data"]["mime_type"],
        "image/jpeg"
    );
    assert_eq!(
        payload["contents"][0]["parts"][1]["inline_data"]["data"],
        "aW1hZ2UtYnl0ZXM="
    );
    assert_eq!(
        payload["generationConfig"]["responseModalities"][0],
        "IMAGE"
    );
    assert_eq!(
        payload["generationConfig"]["imageConfig"]["aspectRatio"],
        "1:1"
    );
    assert_eq!(
        payload["generationConfig"]["imageConfig"]["imageSize"],
        "2K"
    );
}

#[test]
fn builds_openai_generation_request_with_size_mapping() {
    let request = ImageRequest {
        operation: Operation::Generate,
        prompt: "Square logo".to_string(),
        input_images: vec![],
        mask: None,
        model: resolve_model("gpt-image-2").unwrap(),
        resolution: Resolution::Hd2K,
        aspect_ratio: AspectRatio::Square1x1,
        output_format: OutputFormat::Png,
        count: 1,
    };

    let payload = build_generation_request(&request).unwrap();
    assert_eq!(payload["model"], "gpt-image-2");
    assert_eq!(payload["prompt"], "Square logo");
    assert_eq!(payload["quality"], "high");
    assert_eq!(payload["size"], "2048x2048");
    assert_eq!(payload["output_format"], "png");
}

#[test]
fn builds_openai_generation_request_with_4k_landscape_size() {
    let request = ImageRequest {
        operation: Operation::Generate,
        prompt: "4K landscape product poster".to_string(),
        input_images: vec![],
        mask: None,
        model: resolve_model("gpt-image-2").unwrap(),
        resolution: Resolution::Uhd4K,
        aspect_ratio: AspectRatio::Wide16x9,
        output_format: OutputFormat::Png,
        count: 1,
    };

    let payload = build_generation_request(&request).unwrap();
    assert_eq!(payload["size"], "3840x2160");
}

#[test]
fn parses_openai_base64_images() {
    let response = serde_json::json!({
        "data": [
            {"b64_json": "aGVsbG8="},
            {"image_base64": "d29ybGQ="}
        ]
    });

    let images = parse_openai_images(&response).unwrap();
    assert_eq!(images, vec![b"hello".to_vec(), b"world".to_vec()]);
}

#[test]
fn parses_google_base64_images_from_common_shapes() {
    let response = serde_json::json!({
        "output": [
            {"type": "output_image", "data": "aGVsbG8="}
        ],
        "candidates": [
            {
                "content": {
                    "parts": [
                        {"inlineData": {"mimeType": "image/png", "data": "d29ybGQ="}}
                    ]
                }
            }
        ]
    });

    let images = parse_google_images(&response).unwrap();
    assert_eq!(images, vec![b"hello".to_vec(), b"world".to_vec()]);
}

#[test]
fn decodes_base64_with_data_url_prefix() {
    let bytes = decode_base64_image("data:image/png;base64,aGVsbG8=").unwrap();
    assert_eq!(bytes, b"hello");
}

#[test]
fn numbers_multiple_output_paths_before_extension() {
    let first = output_path_for_index(Path::new("poster.png"), 0, 3);
    let second = output_path_for_index(Path::new("poster.png"), 1, 3);
    assert_eq!(first, Path::new("poster_01.png"));
    assert_eq!(second, Path::new("poster_02.png"));
}

#[test]
fn config_prefers_environment_values_over_file_values() {
    let mut file = Config::default();
    file.default_model = Some("nano-banana-2".to_string());
    file.providers.insert(
        "google".to_string(),
        ProviderConfig {
            base_url: Some("https://file.example".to_string()),
            api_key: Some("file-key".to_string()),
        },
    );

    let mut env = BTreeMap::new();
    env.insert(
        "IMAGEGEN_GOOGLE_BASE_URL".to_string(),
        "https://env.example".to_string(),
    );
    env.insert("IMAGEGEN_GOOGLE_API_KEY".to_string(), "env-key".to_string());
    env.insert(
        "IMAGEGEN_DEFAULT_MODEL".to_string(),
        "nano-banana-pro".to_string(),
    );

    let resolved = Config::merge_env(file, &env);
    let google = resolved.providers.get("google").unwrap();
    assert_eq!(resolved.default_model.as_deref(), Some("nano-banana-pro"));
    assert_eq!(google.base_url.as_deref(), Some("https://env.example"));
    assert_eq!(google.api_key.as_deref(), Some("env-key"));
}

#[test]
fn providers_have_customer_friendly_default_base_urls() {
    assert_eq!(
        default_base_url(ProviderKind::OpenAi),
        "https://api.openai.com"
    );
    assert_eq!(
        default_base_url(ProviderKind::Google),
        "https://generativelanguage.googleapis.com"
    );
}

#[test]
fn remote_image_url_safety_rejects_unsafe_urls() {
    assert!(validate_remote_image_url("http://example.com/image.png", false).is_err());
    assert!(validate_remote_image_url("https://user:pass@example.com/image.png", false).is_err());
    assert!(validate_remote_image_url("https://localhost/image.png", false).is_err());
    assert!(validate_remote_image_url("https://example.local/image.png", false).is_err());
    assert!(validate_remote_image_url("https://127.0.0.1/image.png", false).is_err());
    assert!(validate_remote_image_url("https://10.0.0.1/image.png", false).is_err());
    assert!(validate_remote_image_url("https://169.254.1.1/image.png", false).is_err());
    assert!(validate_remote_image_url("https://224.0.0.1/image.png", false).is_err());
}

#[test]
fn remote_image_url_safety_accepts_global_https_ip_literals() {
    let parsed = validate_remote_image_url("https://93.184.216.34/image.png", false).unwrap();
    assert_eq!(parsed.scheme(), "https");
    assert_eq!(parsed.host_str(), Some("93.184.216.34"));
}

#[tokio::test]
async fn remote_image_download_rejects_insecure_urls_before_network() {
    let error = download_remote_image("http://example.com/image.png", 1)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("must use HTTPS"));
}

#[test]
fn retryable_statuses_are_explicit() {
    for status in [429, 500, 502, 503, 504, 524] {
        assert!(is_retryable_http_status(status), "{status} should retry");
    }
    for status in [400, 401, 403, 404, 422] {
        assert!(
            !is_retryable_http_status(status),
            "{status} should not retry"
        );
    }
}

#[test]
fn skill_instructions_use_bundled_runtime_not_project_local_bin() {
    let skill = include_str!("../SKILL.md");
    assert!(
        skill.contains("skill 安装目录") || skill.contains("Skill installation directory"),
        "skill should explain that bin/imagegen is resolved from the skill installation directory"
    );
    assert!(
        skill.contains("bin/imagegen"),
        "skill should reference the bundled runtime binary"
    );
    assert!(
        !skill.contains("./bin/imagegen"),
        "skill must not require a project-local ./bin/imagegen"
    );
    assert!(
        skill.contains("--api-key-stdin"),
        "skill should document safe guided setup without putting API keys on the command line"
    );
}
