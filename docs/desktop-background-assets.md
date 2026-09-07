# 桌面背景素材

2026-09-05 使用内置 `image_gen` 工具生成，未使用 CLI/API 方式。八套主题均有独立背景图片：暗色为暮色、雾蓝、石墨、星云，亮色为暖阳、晴空、薰衣草、青瓷。

### daylight.png

文件：`apps/stellatune/assets/images/backgrounds/daylight.png`。使用内置 `image_gen` 生成并保存到项目，最终提示词：

```text
Use case: photorealistic-natural. Asset type: full bleed background bitmap for a desktop music player, landscape 1536x1024. Create a deliberately defocused photographic atmosphere, bright and low contrast across the entire frame, suitable behind dark text and translucent light panels. Real optical softness and subtle organic light variations, not a flat digital gradient. No text, no interface, no people, no borders, no watermark. No sharp foreground details or dark vignette. Theme: clear airy daylight. Pale powder blue sky and wispy white clouds reflected in calm coastal water, horizon dissolving in bright haze. Cool blue-white palette with a hint of silver, high key, serene and spacious. Broad softly blurred forms.
```

### lavender.png

文件：`apps/stellatune/assets/images/backgrounds/lavender.png`。使用内置 `image_gen` 生成并保存到项目，最终提示词：

```text
Use case: photorealistic-natural. Asset type: full bleed background bitmap for a desktop music player, landscape 1536x1024. Create a deliberately defocused photographic atmosphere, bright and low contrast across the entire frame, suitable behind dark text and translucent light panels. Real optical softness and subtle organic light variations, not a flat digital gradient. No text, no interface, no people, no borders, no watermark. No sharp foreground details or dark vignette. Theme: lavender morning. A distant field of very pale lilac flowers in luminous mist, soft pearl-white sky, subtle dusty rose light. Low saturation lavender and ivory with barely visible blurred flower shapes at the bottom. Bright, gentle, airy, no deep purple.
```

### celadon.png

文件：`apps/stellatune/assets/images/backgrounds/celadon.png`。使用内置 `image_gen` 生成并保存到项目，最终提示词：

```text
Use case: photorealistic-natural. Asset type: full bleed background bitmap for a desktop music player, landscape 1536x1024. Create a deliberately defocused photographic atmosphere, bright and low contrast across the entire frame, suitable behind dark text and translucent light panels. Real optical softness and subtle organic light variations, not a flat digital gradient. No text, no interface, no people, no borders, no watermark. No sharp foreground details or dark vignette. Theme: celadon garden. Fresh pale sage green foliage beyond frosted glass in diffuse morning light, soft mint-white light and faint warm ivory reflections. Very blurred organic leaf silhouettes, restful, clean, bright and spacious. No dark forest, no yellow-green cast.
```


## 补充背景

雾蓝、石墨、星云素材同样使用内置 `image_gen` 工具生成。三张均为 1536×1024 PNG，已复制到项目的 `apps/stellatune/assets/images/backgrounds/`，不依赖生成工具的输出目录。

### mist.png

文件：`apps/stellatune/assets/images/backgrounds/mist.png`

最终提示词：

```text
Use case: photorealistic-natural. Asset type: full-bleed atmospheric background bitmap for a calm desktop music player, landscape 1536x1024. A heavily defocused blue-hour scene of mist over a quiet lake and distant woodland, viewed through frosted glass. All scenery is already strongly blurred into large organic layers of light, mist and shadow, photographic depth rather than a flat digital gradient. Desaturated slate blue, smoky steel blue, muted cyan-gray light in the upper right, deep blue-gray on the left and bottom. Quiet darker leftmost 15 percent behind navigation, broad soft illumination along right margin, medium-dark exposure. Barely perceptible woodland silhouettes and water reflections dissolved into haze, no identifiable objects, no hard horizon. Refined soft diffusion, faint fine grain, gentle tonal variations. No people, no sharp trees, no bright white highlights, no text, no logo, no stars, no UI, no frame or border. This is a finished pre-blurred background image only.
```

### graphite.png

文件：`apps/stellatune/assets/images/backgrounds/graphite.png`

最终提示词：

