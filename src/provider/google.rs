use crate::image_io::decode_base64_image;
use crate::provider::{ImageRequest, OutputFormat};
use anyhow::{bail, Result};
use serde_json::{json, Value};

pub fn api_key_header_name() -> &'static str {
    "x-goog-api-key"
}

pub fn build_interactions_request(request: &ImageRequest) -> Result<Value> {
    let mut input = Vec::new();
    for image in &request.input_images {
        input.push(json!({
            "type": "image",
            "mime_type": image.mime_type,
            "data": base64::prelude::BASE64_STANDARD.encode(&image.data),
        }));
    }
    if let Some(mask) = &request.mask {
        input.push(json!({
            "type": "image",
            "role": "mask",
            "mime_type": mask.mime_type,
            "data": base64::prelude::BASE64_STANDARD.encode(&mask.data),
        }));
    }
    input.push(json!({
        "type": "text",
        "text": request.prompt,
    }));

    let mut payload = json!({
        "model": request.model.wire_model,
        "input": input,
        "response_format": {
            "type": "image",
            "mime_type": google_mime_type(request.output_format),
            "image_size": request.resolution.as_str(),
            "aspect_ratio": request.aspect_ratio.as_str()
        }
    });
    if request.count > 1 {
        payload["response_format"]["number_of_images"] = json!(request.count);
    }
    Ok(payload)
}

pub fn build_gemini_generate_content_request(request: &ImageRequest) -> Result<Value> {
    let mut parts = vec![json!({
        "text": request.prompt,
    })];
    for image in &request.input_images {
        parts.push(json!({
            "inline_data": {
                "mime_type": image.mime_type,
                "data": base64::prelude::BASE64_STANDARD.encode(&image.data),
            }
        }));
    }
    if let Some(mask) = &request.mask {
        parts.push(json!({
            "inline_data": {
                "mime_type": mask.mime_type,
                "data": base64::prelude::BASE64_STANDARD.encode(&mask.data),
            }
        }));
    }

    Ok(json!({
        "contents": [
            {
                "role": "user",
                "parts": parts
            }
        ],
        "generationConfig": {
            "responseModalities": ["IMAGE"],
            "imageConfig": {
                "aspectRatio": request.aspect_ratio.as_str(),
                "imageSize": request.resolution.as_str()
            }
        }
    }))
}

fn google_mime_type(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::Png => "image/png",
        OutputFormat::Jpeg => "image/jpeg",
        OutputFormat::Webp => "image/webp",
    }
}

pub fn parse_google_images(response: &Value) -> Result<Vec<Vec<u8>>> {
    let mut images = Vec::new();
    collect_google_images(response, &mut images)?;
    if images.is_empty() {
        bail!("Google response did not include base64 image data");
    }
    Ok(images)
}

fn collect_google_images(value: &Value, images: &mut Vec<Vec<u8>>) -> Result<()> {
    match value {
        Value::Object(map) => {
            let type_is_output_image = map
                .get("type")
                .and_then(Value::as_str)
                .map(|value| value == "output_image" || value == "image")
                .unwrap_or(false);
            if type_is_output_image {
                if let Some(data) = map.get("data").and_then(Value::as_str) {
                    images.push(decode_base64_image(data)?);
                }
            }
            if let Some(inline_data) = map.get("inlineData").or_else(|| map.get("inline_data")) {
                if let Some(data) = inline_data.get("data").and_then(Value::as_str) {
                    images.push(decode_base64_image(data)?);
                }
            }
            if let Some(output_image) = map.get("output_image") {
                if let Some(data) = output_image.get("data").and_then(Value::as_str) {
                    images.push(decode_base64_image(data)?);
                }
            }
            for key in [
                "output",
                "candidates",
                "content",
                "parts",
                "inlineData",
                "inline_data",
            ] {
                if let Some(child) = map.get(key) {
                    collect_google_images(child, images)?;
                }
            }
            for (key, child) in map {
                if [
                    "output",
                    "candidates",
                    "content",
                    "parts",
                    "inlineData",
                    "inline_data",
                ]
                .contains(&key.as_str())
                {
                    continue;
                }
                collect_google_images(child, images)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_google_images(item, images)?;
            }
        }
        _ => {}
    }
    Ok(())
}

use base64::Engine;
