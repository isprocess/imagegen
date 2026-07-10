use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

pub mod google;
pub mod openai;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderKind {
    OpenAi,
    Google,
}

impl ProviderKind {
    pub fn key(&self) -> &'static str {
        match self {
            ProviderKind::OpenAi => "openai",
            ProviderKind::Google => "google",
        }
    }
}

impl fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Model {
    GptImage2,
    NanoBanana2,
    NanoBananaPro,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedModel {
    pub model: Model,
    pub provider: ProviderKind,
    pub wire_model: String,
    pub user_model: String,
}

pub fn resolve_model(value: &str) -> Result<ResolvedModel> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "gpt-image-2" => Ok(ResolvedModel {
            model: Model::GptImage2,
            provider: ProviderKind::OpenAi,
            wire_model: "gpt-image-2".to_string(),
            user_model: "gpt-image-2".to_string(),
        }),
        "nano-banana-2" | "gemini-3.1-flash-image" => Ok(ResolvedModel {
            model: Model::NanoBanana2,
            provider: ProviderKind::Google,
            wire_model: "gemini-3.1-flash-image".to_string(),
            user_model: "nano-banana-2".to_string(),
        }),
        "nano-banana-pro" | "gemini-3-pro-image" => Ok(ResolvedModel {
            model: Model::NanoBananaPro,
            provider: ProviderKind::Google,
            wire_model: "gemini-3-pro-image".to_string(),
            user_model: "nano-banana-pro".to_string(),
        }),
        _ => {
            bail!("unknown model {value:?}; choose gpt-image-2, nano-banana-2, or nano-banana-pro")
        }
    }
}

pub fn default_base_url(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::OpenAi => "https://api.openai.com",
        ProviderKind::Google => "https://generativelanguage.googleapis.com",
    }
}

pub fn is_retryable_http_status(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504 | 524)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operation {
    Generate,
    Edit,
    Compose,
}

impl Operation {
    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "generate" | "make" => Ok(Operation::Generate),
            "edit" => Ok(Operation::Edit),
            "compose" => Ok(Operation::Compose),
            _ => bail!("unknown operation {value:?}; choose generate, edit, or compose"),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Operation::Generate => "generate",
            Operation::Edit => "edit",
            Operation::Compose => "compose",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resolution {
    Small1K,
    Hd2K,
    Uhd4K,
}

impl Resolution {
    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "1k" | "small" => Ok(Resolution::Small1K),
            "2k" | "hd" => Ok(Resolution::Hd2K),
            "4k" | "uhd" => Ok(Resolution::Uhd4K),
            _ => bail!("unknown resolution {value:?}; choose 1K, 2K, or 4K"),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Resolution::Small1K => "1K",
            Resolution::Hd2K => "2K",
            Resolution::Uhd4K => "4K",
        }
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AspectRatio {
    Square1x1,
    Wide16x9,
    Tall9x16,
    Photo3x2,
    Photo2x3,
    Landscape4x3,
    Portrait3x4,
}

impl AspectRatio {
    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "1:1" | "square" => Ok(AspectRatio::Square1x1),
            "16:9" | "wide" => Ok(AspectRatio::Wide16x9),
            "9:16" | "tall" => Ok(AspectRatio::Tall9x16),
            "3:2" => Ok(AspectRatio::Photo3x2),
            "2:3" => Ok(AspectRatio::Photo2x3),
            "4:3" | "landscape" => Ok(AspectRatio::Landscape4x3),
            "3:4" | "portrait" => Ok(AspectRatio::Portrait3x4),
            _ => bail!("unknown aspect ratio {value:?}"),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            AspectRatio::Square1x1 => "1:1",
            AspectRatio::Wide16x9 => "16:9",
            AspectRatio::Tall9x16 => "9:16",
            AspectRatio::Photo3x2 => "3:2",
            AspectRatio::Photo2x3 => "2:3",
            AspectRatio::Landscape4x3 => "4:3",
            AspectRatio::Portrait3x4 => "3:4",
        }
    }

    pub fn openai_size(&self, resolution: Resolution) -> String {
        let long_edge = match resolution {
            Resolution::Small1K => 1024,
            Resolution::Hd2K => 2048,
            Resolution::Uhd4K => 3840,
        };
        let (width_ratio, height_ratio) = match self {
            AspectRatio::Square1x1 => (1, 1),
            AspectRatio::Wide16x9 => (16, 9),
            AspectRatio::Tall9x16 => (9, 16),
            AspectRatio::Photo3x2 => (3, 2),
            AspectRatio::Photo2x3 => (2, 3),
            AspectRatio::Landscape4x3 => (4, 3),
            AspectRatio::Portrait3x4 => (3, 4),
        };
        let (width, height) = if width_ratio >= height_ratio {
            (
                long_edge,
                scale_short_edge(long_edge, height_ratio, width_ratio),
            )
        } else {
            (
                scale_short_edge(long_edge, width_ratio, height_ratio),
                long_edge,
            )
        };
        format!("{width}x{height}")
    }
}

fn scale_short_edge(long_edge: u32, short_ratio: u32, long_ratio: u32) -> u32 {
    let raw = long_edge * short_ratio / long_ratio;
    (raw / 8).max(1) * 8
}

impl fmt::Display for AspectRatio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    Png,
    Jpeg,
    Webp,
}

impl OutputFormat {
    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "png" => Ok(OutputFormat::Png),
            "jpg" | "jpeg" => Ok(OutputFormat::Jpeg),
            "webp" => Ok(OutputFormat::Webp),
            _ => bail!("unknown output format {value:?}; choose png, jpeg, or webp"),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Jpeg => "jpeg",
            OutputFormat::Webp => "webp",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Jpeg => "jpg",
            OutputFormat::Webp => "webp",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageInput {
    pub source: String,
    pub data: Vec<u8>,
    pub mime_type: String,
    pub filename: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRequest {
    pub operation: Operation,
    pub prompt: String,
    pub input_images: Vec<ImageInput>,
    pub mask: Option<ImageInput>,
    pub model: ResolvedModel,
    pub resolution: Resolution,
    pub aspect_ratio: AspectRatio,
    pub output_format: OutputFormat,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputPlan {
    pub output: PathBuf,
    pub dry_run: bool,
    pub timeout_seconds: u64,
    pub retries: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunReport {
    pub ok: bool,
    pub operation: String,
    pub provider: String,
    pub model: String,
    pub wire_model: String,
    pub resolution: String,
    pub aspect_ratio: String,
    pub output_format: String,
    pub requested_count: u32,
    pub returned_count: usize,
    pub outputs: Vec<String>,
    pub dry_run: bool,
    pub timeout_seconds: u64,
    pub retries: u32,
}
