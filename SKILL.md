---
name: imagegenexpert
description: Use when generating, editing, enhancing, composing, or saving AI images with gpt-image-2, nano-banana-2, or nano-banana-pro; when users mention image generation, posters, avatars, logos, product images, 生成图片, 生图, 画图, 海报, 头像, 修图, 图片合成; or when users explicitly invoke $imagegenexpert for guided expert parameter selection.
---

# ImageGen Expert

这个 skill 通过随 skill 发布包内置的 `bin/imagegen` 生成、编辑和合成图片。用户项目目录只作为输入/输出工作区，不需要放置二进制或 Rust 开发环境。

## 运行时定位

先定位命令，再做任何配置或生图操作：

```bash
# SKILL_DIR 是本 SKILL.md 所在的 skill 安装目录。
IMAGEGEN="$SKILL_DIR/bin/imagegen"
```

如果 skill 安装目录下的 `bin/imagegen` 不存在或不可执行，可退回到 `command -v imagegen` 找到的全局命令。只有维护这个仓库本身时，才允许用源码开发命令作为兜底；普通用户项目不得假设存在 Cargo、源码目录或项目本地二进制。

不要输出、复述或猜测 API key。

## 调用前检查

每次真实调用模型前先运行：

```bash
"$IMAGEGEN" doctor
```

`doctor` 会返回 provider 配置状态。模型和 provider 对应关系：

- `nano-banana-2`、`nano-banana-pro` 使用 `google`
- `gpt-image-2` 使用 `openai`

如果所选模型对应 provider 未配置 API key，先进入配置问答，不要直接执行生图命令。

## 友好配置问答

缺配置时一次只问一个问题：

1. 选择 provider：`google` 或 `openai`。如果用户已选 `nano-banana-*`，默认 `google`；已选 `gpt-image-2`，默认 `openai`。
2. 选择接口地址：
   - Google 官方：`https://generativelanguage.googleapis.com`
   - OpenAI 官方：`https://api.openai.com`
   - 自定义中转：请用户提供 base URL
3. 让用户提供 API key。不要复述 key，不要把 key 放进最终回复。
4. 用 stdin 写入配置，避免把 key 放在命令行参数里：

```bash
"$IMAGEGEN" config write \
  --provider google \
  --base-url https://generativelanguage.googleapis.com \
  --api-key-stdin
```

5. 配置后再次运行 `"$IMAGEGEN" doctor`。对应 provider 已配置后，再回到原来的参数问答或生图流程。

如果当前工具环境无法安全地把用户提供的 key 写入 stdin，就给用户一条带占位符的命令让用户本地执行；占位符必须是 `YOUR_API_KEY`，不要替用户填充或展示真实 key。

## 普通模式

普通用户请求直接选择友好预设，少问参数，但仍先完成运行时定位和 `doctor` 检查：

```bash
"$IMAGEGEN" make "高端茶饮品牌门店海报" --pro --wide -o outputs/poster.png
"$IMAGEGEN" edit input.png "提升清晰度，保持自然真实" --hd -o outputs/restored.png
"$IMAGEGEN" compose --image product.png --image scene.png --prompt "把产品自然放入场景" --pro --wide -o outputs/composite.png
```

常用预设：

- `--fast`: 快速预览，`nano-banana-2`，`1K`
- `--standard`: 常规成图，`nano-banana-2`，`2K`
- `--pro`: 高质量交付，`nano-banana-pro`，`4K`
- `--small` / `--hd` / `--uhd`: `1K` / `2K` / `4K`
- `--square` / `--wide` / `--tall`: `1:1` / `16:9` / `9:16`
- `--portrait` / `--landscape`: `3:4` / `4:3`

第三方中转站生图可能很慢，默认 `--timeout 900`。默认不要加 `--retries`，因为超时不一定代表上游失败，重复请求可能造成重复扣费；只有用户明确要求重试时才使用。

## 专家问答模式

用户显式输入 `$imagegenexpert ...` 时，进入问答选择模式，不要立刻执行长命令。

流程：

1. 先从用户文本提取明确参数。
2. 定位 `IMAGEGEN` 并运行 `doctor`，记录 provider 配置状态。
3. 一次只问一个问题。
4. 使用编号选项，推荐项排第一。
5. 已明确的参数不要再问。
6. 确定模型后，如果对应 provider 未配置，先走“友好配置问答”。
7. 最后展示完整命令。
8. 只有用户回复 `yes`、`确认` 或同义明确确认后才执行。

问题顺序：

1. 操作：生成新图、编辑单图、多图合成。
2. 模型：`nano-banana-pro`、`nano-banana-2`、`gpt-image-2`。
3. 清晰度：`1K`、`2K`、`4K`。
4. 画幅：`1:1`、`16:9`、`9:16`、`3:2`、`2:3`、`4:3`、`3:4`。
5. 输出格式：`png`、`jpeg`、`webp`。
6. 数量：默认 `1`，需要多版本时可选 `2-4`。
7. 输出路径。
8. 最终确认。

示例用户输入：

```text
$imagegenexpert 生成一张 4K 横版产品海报，使用高质量模型
```

建议回复：

```text
我识别到：生成新图、4K、横版、高质量。
请选择模型：
1. nano-banana-pro（推荐，高质量商业图、产品海报）
2. nano-banana-2（速度更快，适合草稿和常规图）
3. gpt-image-2（使用 OpenAI Images API）
```

最终确认格式：

```text
即将执行：
"$IMAGEGEN" generate \
  --model nano-banana-pro \
  --prompt "生成一张 4K 横版产品海报，使用高质量模型" \
  --resolution 4K \
  --aspect-ratio 16:9 \
  --output-format png \
  --timeout 900 \
  --output outputs/product-poster.png

确认执行吗？yes/no
```

## 专家命令

问答最终映射到：

```bash
"$IMAGEGEN" generate \
  --model nano-banana-pro \
  --prompt "高端产品海报" \
  --resolution 4K \
  --aspect-ratio 16:9 \
  --output-format png \
  --timeout 900 \
  --output outputs/product-poster.png
```

支持模型：

- `nano-banana-pro`: Google provider，wire model `gemini-3-pro-image`
- `nano-banana-2`: Google provider，wire model `gemini-3.1-flash-image`
- `gpt-image-2`: OpenAI provider

## 输出约定

CLI 输出 JSON。成功后向用户报告保存路径：

```json
{
  "ok": true,
  "operation": "generate",
  "provider": "google",
  "model": "nano-banana-pro",
  "resolution": "4K",
  "aspect_ratio": "16:9",
  "outputs": ["outputs/product-poster.png"]
}
```

模型返回 base64 图片时，CLI 会解码并写入文件。多图输出会在扩展名前追加 `_01`、`_02`。

## 错误处理

- 缺少 API key：进入友好配置问答。
- 参数不支持：给出允许选项。
- Provider 错误：概括 HTTP 状态和错误信息，不泄露密钥。
- 超时：先说明中转站可能很慢，建议加大 `--timeout`，不要直接建议重试。
- 普通模式请求含糊：做合理默认选择。
- 专家模式请求含糊：继续问下一个参数问题。