```text
Use case: photorealistic-natural. Asset type: full-bleed atmospheric background bitmap for a calm desktop music player, landscape 1536x1024. Abstract deeply defocused photographic atmosphere of nocturnal fog and diffuse distant light seen through smoky glass. Finished strong optical blur throughout, broad irregular drifting layers of shadow and soft silver light with photographic depth, not a plain gradient. Palette of graphite charcoal, neutral pewter gray, faint cool slate undertone; restrained silver light concentrated in upper right and right middle, leftmost 15 percent darker quiet space for navigation, bottom softly shaded. Distinct large-scale organic tonal variation without identifiable scenery. Refined, calm, tactile fine grain, mostly medium-dark values, no white-hot highlights. No text, logo, UI, frame, border, people, sharp edges, geometric objects, distinct circular bokeh, no metallic or marble veins. Only an unobtrusive finished pre-blurred wallpaper.
```

### nebula.png

文件：`apps/stellatune/assets/images/backgrounds/nebula.png`

最终提示词：

```text
Use case: stylized-concept. Asset type: full-bleed atmospheric background bitmap for a calm desktop music player, landscape 1536x1024. A soft cinematic nebula seen through diffused frosted glass, sweeping organic clouds of interstellar dust in smoky indigo and muted violet with a restrained blue-teal glow and subtle dusty rose light. Spacious layered nebulous forms, deep blue-black leftmost 15 percent reserved for navigation, soft luminous cloud arcs toward the upper right and right middle, dark gentle falloff at bottom, center not busy. Strongly softened pre-blurred cloudy structures, only a few very faint tiny stars, no sharp high-frequency details, no bright central star. Dark-to-medium exposure, subdued saturation, elegant calm music listening mood, fine photographic grain. Enough variation to feel like cosmic clouds instead of a plain computer gradient. No planets, spacecraft, astronauts, lens flares, bright white cores, neon colors, text, logos, UI, frame or border. Background atmosphere only.
```

### sunroom.png

文件：`apps/stellatune/assets/images/backgrounds/sunroom.png`

暖阳采用明亮的奶油米色摄影背景，配合浅色遮罩、暖灰背景文字和焦糖强调色。使用内置 `image_gen` 工具生成。

最终提示词：

```text
Use case: photorealistic-natural. Asset type: full-bleed atmospheric background for a cozy daytime desktop music player, landscape 1536x1024. An already heavily defocused warm sunlit room seen through frosted glass: soft daylight through linen curtains, vague warm wood and faint plant silhouettes completely melted into organic pools of light and shadow. Overall BRIGHT and airy, creamy ivory, oatmeal beige, pale apricot, soft honey light and gentle warm taupe shadows. Leftmost 15 percent softly even beige for dark navigation text, upper right diffuse warm sunlight, large soft hazy forms along right and lower margins, center quiet for interface cards. Medium-light to light values throughout, no dark navy or charcoal areas. Photographic layered depth, tactile softness, subtle fine grain, calm welcoming relaxed home atmosphere. No identifiable furniture or sharp objects, no literal window grid, no hard lines, no sharp foliage, no blown-out white glare, no orange oversaturation, no text, logos, people, UI, frame or borders. Finished strongly pre-blurred wallpaper only, not an app screenshot or flat computer gradient.
```


- 文件：`apps/stellatune/assets/images/backgrounds/dusk.png`
- 用途：主界面全窗口背景。图片本身已模糊，不包含页面、文字或边框。
- 显示：`BoxFit.cover`，最多解码为 1536 像素宽，叠加主题配置的 `backgroundTint` 降低亮部对比，切换主题交叉淡化。没有实时 BackdropFilter。
- 配置：`lib/ui/theme/desktop_theme.dart` 的 `backgroundAsset`、`backgroundTint`；图片无法加载时显示该主题的渐变底色。
- 播放详情依然根据曲目封面取色，不使用此图。

暮色最终提示词：

```text
Use case: photorealistic-natural. Asset type: full-bleed background texture for a calm desktop music player, landscape 1536x1024. Generate a finished deeply defocused photographic atmosphere, as if dusk light seen through frosted glass. Entire image must be already strongly blurred, broad irregular organic pools of light with hazy layered depth, not a flat computer gradient. Cool slate-blue shadows on left and upper center, smoky lavender-gray in middle, restrained warm peach amber diffuse light toward upper right and lower right, faint soft distant foliage silhouettes and distant city bokeh melted into large indistinct shapes. Dark-to-medium exposure, desaturated refined colors, no white-hot highlights. Leftmost 15 percent is darker quiet space behind a navigation sidebar; center behind cards stays unobtrusive; enough large-scale tonal variety visible along margins. Photographic soft diffusion, smooth tonal transitions, subtle very fine film grain, premium editorial mood. No identifiable subjects, no sharp edges, no legible buildings, no distinct flowers, no hard horizon, no circular bubble pattern, no text, no logos, no UI elements, no frames, no borders. This is only the blurred atmosphere bitmap, never an app screenshot.
```
