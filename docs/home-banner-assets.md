# 首页主题横幅素材

2026-09-05 使用内置 `image_gen` 生成八张 2048×768 横幅并复制到项目。没有使用 CLI/API 模式。原图保留在 Codex 生成目录。

- 主题通过 `ArtworkPalette.homeBannerAsset` 配置；文字、按钮和两侧遮罩使用主题颜色。
- 主横幅采用居中 `BoxFit.cover` 裁切；右侧引言卡复用同套素材的局部构图。只淡化图片，继续播放按钮保持挂载。
- 图片最多解码到 1536 像素宽，不包含生成文字；文案由 Flutter 渲染。
- 新用户默认晴空；保留已保存主题。预览截图为 `apps/stellatune/build/visual-review/home-theme-<主题名>.png`。

## dusk

文件：`apps/stellatune/assets/images/banners/dusk.png`

最终提示词：

```text
Use case: photorealistic-natural. Asset type: cinematic editorial hero banner for a calm music player. Generate a wide landscape 2048x768 image, full bleed, no text or lettering, no UI, no watermark, no frame, no people. Main visual interest in the middle 40 percent horizontally. Leftmost 25 percent and rightmost 30 percent must be quiet negative space for UI copy. All essential scenery lies in the central horizontal band, safe for a further panoramic crop to 4.5:1. Elegant photographic depth, restrained detail and low saturation, natural subtle grain. This is an attractive focused banner photograph with softer distant atmosphere, not a completely blurred wallpaper. Scene: a quiet urban rooftop at blue hour with the distant city and a peach sunset on the horizon. Slate blue and smoky mauve shadows, tiny warm amber lights in the middle distance. Keep both left and right edges dark slate and uncluttered for white text, sky brightest only near center. Reflective, intimate evening mood.
```

## mist

文件：`apps/stellatune/assets/images/banners/mist.png`

最终提示词：

```text
Use case: photorealistic-natural. Asset type: cinematic editorial hero banner for a calm music player. Generate a wide landscape 2048x768 image, full bleed, no text or lettering, no UI, no watermark, no frame, no people. Main visual interest in the middle 40 percent horizontally. Leftmost 25 percent and rightmost 30 percent must be quiet negative space for UI copy. All essential scenery lies in the central horizontal band, safe for a further panoramic crop to 4.5:1. Elegant photographic depth, restrained detail and low saturation, natural subtle grain. This is an attractive focused banner photograph with softer distant atmosphere, not a completely blurred wallpaper. Scene: a still mountain lake with a small wooden jetty in the center and faint mountain silhouettes disappearing into cool fog. Smoky desaturated teal, steel blue and silver reflections. Medium-dark exposure; left and right edges even blue-gray dark enough for white text, no white fog at edges. Quiet, contemplative atmosphere.
```

## graphite

文件：`apps/stellatune/assets/images/banners/graphite.png`

最终提示词：

```text
Use case: photorealistic-natural. Asset type: cinematic editorial hero banner for a calm music player. Generate a wide landscape 2048x768 image, full bleed, no text or lettering, no UI, no watermark, no frame, no people. Main visual interest in the middle 40 percent horizontally. Leftmost 25 percent and rightmost 30 percent must be quiet negative space for UI copy. All essential scenery lies in the central horizontal band, safe for a further panoramic crop to 4.5:1. Elegant photographic depth, restrained detail and low saturation, natural subtle grain. This is an attractive focused banner photograph with softer distant atmosphere, not a completely blurred wallpaper. Scene: rainy city at night seen through a wide cafe window, rain on glass and distant defocused lights concentrated in the center, a subtle central window reflection. Charcoal gray and graphite with restrained silver highlights. Monochromatic, sophisticated, intimate; dark quiet left and right edges for white copy, no neon signs or lettering.
```

## nebula

文件：`apps/stellatune/assets/images/banners/nebula.png`

最终提示词：

```text
Use case: stylized-concept. Asset type: cinematic editorial hero banner for a calm music player. Generate a wide landscape 2048x768 image, full bleed, no text or lettering, no UI, no watermark, no frame, no people. Main visual interest in the middle 40 percent horizontally. Leftmost 25 percent and rightmost 30 percent must be quiet negative space for UI copy. All essential scenery lies in the central horizontal band, safe for a further panoramic crop to 4.5:1. Elegant photographic depth, restrained detail and low saturation, natural subtle grain. This is an attractive focused banner photograph with softer distant atmosphere, not a completely blurred wallpaper. Scene: flowing interstellar dust clouds in smoky indigo and muted violet with a soft silver-blue luminous nebula in the middle. A few tiny stars, cinematic astronomical photograph aesthetic. Left and right edges are calm deep indigo negative space for white copy. No planets, spaceships, bright starbursts or neon colors.
```

