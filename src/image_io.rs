use crate::provider::OutputFormat;
use anyhow::{anyhow, bail, Context, Result};
use base64::prelude::*;
use reqwest::redirect::{Attempt, Policy};
use reqwest::Client;
use std::fs;
use std::net::{IpAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::Duration;
use url::Url;

pub fn decode_base64_image(value: &str) -> Result<Vec<u8>> {
    let data = value
        .split_once(',')
        .filter(|(prefix, _)| prefix.to_ascii_lowercase().contains(";base64"))
        .map(|(_, data)| data)
        .unwrap_or(value)
        .trim();
    BASE64_STANDARD
        .decode(data)
        .with_context(|| "image response contained invalid base64")
}

pub fn output_path_for_index(output: &Path, index: usize, total: usize) -> PathBuf {
    if total <= 1 {
        return output.to_path_buf();
    }
    let parent = output.parent().unwrap_or_else(|| Path::new(""));
    let stem = output
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let extension = output.extension().and_then(|value| value.to_str());
    let filename = match extension {
        Some(ext) if !ext.is_empty() => format!("{stem}_{:02}.{ext}", index + 1),
        _ => format!("{stem}_{:02}", index + 1),
    };
    parent.join(filename)
}

pub fn ensure_output_extension(path: &Path, format: OutputFormat) -> PathBuf {
    if path.extension().is_some() {
        path.to_path_buf()
    } else {
        path.with_extension(format.extension())
    }
}

pub fn write_images(
    output: &Path,
    images: &[Vec<u8>],
    output_format: OutputFormat,
) -> Result<Vec<PathBuf>> {
    if images.is_empty() {
        bail!("provider response did not include any images");
    }
    let output = ensure_output_extension(output, output_format);
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create output directory {}", parent.display())
            })?;
        }
    }

    let mut paths = Vec::with_capacity(images.len());
    for (index, bytes) in images.iter().enumerate() {
        let path = output_path_for_index(&output, index, images.len());
        fs::write(&path, convert_if_needed(bytes, output_format)?)
            .with_context(|| format!("failed to write image {}", path.display()))?;
        paths.push(path);
    }
    Ok(paths)
}

fn convert_if_needed(bytes: &[u8], output_format: OutputFormat) -> Result<Vec<u8>> {
    if output_format == OutputFormat::Png && bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(bytes.to_vec());
    }
    if output_format == OutputFormat::Jpeg && bytes.starts_with(b"\xff\xd8") {
        return Ok(bytes.to_vec());
    }
    if output_format == OutputFormat::Webp && bytes.starts_with(b"RIFF") {
        return Ok(bytes.to_vec());
    }

    let image = image::load_from_memory(bytes)
        .map_err(|err| anyhow!("failed to decode provider image for conversion: {err}"))?;
    let mut cursor = std::io::Cursor::new(Vec::new());
    match output_format {
        OutputFormat::Png => image.write_to(&mut cursor, image::ImageFormat::Png)?,
        OutputFormat::Jpeg => image.write_to(&mut cursor, image::ImageFormat::Jpeg)?,
        OutputFormat::Webp => image.write_to(&mut cursor, image::ImageFormat::WebP)?,
    }
    Ok(cursor.into_inner())
}

pub fn guess_mime_type(path_or_url: &str) -> String {
    let lower = path_or_url.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png".to_string()
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg".to_string()
    } else if lower.ends_with(".webp") {
        "image/webp".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}

pub fn remote_image_client(timeout_seconds: u64) -> Result<Client> {
    Ok(Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .redirect(Policy::custom(validate_redirect))
        .build()?)
}

pub async fn download_remote_image(
    source: &str,
    timeout_seconds: u64,
) -> Result<(Vec<u8>, String)> {
    let client = remote_image_client(timeout_seconds)?;
    download_remote_image_with_client(&client, source).await
}

pub async fn download_remote_image_with_client(
    client: &Client,
    source: &str,
) -> Result<(Vec<u8>, String)> {
    validate_remote_image_url(source, true)?;
    let response = client
        .get(source)
        .header(
            reqwest::header::ACCEPT,
            "image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
        )
        .send()
        .await
        .with_context(|| format!("failed to download remote image {source}"))?;
    let status = response.status();
    if !status.is_success() {
        bail!("image download failed with HTTP {status}: {source}");
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .split(';')
        .next()
        .unwrap_or("application/octet-stream")
        .trim()
        .to_string();
    if !content_type.starts_with("image/") {
        bail!("remote image response Content-Type is not image/*: {content_type}");
    }
    let bytes = response.bytes().await?.to_vec();
    Ok((bytes, content_type))
}

fn validate_redirect(attempt: Attempt<'_>) -> reqwest::redirect::Action {
    if attempt.previous().len() >= 5 {
        return attempt.stop();
    }
    if validate_remote_image_url(attempt.url().as_str(), true).is_ok() {
        attempt.follow()
    } else {
        attempt.stop()
    }
}

pub fn validate_remote_image_url(value: &str, resolve_dns: bool) -> Result<Url> {
    let url = Url::parse(value).with_context(|| "image URL must be a valid URL")?;
    if url.scheme() != "https" {
        bail!("image URL must use HTTPS");
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("image URL must not include credentials");
    }
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("image URL must include a hostname"))?;
    validate_host(
        host,
        url.port_or_known_default().unwrap_or(443),
        resolve_dns,
    )?;
    Ok(url)
}

fn validate_host(host: &str, port: u16, resolve_dns: bool) -> Result<()> {
    let normalized = host
        .trim_matches(['[', ']'])
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if normalized.is_empty() {
        bail!("image URL must include a hostname");
    }
    if normalized == "localhost" || normalized.ends_with(".localhost") {
        bail!("image URL must not use localhost");
    }
    if normalized.ends_with(".local") {
        bail!("image URL must not use .local hostnames");
    }
    if let Ok(ip) = normalized.parse::<IpAddr>() {
        if let Some(reason) = unsafe_ip_reason(ip) {
            bail!("image URL host is blocked because it is a {reason}: {ip}");
        }
        return Ok(());
    }
    if resolve_dns {
        let resolved = (normalized.as_str(), port)
            .to_socket_addrs()
            .with_context(|| format!("could not resolve image URL hostname {host:?}"))?;
        for address in resolved {
            let ip = address.ip();
            if let Some(reason) = unsafe_ip_reason(ip) {
                bail!("image URL hostname {host:?} resolves to blocked {reason}: {ip}");
            }
        }
    }
    Ok(())
}

fn unsafe_ip_reason(ip: IpAddr) -> Option<&'static str> {
    match ip {
        IpAddr::V4(ip) => {
            if ip.is_loopback() {
                Some("loopback address")
            } else if ip.is_private() {
                Some("private address")
            } else if ip.is_link_local() {
                Some("link-local address")
            } else if ip.is_multicast() {
                Some("multicast address")
            } else if ip.is_broadcast() {
                Some("broadcast address")
            } else if ip.is_documentation() {
                Some("documentation address")
            } else if ip.is_unspecified() {
                Some("unspecified address")
            } else {
                None
            }
        }
        IpAddr::V6(ip) => {
            if ip.is_loopback() {
                Some("loopback address")
            } else if ip.is_unique_local() {
                Some("private address")
            } else if ip.is_unicast_link_local() {
                Some("link-local address")
            } else if ip.is_multicast() {
                Some("multicast address")
            } else if ip.is_unspecified() {
                Some("unspecified address")
            } else {
                None
            }
        }
    }
}
