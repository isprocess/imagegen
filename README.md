# ImageGen Expert

ImageGen Expert 是面向 Codex 和 Claude Code 的 AI 生图 skill。发布包内置 `bin/imagegen`，用户项目目录只用于读取参考图和写输出图，不需要 Rust、Cargo、源码目录或项目本地二进制。

## 能力

- 支持 `gpt-image-2`、`nano-banana-2`、`nano-banana-pro`
- 支持生成新图、单图编辑、多图合成
- 支持 `1K`、`2K`、`4K`
- 支持 `png`、`jpeg`、`webp`
- 自动把模型返回的 base64 图片解码落盘
- 默认 `900` 秒超时，适合较慢的第三方中转站

## Runtime 发布包

维护者从源码仓库打包：

```bash
bash scripts/package_skill.sh dist/imagegenexpert
```

发布目录只包含：

```text
imagegenexpert/
  SKILL.md
  agents/openai.yaml
  bin/imagegen
```

不应包含 `Cargo.toml`、`src/`、`tests/`、`target/`、`temp/`、`docs/` 等开发环境文件。

把 `imagegenexpert/` 放到对应 agent 的 skills 目录后即可使用。Windows 包内二进制命名可为 `bin/imagegen.exe`，skill 运行时仍应从 skill 安装目录定位二进制，而不是从用户项目目录定位。

## 配置

Agent 唤起 skill 后应先运行：

```bash
bin/imagegen doctor
```

如果 provider 未配置，skill 会进入友好问答模式，引导选择 provider、官方接口或自定义中转，并写入配置。

仍然可以手动配置：

```bash
imagegen config write --provider google --base-url https://generativelanguage.googleapis.com --api-key "$GEMINI_API_KEY"
imagegen config write --provider openai --base-url https://api.openai.com --api-key "$OPENAI_API_KEY"
```

为了避免把 key 放进命令行历史或进程列表，推荐使用 stdin：

```bash
imagegen config write --provider google --base-url https://generativelanguage.googleapis.com --api-key-stdin
```

也可以从环境变量读取：

```bash
imagegen config write --provider google --base-url https://generativelanguage.googleapis.com --api-key-env IMAGEGEN_GOOGLE_API_KEY
```

环境变量仍受支持：

```bash
export IMAGEGEN_GOOGLE_API_KEY="..."
export IMAGEGEN_OPENAI_API_KEY="..."
export IMAGEGEN_GOOGLE_BASE_URL="https://your-relay.example"
export IMAGEGEN_OPENAI_BASE_URL="https://your-relay.example"
export IMAGEGEN_DEFAULT_MODEL="nano-banana-pro"
```

## 快速使用

普通模式：

```bash
imagegen make "高端产品海报，摄影棚灯光，白底" --pro --wide -o outputs/poster.png
imagegen edit input.png "增强清晰度，修复噪点，保持真实质感" --hd -o outputs/restored.png
imagegen compose --image product.png --image background.png --prompt "把产品自然放入背景中" --pro --wide -o outputs/composite.png
```

专家模式：

```bash
imagegen generate \
  --model nano-banana-pro \
  --prompt "生成一张 4K 横版产品海报，使用高质量模型" \
  --resolution 4K \
  --aspect-ratio 16:9 \
  --output-format png \
  --timeout 900 \
  --output outputs/product-poster.png
```

参数预览，不调用模型：

```bash
imagegen make "科技产品海报" --pro --wide --dry-run
```

## Codex 和 Claude Code 交互

Codex：

```text
$imagegenexpert 生成一张 4K 横版产品海报，使用高质量模型
```

Claude Code：

```text
/imagegenexpert 生成一张 4K 横版产品海报，使用高质量模型
```

Agent 应先定位 skill 内置二进制，运行 `doctor`，在配置可用后按顺序问模型、清晰度、比例、格式、数量、输出路径，并在最终命令展示后等待确认。

## 模型和接口

- `gpt-image-2`: OpenAI Images API，默认 `https://api.openai.com`
- `nano-banana-2`: Google provider，wire model `gemini-3.1-flash-image`
- `nano-banana-pro`: Google provider，wire model `gemini-3-pro-image`

Google provider 优先调用 `/v1beta/interactions`，遇到不支持该路径的中转时会 fallback 到 Gemini `generateContent` 兼容路径。OpenAI provider 使用 `/v1/images/generations` / `/v1/images/edits`。

## 慢请求和重试

生图请求通常比文本模型慢，第三方中转站可能需要数分钟。默认超时是 `900` 秒。

默认 `--retries 0`。不要把超时自动重试作为默认行为，因为上游可能仍在生成，重复请求可能重复扣费。只有明确需要处理网络错误、429 或 5xx 时再设置：

```bash
imagegen generate ... --retries 2
```

## 源码维护

源码仓库需要 Rust 1.88 或更新版本：

```bash
cargo test
cargo build --release
```

源码维护者可以用 `cargo run -- ...` 调试 CLI。发布后的 skill 不依赖 Cargo，也不应要求用户项目存在源码仓库结构。
