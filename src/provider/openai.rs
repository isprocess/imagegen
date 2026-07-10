use crate::image_io::decode_base64_image;
use crate::provider::{ImageInput, ImageRequest};
use anyhow::{bail, Result};
use reqwest::multipart::{Form, Part};
use serde_json::{json, Value};

pub fn build_generation_request(request: &ImageRequest) -> Result<Value> {
    if !request.input_images.is_empty() {
        bail!("OpenAI generation request cannot include input images");
    }
    let mut payload = json!({
        "model": request.model.wire_model,
        "prompt": request.prompt,
        "size": request.aspect_ratio.openai_size(request.resolution),
        "quality": "high",
        "output_format": request.output_format.as_str(),
    });
    if request.count > 1 {
        payload["n"] = json!(request.count);
    }
    Ok(payload)
}

pub fn build_edit_form(request: &ImageRequest) -> Result<Form> {
    if request.input_images.is_empty() {
        bail!("OpenAI edit request requires at least one input image");
    }
    let mut form = Form::new()
        .text("model", request.model.wire_model.clone())
        .text("prompt", request.prompt.clone())
        .text("size", request.aspect_ratio.openai_size(request.resolution))
        .text("quality", "high")
        .text("output_format", request.output_format.as_str().to_string());
    if request.count > 1 {
        form = form.text("n", request.count.to_string());
    }
    for image in &request.input_images {
        form = form.part("image[]", image_part(image)?);
    }
    if let Some(mask) = &request.mask {
        form = form.part("mask", image_part(mask)?);
    }
    Ok(form)
}

fn image_part(input: &ImageInput) -> Result<Part> {
    Ok(Part::bytes(input.data.clone())
        .file_name(input.filename.clone())
        .mime_str(&input.mime_type)?)
}

pub fn parse_openai_images(response: &Value) -> Result<Vec<Vec<u8>>> {
    let items = response
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("OpenAI response did not include data[]"))?;
    let mut images = Vec::new();
    for item in items {
        if let Some(value) = item.get("b64_json").and_then(Value::as_str) {
            images.push(decode_base64_image(value)?);
        } else if let Some(value) = item.get("image_base64").and_then(Value::as_str) {
            images.push(decode_base64_image(value)?);
        }
    }
    if images.is_empty() {
        bail!("OpenAI response did not include base64 image data");
    }
    Ok(images)
}
