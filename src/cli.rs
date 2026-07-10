use crate::config::{load_effective_config, write_provider_config, Config, ProviderConfig};
use crate::image_io::{
    download_remote_image_with_client, guess_mime_type, remote_image_client, write_images,
};
use crate::provider::google::{
    api_key_header_name, build_gemini_generate_content_request, build_interactions_request,
    parse_google_images,
};
use crate::provider::openai::{build_edit_form, build_generation_request, parse_openai_images};
use crate::provider::{
    default_base_url, is_retryable_http_status, resolve_model, AspectRatio, ImageInput,
    ImageRequest, Operation, OutputFormat, OutputPlan, ProviderKind, Resolution, RunReport,
};
use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use reqwest::Client;
use serde_json::json;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;
use url::Url;

#[derive(Debug, Parser)]
#[command(
    name = "imagegen",
    version,
    about = "Generate and edit images with AI models."
)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Make(FriendlyMake),
    Edit(FriendlyEdit),
    Compose(FriendlyCompose),
    Generate(GenerateArgs),
    Doctor(ConfigPathArgs),
    Models,
    Presets,
    Config(ConfigCommand),
}

#[derive(Debug, Args)]
pub struct ConfigPathArgs {
    #[arg(long)]
    config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct FriendlyFlags {
    #[arg(long, conflicts_with_all = ["standard", "pro"])]
    fast: bool,
    #[arg(long, conflicts_with_all = ["fast", "pro"])]
    standard: bool,
    #[arg(long, conflicts_with_all = ["fast", "standard"])]
    pro: bool,
    #[arg(long, conflicts_with_all = ["hd", "uhd", "resolution"])]
    small: bool,
    #[arg(long, conflicts_with_all = ["small", "uhd", "resolution"])]
    hd: bool,
    #[arg(long, conflicts_with_all = ["small", "hd", "resolution"])]
    uhd: bool,
    #[arg(long, conflicts_with_all = ["wide", "tall", "portrait", "landscape", "aspect_ratio"])]
    square: bool,
    #[arg(long, conflicts_with_all = ["square", "tall", "portrait", "landscape", "aspect_ratio"])]
    wide: bool,
    #[arg(long, conflicts_with_all = ["square", "wide", "portrait", "landscape", "aspect_ratio"])]
    tall: bool,
    #[arg(long, conflicts_with_all = ["square", "wide", "tall", "landscape", "aspect_ratio"])]
    portrait: bool,
    #[arg(long, conflicts_with_all = ["square", "wide", "tall", "portrait", "aspect_ratio"])]
    landscape: bool,
    #[arg(long)]
    model: Option<String>,
    #[arg(long)]
    resolution: Option<String>,
    #[arg(long)]
    aspect_ratio: Option<String>,
    #[arg(long, default_value = "png")]
    output_format: String,
    #[arg(short = 'o', long, default_value = "generated-image.png")]
    output: PathBuf,
    #[arg(long, default_value_t = 1)]
    count: u32,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    base_url: Option<String>,
    #[arg(long)]
    api_key: Option<String>,
    #[arg(long, default_value_t = 900)]
    timeout: u64,
    #[arg(long, default_value_t = 0)]
    retries: u32,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args)]
pub struct FriendlyMake {
    prompt: String,
    #[command(flatten)]
    flags: FriendlyFlags,
}

#[derive(Debug, Args)]
pub struct FriendlyEdit {
    image: String,
    prompt: String,
    #[command(flatten)]
    flags: FriendlyFlags,
}

#[derive(Debug, Args)]
pub struct FriendlyCompose {
    #[arg(long = "image", required = true)]
    images: Vec<String>,
    #[arg(long)]
    prompt: String,
    #[command(flatten)]
    flags: FriendlyFlags,
}