## sunroom

文件：`apps/stellatune/assets/images/banners/sunroom.png`

最终提示词：

```text
Use case: photorealistic-natural. Asset type: cinematic editorial hero banner for a calm music player. Generate a wide landscape 2048x768 image, full bleed, no text or lettering, no UI, no watermark, no frame, no people. Main visual interest in the middle 40 percent horizontally. Leftmost 25 percent and rightmost 30 percent must be quiet negative space for UI copy. All essential scenery lies in the central horizontal band, safe for a further panoramic crop to 4.5:1. Elegant photographic depth, restrained detail and low saturation, natural subtle grain. This is an attractive focused banner photograph with softer distant atmosphere, not a completely blurred wallpaper. Scene: a sunlit cozy listening room, a small linen armchair and wooden side table with a ceramic cup concentrated in the middle, gentle daylight through sheer curtains. Warm creamy ivory, oatmeal and pale honey. Entire left and right copy areas bright plain softly lit linen or plaster, suitable for dark brown text. Airy and welcoming, no dark patches at edges, no window grid through copy.
```

## daylight

文件：`apps/stellatune/assets/images/banners/daylight.png`

最终提示词：

```text
Use case: photorealistic-natural. Asset type: cinematic editorial hero banner for a calm music player. Generate a wide landscape 2048x768 image, full bleed, no text or lettering, no UI, no watermark, no frame, no people. Main visual interest in the middle 40 percent horizontally. Leftmost 25 percent and rightmost 30 percent must be quiet negative space for UI copy. All essential scenery lies in the central horizontal band, safe for a further panoramic crop to 4.5:1. Elegant photographic depth, restrained detail and low saturation, natural subtle grain. This is an attractive focused banner photograph with softer distant atmosphere, not a completely blurred wallpaper. Scene: a luminous calm seashore under clear pale blue sky, thin white clouds above, translucent water and a small pale sand dune in the center. Powder blue, pearl white and pale sand, bright high-key exposure. Both copy areas light and even for dark navy text, no strong horizon or waves crossing the text. Fresh, spacious daylight.
```

## lavender

文件：`apps/stellatune/assets/images/banners/lavender.png`

最终提示词：

```text
Use case: photorealistic-natural. Asset type: cinematic editorial hero banner for a calm music player. Generate a wide landscape 2048x768 image, full bleed, no text or lettering, no UI, no watermark, no frame, no people. Main visual interest in the middle 40 percent horizontally. Leftmost 25 percent and rightmost 30 percent must be quiet negative space for UI copy. All essential scenery lies in the central horizontal band, safe for a further panoramic crop to 4.5:1. Elegant photographic depth, restrained detail and low saturation, natural subtle grain. This is an attractive focused banner photograph with softer distant atmosphere, not a completely blurred wallpaper. Scene: a pale lavender field in gentle early morning haze, soft lilac blossoms concentrated in the middle foreground and low rolling hills at the horizon. Pearl white sky, pastel lilac and a faint blush light. Bright high-key exposure, edges especially quiet and light for dark plum copy. Gentle realistic photography, not saturated purple.
```

## celadon

文件：`apps/stellatune/assets/images/banners/celadon.png`

最终提示词：

```text
Use case: photorealistic-natural. Asset type: cinematic editorial hero banner for a calm music player. Generate a wide landscape 2048x768 image, full bleed, no text or lettering, no UI, no watermark, no frame, no people. Main visual interest in the middle 40 percent horizontally. Leftmost 25 percent and rightmost 30 percent must be quiet negative space for UI copy. All essential scenery lies in the central horizontal band, safe for a further panoramic crop to 4.5:1. Elegant photographic depth, restrained detail and low saturation, natural subtle grain. This is an attractive focused banner photograph with softer distant atmosphere, not a completely blurred wallpaper. Scene: serene minimalist courtyard garden with a celadon ceramic pot and delicate leafy branch in the central composition, very pale sage plaster wall and diffuse morning sun. Misty mint, sage green and soft ivory, bright high-key exposure. Left and right areas plain softly lit wall suitable for dark green text. Tactile natural ceramics and restful airy garden atmosphere.
```