#[derive(Debug, Args)]
pub struct GenerateArgs {
    #[arg(long)]
    prompt: String,
    #[arg(long = "image")]
    images: Vec<String>,
    #[arg(long)]
    mask: Option<String>,
    #[arg(long)]
    model: Option<String>,
    #[arg(long, default_value = "2K")]
    resolution: String,
    #[arg(long, default_value = "1:1")]
    aspect_ratio: String,
    #[arg(long, default_value = "png")]
    output_format: String,
    #[arg(short = 'o', long, default_value = "generated-image.png")]
    output: PathBuf,
    #[arg(long, default_value_t = 1)]
    count: u32,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    base_url: Option<String>,
    #[arg(long)]
    api_key: Option<String>,
    #[arg(long, default_value_t = 900)]
    timeout: u64,
    #[arg(long, default_value_t = 0)]
    retries: u32,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args)]
pub struct ConfigCommand {
    #[command(subcommand)]
    command: ConfigSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ConfigSubcommand {
    Write(ConfigWriteArgs),
}

#[derive(Debug, Args)]
pub struct ConfigWriteArgs {
    #[arg(long)]
    provider: String,
    #[arg(long)]
    base_url: String,
    #[arg(long, conflicts_with_all = ["api_key_env", "api_key_stdin"])]
    api_key: Option<String>,
    #[arg(long, conflicts_with_all = ["api_key", "api_key_stdin"])]
    api_key_env: Option<String>,
    #[arg(long, conflicts_with_all = ["api_key", "api_key_env"])]
    api_key_stdin: bool,
    #[arg(long)]
    config: Option<PathBuf>,
}

pub async fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    let value = run(cli).await?;
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

pub async fn run(cli: Cli) -> Result<serde_json::Value> {
    match cli.command {
        Commands::Make(args) => {
            run_request(
                friendly_to_request(Operation::Generate, args.prompt, vec![], None, args.flags)
                    .await?,
            )
            .await
        }
        Commands::Edit(args) => {
            run_request(
                friendly_to_request(
                    Operation::Edit,
                    args.prompt,
                    vec![args.image],
                    None,
                    args.flags,
                )
                .await?,
            )
            .await
        }
        Commands::Compose(args) => {
            run_request(
                friendly_to_request(
                    Operation::Compose,
                    args.prompt,
                    args.images,
                    None,
                    args.flags,
                )
                .await?,
            )
            .await
        }
        Commands::Generate(args) => {
            let operation = if args.images.is_empty() {
                Operation::Generate
            } else if args.images.len() == 1 {
                Operation::Edit
            } else {
                Operation::Compose
            };
            run_request(generate_to_request(operation, args).await?).await
        }
        Commands::Doctor(args) => doctor(args.config.as_deref()),
        Commands::Models => Ok(models_json()),
        Commands::Presets => Ok(presets_json()),
        Commands::Config(config) => match config.command {
            ConfigSubcommand::Write(args) => {
                let provider = parse_provider(&args.provider)?;
                let api_key = resolve_config_api_key(&args)?;
                let path = write_provider_config(
                    args.config.as_deref(),
                    provider.clone(),
                    &args.base_url,
                    &api_key,
                )?;
                Ok(json!({
                    "ok": true,
                    "operation": "config-write",
                    "provider": provider.key(),
                    "config_path": path,
                }))
            }
        },
    }
}

fn resolve_config_api_key(args: &ConfigWriteArgs) -> Result<String> {
    let value = if let Some(value) = &args.api_key {
        value.clone()
    } else if let Some(name) = &args.api_key_env {
        std::env::var(name).with_context(|| format!("failed to read API key env var {name}"))?
    } else if args.api_key_stdin {
        let mut value = String::new();
        std::io::stdin()
            .read_to_string(&mut value)
            .context("failed to read API key from stdin")?;
        value.trim_end_matches(['\r', '\n']).to_string()
    } else {
        bail!("missing apiKey; pass --api-key, --api-key-env, or --api-key-stdin")
    };
    crate::config::validate_config_value("apiKey", &value)?;
    Ok(value)
}

async fn friendly_to_request(
    operation: Operation,
    prompt: String,
    images: Vec<String>,
    mask: Option<String>,
    flags: FriendlyFlags,
) -> Result<(
    ImageRequest,
    OutputPlan,
    Config,
    Option<String>,
    Option<String>,
)> {
    let config = load_effective_config(flags.config.as_deref())?;
    let model = friendly_model(&flags, &config)?;
    let resolution = friendly_resolution(&flags)?;
    let aspect_ratio = friendly_aspect_ratio(&flags)?;
    let request = ImageRequest {
        operation,
        prompt,
        input_images: load_inputs(&images, flags.timeout).await?,
        mask: load_optional_input(mask.as_deref(), flags.timeout).await?,
        model,
        resolution,
        aspect_ratio,
        output_format: OutputFormat::parse(&flags.output_format)?,
        count: flags.count,
    };
    let plan = OutputPlan {
        output: flags.output,
        dry_run: flags.dry_run,
        timeout_seconds: flags.timeout,
        retries: flags.retries,
    };
    Ok((request, plan, config, flags.base_url, flags.api_key))
}

async fn generate_to_request(
    operation: Operation,
    args: GenerateArgs,
) -> Result<(
    ImageRequest,
    OutputPlan,
    Config,
    Option<String>,
    Option<String>,
)> {
    let config = load_effective_config(args.config.as_deref())?;
    let model_name = args
        .model
        .as_deref()
        .or(config.default_model.as_deref())
        .unwrap_or("nano-banana-2");
    let request = ImageRequest {
        operation,
        prompt: args.prompt,
        input_images: load_inputs(&args.images, args.timeout).await?,
        mask: load_optional_input(args.mask.as_deref(), args.timeout).await?,
        model: resolve_model(model_name)?,
        resolution: Resolution::parse(&args.resolution)?,
        aspect_ratio: AspectRatio::parse(&args.aspect_ratio)?,
        output_format: OutputFormat::parse(&args.output_format)?,
        count: args.count,
    };
    let plan = OutputPlan {
        output: args.output,
        dry_run: args.dry_run,
        timeout_seconds: args.timeout,
        retries: args.retries,
    };
    Ok((request, plan, config, args.base_url, args.api_key))
}

fn friendly_model(
    flags: &FriendlyFlags,
    config: &Config,
) -> Result<crate::provider::ResolvedModel> {
    if let Some(model) = &flags.model {
        return resolve_model(model);
    }
    if flags.pro {
        return resolve_model("nano-banana-pro");
    }
    if flags.fast {
        return resolve_model("nano-banana-2");
    }
    if let Some(model) = config.default_model.as_deref() {
        return resolve_model(model);
    }
    resolve_model("nano-banana-2")
}

fn friendly_resolution(flags: &FriendlyFlags) -> Result<Resolution> {
    if let Some(value) = &flags.resolution {
        return Resolution::parse(value);
    }
    if flags.small || flags.fast {
        Ok(Resolution::Small1K)
    } else if flags.uhd || flags.pro {
        Ok(Resolution::Uhd4K)
    } else {
        Ok(Resolution::Hd2K)
    }
}

fn friendly_aspect_ratio(flags: &FriendlyFlags) -> Result<AspectRatio> {
    if let Some(value) = &flags.aspect_ratio {
        return AspectRatio::parse(value);
    }
    if flags.wide {
        Ok(AspectRatio::Wide16x9)
    } else if flags.tall {
        Ok(AspectRatio::Tall9x16)
    } else if flags.portrait {
        Ok(AspectRatio::Portrait3x4)
    } else if flags.landscape {
        Ok(AspectRatio::Landscape4x3)
    } else {
        Ok(AspectRatio::Square1x1)
    }
}

async fn run_request(
    (request, plan, config, cli_base_url, cli_api_key): (
        ImageRequest,
        OutputPlan,
        Config,
        Option<String>,
        Option<String>,
    ),
) -> Result<serde_json::Value> {
    if request.count < 1 {
        bail!("--count must be at least 1");
    }
    if plan.dry_run {
        return Ok(report(&request, &plan, Vec::new(), true));
    }

    let provider_config = effective_provider_config(&request, &config, cli_base_url, cli_api_key)?;
    let base_url = provider_config
        .base_url
        .as_deref()
        .context("missing baseUrl; set provider config or pass --base-url")?;
    let api_key = provider_config
        .api_key
        .as_deref()
        .context("missing apiKey; set provider config or pass --api-key")?;
    validate_https_base_url(base_url)?;
    let client = Client::builder()
        .timeout(Duration::from_secs(plan.timeout_seconds))
        .build()?;
    let images = match request.model.provider {
        ProviderKind::OpenAi => {
            call_openai(&client, base_url, api_key, &request, plan.retries).await?
        }
        ProviderKind::Google => {
            call_google(&client, base_url, api_key, &request, plan.retries).await?
        }
    };
    let outputs = write_images(&plan.output, &images, request.output_format)?;
    Ok(report(&request, &plan, outputs, false))
}

fn effective_provider_config(
    request: &ImageRequest,
    config: &Config,
    cli_base_url: Option<String>,
    cli_api_key: Option<String>,
) -> Result<ProviderConfig> {
    let mut provider = config.provider(request.model.provider.clone());
    if provider
        .base_url
        .as_ref()
        .map(|v| v.trim().is_empty())
        .unwrap_or(true)
    {
        provider.base_url = Some(default_base_url(request.model.provider.clone()).to_string());
    }
    if let Some(value) = cli_base_url {
        provider.base_url = Some(value);
    }
    if let Some(value) = cli_api_key {
        provider.api_key = Some(value);
    }
    Ok(provider)
}

async fn call_openai(
    client: &Client,
    base_url: &str,
    api_key: &str,
    request: &ImageRequest,
    retries: u32,
) -> Result<Vec<Vec<u8>>> {
    retry_provider_call(retries, || async {
        call_openai_once(client, base_url, api_key, request).await
    })
    .await
}

async fn call_openai_once(
    client: &Client,
    base_url: &str,
    api_key: &str,
    request: &ImageRequest,
) -> Result<Vec<Vec<u8>>, ProviderCallError> {
    let url = if request.input_images.is_empty() {
        format!("{}/v1/images/generations", base_url.trim_end_matches('/'))
    } else {
        format!("{}/v1/images/edits", base_url.trim_end_matches('/'))
    };
    let response = if request.input_images.is_empty() {
        client
            .post(url)
            .bearer_auth(api_key)
            .json(&build_generation_request(request).map_err(ProviderCallError::non_retryable)?)
            .send()
            .await
            .map_err(ProviderCallError::retryable)?
    } else {
        client
            .post(url)
            .bearer_auth(api_key)
            .multipart(build_edit_form(request).map_err(ProviderCallError::non_retryable)?)
            .send()
            .await
            .map_err(ProviderCallError::retryable)?
    };
    let status = response.status();
    let value = response_json(response, "OpenAI").await?;
    if !status.is_success() {
        return Err(ProviderCallError::from_status(
            status.as_u16(),
            anyhow::anyhow!("OpenAI API request failed with HTTP {status}: {value}"),
        ));
    }
    parse_openai_images(&value).map_err(ProviderCallError::non_retryable)
}

async fn call_google(
    client: &Client,
    base_url: &str,
    api_key: &str,
    request: &ImageRequest,
    retries: u32,
) -> Result<Vec<Vec<u8>>> {
    retry_provider_call(retries, || async {
        call_google_once(client, base_url, api_key, request).await
    })
    .await
}

async fn call_google_once(
    client: &Client,
    base_url: &str,
    api_key: &str,
    request: &ImageRequest,
) -> Result<Vec<Vec<u8>>, ProviderCallError> {
    let url = format!("{}/v1beta/interactions", base_url.trim_end_matches('/'));
    let response = client
        .post(url)
        .header(api_key_header_name(), api_key)
        .json(&build_interactions_request(request).map_err(ProviderCallError::non_retryable)?)
        .send()
        .await
        .map_err(ProviderCallError::retryable)?;
    let status = response.status();
    let value = response_json(response, "Google").await?;
    if status.as_u16() == 404 && value.to_string().contains("Invalid URL") {
        return call_google_gemini_once(client, base_url, api_key, request).await;
    }
    if !status.is_success() {
        return Err(ProviderCallError::from_status(
            status.as_u16(),
            anyhow::anyhow!("Google API request failed with HTTP {status}: {value}"),
        ));
    }
    parse_google_images(&value).map_err(ProviderCallError::non_retryable)
}

async fn call_google_gemini_once(
    client: &Client,
    base_url: &str,
    api_key: &str,
    request: &ImageRequest,
) -> Result<Vec<Vec<u8>>, ProviderCallError> {
    let url = format!(
        "{}/v1beta/models/{}:generateContent",
        base_url.trim_end_matches('/'),
        request.model.wire_model
    );
    let response = client
        .post(url)
        .header(api_key_header_name(), api_key)
        .json(
            &build_gemini_generate_content_request(request)
                .map_err(ProviderCallError::non_retryable)?,
        )
        .send()
        .await
        .map_err(ProviderCallError::retryable)?;
    let status = response.status();
    let value = response_json(response, "Google Gemini").await?;
    if !status.is_success() {
        return Err(ProviderCallError::from_status(
            status.as_u16(),
            anyhow::anyhow!("Google Gemini API request failed with HTTP {status}: {value}"),
        ));
    }
    parse_google_images(&value).map_err(ProviderCallError::non_retryable)
}

async fn response_json(
    response: reqwest::Response,
    provider_name: &str,
) -> Result<serde_json::Value, ProviderCallError> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(ProviderCallError::retryable)?;
    match serde_json::from_str(&body) {
        Ok(value) => Ok(value),
        Err(error) if status.is_success() => Err(ProviderCallError::non_retryable(
            anyhow::anyhow!("{provider_name} API response was not valid JSON: {error}"),
        )),
        Err(_) => Ok(json!({ "body": body })),
    }
}

struct ProviderCallError {
    retryable: bool,
    error: anyhow::Error,
}

impl ProviderCallError {
    fn retryable(error: impl Into<anyhow::Error>) -> Self {
        Self {
            retryable: true,
            error: error.into(),
        }
    }

    fn non_retryable(error: impl Into<anyhow::Error>) -> Self {
        Self {
            retryable: false,
            error: error.into(),
        }
    }

    fn from_status(status: u16, error: anyhow::Error) -> Self {
        Self {
            retryable: is_retryable_http_status(status),
            error,
        }
    }
}

async fn retry_provider_call<F, Fut>(retries: u32, mut call: F) -> Result<Vec<Vec<u8>>>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<Vec<Vec<u8>>, ProviderCallError>>,
{
    let attempts = retries.saturating_add(1);
    for attempt in 0..attempts {
        match call().await {
            Ok(images) => return Ok(images),
            Err(error) if error.retryable && attempt + 1 < attempts => {
                tokio::time::sleep(retry_delay(attempt)).await;
            }
            Err(error) => return Err(error.error),
        }
    }
    unreachable!("attempts always contains at least one iteration")
}

fn retry_delay(attempt: u32) -> Duration {
    Duration::from_secs(2_u64.saturating_pow(attempt.min(4)))
}

fn report(
    request: &ImageRequest,
    plan: &OutputPlan,
    outputs: Vec<PathBuf>,
    dry_run: bool,
) -> serde_json::Value {
    let outputs: Vec<String> = outputs
        .into_iter()
        .map(|path| path.display().to_string())
        .collect();
    let report = RunReport {
        ok: true,
        operation: request.operation.as_str().to_string(),
        provider: request.model.provider.key().to_string(),
        model: request.model.user_model.clone(),
        wire_model: request.model.wire_model.clone(),
        resolution: request.resolution.as_str().to_string(),
        aspect_ratio: request.aspect_ratio.as_str().to_string(),
        output_format: request.output_format.as_str().to_string(),
        requested_count: request.count,
        returned_count: outputs.len(),
        outputs,
        dry_run,
        timeout_seconds: plan.timeout_seconds,
        retries: plan.retries,
    };
    let mut value = serde_json::to_value(report).expect("serializing report cannot fail");
    if dry_run {
        value["would_write"] = json!(plan.output);
    }
    value
}

async fn load_inputs(sources: &[String], timeout_seconds: u64) -> Result<Vec<ImageInput>> {
    let client = if sources.iter().any(|source| is_remote_source(source)) {
        Some(remote_image_client(timeout_seconds)?)
    } else {
        None
    };
    let mut inputs = Vec::with_capacity(sources.len());
    for source in sources {
        inputs.push(load_input(source, client.as_ref()).await?);
    }
    Ok(inputs)
}

async fn load_optional_input(
    source: Option<&str>,
    timeout_seconds: u64,
) -> Result<Option<ImageInput>> {
    let Some(source) = source else {
        return Ok(None);
    };
    let client = if is_remote_source(source) {
        Some(remote_image_client(timeout_seconds)?)
    } else {
        None
    };
    load_input(source, client.as_ref()).await.map(Some)
}

async fn load_input(source: &str, client: Option<&Client>) -> Result<ImageInput> {
    if is_remote_source(source) {
        let client = client.context("remote image download client was not initialized")?;
        let (data, mime_type) = download_remote_image_with_client(client, source).await?;
        let filename = Url::parse(source)
            .ok()
            .and_then(|url| {
                url.path_segments()
                    .and_then(|mut segments| segments.next_back().map(str::to_string))
            })
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "image.png".to_string());
        return Ok(ImageInput {
            source: source.to_string(),
            data,
            mime_type,
            filename,
        });
    }
    let path = Path::new(source);
    let data = fs::read(path).with_context(|| format!("failed to read input image {source}"))?;
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("image.png")
        .to_string();
    Ok(ImageInput {
        source: source.to_string(),
        data,
        mime_type: guess_mime_type(source),
        filename,
    })
}

fn is_remote_source(source: &str) -> bool {
    source.starts_with("http://") || source.starts_with("https://")
}

fn validate_https_base_url(base_url: &str) -> Result<()> {
    let parsed = Url::parse(base_url).with_context(|| "baseUrl must be a valid URL")?;
    if parsed.scheme() != "https" {
        bail!("baseUrl must use HTTPS");
    }
    if parsed.username() != "" || parsed.password().is_some() {
        bail!("baseUrl must not include credentials");
    }
    if parsed.host_str().unwrap_or("").trim().is_empty() {
        bail!("baseUrl must include a hostname");
    }
    Ok(())
}

fn parse_provider(value: &str) -> Result<ProviderKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "openai" => Ok(ProviderKind::OpenAi),
        "google" => Ok(ProviderKind::Google),
        _ => bail!("unknown provider {value:?}; choose openai or google"),
    }
}

fn doctor(path: Option<&Path>) -> Result<serde_json::Value> {
    let config = load_effective_config(path)?;
    let openai = config.provider(ProviderKind::OpenAi);
    let google = config.provider(ProviderKind::Google);
    Ok(json!({
        "ok": true,
        "operation": "doctor",
        "default_model": config.default_model,
        "providers": {
            "openai": {
                "base_url_configured": openai.base_url.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false),
                "api_key_configured": openai.api_key.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false)
            },
            "google": {
                "base_url_configured": google.base_url.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false),
                "api_key_configured": google.api_key.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false)
            }
        }
    }))
}

fn models_json() -> serde_json::Value {
    json!({
        "ok": true,
        "models": [
            {"model": "nano-banana-pro", "provider": "google", "wire_model": "gemini-3-pro-image"},
            {"model": "nano-banana-2", "provider": "google", "wire_model": "gemini-3.1-flash-image"},
            {"model": "gpt-image-2", "provider": "openai", "wire_model": "gpt-image-2"}
        ]
    })
}

fn presets_json() -> serde_json::Value {
    json!({
        "ok": true,
        "presets": {
            "fast": {"model": "nano-banana-2", "resolution": "1K"},
            "standard": {"model": "nano-banana-2", "resolution": "2K"},
            "pro": {"model": "nano-banana-pro", "resolution": "4K"}
        },
        "shapes": {
            "square": "1:1",
            "wide": "16:9",
            "tall": "9:16",
            "portrait": "3:4",
            "landscape": "4:3"
        }
    })
}
