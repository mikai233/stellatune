# StellaTune

[English](README.md) | 简体中文

> 一款为发烧友和极客打造的新一代跨平台音乐播放器。

StellaTune 不仅仅是一个音乐播放器；它是一个开源、可扩展的音频平台，拥有由 **Flutter** 构建的精美用户界面，以及由 **Rust** 编写的极速、内存安全的核心引擎。无论您是管理庞大的本地音乐库，还是通过自定义插件进行流媒体播放，StellaTune 都能为您带来不妥协的音频体验。

项目正在逐步演进为多前端形态：一个适合作为日常主力的 **Flutter** 应用、一个面向终端工作流的 **TUI**，以及一个专注高级渲染、Shader 与音频响应式视觉效果的 **GUI 前端**。

---

## 核心特性

- **发烧级音频体验**：追求真正的纯保真与低延迟播放。基于 Rust 构建的音频管线确保您的音乐能完好无损地呈现。
- **卓越的跨平台能力**：从底层开始为真正的跨平台而生。在桌面端（Windows、macOS、Linux）和移动端都能享受流畅、响应迅速且具有原生体验的
  UI，且所有平台统一运行在稳定强健的 Rust 核心之上。
- **强大的插件生态系统**：无限的扩展可能。StellaTune 拥有极其灵活的 WebAssembly (Wasm) 与原生插件系统。您可以通过放入自定义的音频源、解码器、歌词服务、DSP
  以及输出端插件，来将播放器定制成您理想的模样。
- **现代化的本地音乐库**：为现代音乐爱好者精心打造，美观与实用并存的用户体验，让您能轻松构建、整理和欣赏您的本地收藏。

## 界面预览

![StellaTune Player Interface](docs/assets/app-screenshot.png)

---

## 前端路线图

StellaTune 正在有意识地将前端体验拆分为建立在同一套 Rust 后端之上的不同产品形态：

- **`apps/stellatune`**：面向桌面端与移动端的现代 Flutter 前端，目标是提供更适合作为日常主力的体验，并保持相对克制的视觉风格。*（进行中）*
- **`apps/stellatune-gui`**：Rust 原生图形前端，聚焦实验性渲染、Shader、粒子系统与音频响应式动画。*（早期）*
- **`apps/stellatune-tui`**：面向终端的前端，强调键盘工作流、远程会话可用性以及低依赖环境。*（进行中）*

---

## 快速开始

### 面向普通用户
*（适用于 Windows、macOS 和 Linux 平台的预编译文件和安装包很快就会在 Releases 页面发布。）*

### 面向开发者

StellaTune 非常欢迎开发者们在其核心基础之上进行构建，或者为社区开发令人惊叹的新插件。

#### 环境要求

要从源码构建 StellaTune，您需要：
- [Flutter SDK](https://flutter.dev/docs/get-started/install) （推荐使用 stable 稳定版分支）
- [Rust toolchain](https://rustup.rs/) （stable 版本）
- [Node.js 20](https://nodejs.org/) （用于特定插件打包以及 Sidecar 服务）

安装代码生成器并添加 Wasm 编译目标以准备开发环境：

```bash
cargo install flutter_rust_bridge_codegen --locked
rustup target add wasm32-wasip2
```

#### 运行桌面端应用（以 Windows 为例）

```bash
cd apps/stellatune
flutter pub get
flutter_rust_bridge_codegen generate
flutter run -d windows
```
*注：Rust 后端构建产物会在 `flutter run` 或 `flutter build` 过程中自动编译。*

---

## 插件开发

StellaTune 真正的力量在于其模块化的架构体系。插件可以使用 Rust（以及其他能编译为 Wasm 的语言）编写，并在运行时被动态加载。插件生态包含：`source` (音源), `decoder` (解码器), `lyrics` (歌词), `dsp` (数字信号处理), 和 `output-sink` (音频输出池)。

想要构建自己的扩展插件？请查看我们的开发指南（英文）：
- [Wasm 插件 SDK 快速入门](docs/wasm-plugin-sdk-quickstart.md)
- [Wasm 插件 Manifest 编写指南](docs/wasm-plugin-manifest.md)
- [插件事件协议](docs/plugin-event-protocol.md)

---

## 架构与 Monorepo 代码库

StellaTune 采用 Monorepo 库结构，通过精心设计的结构分离关注点，同时使开发变得简单直接：

- **`apps/stellatune`**: 面向用户的主要应用程序（Flutter 桌面端/移动端）。
- **`apps/stellatune-gui`**: Rust 原生图形前端，面向自定义渲染与强视觉效果。
- **`apps/stellatune-tui`**: 终端用户界面（Rust TUI 版本），复用同一核心。
- **`crates/stellatune-audio*`**: 核心音频运行时、音频管线及播放适配器。
- **`crates/stellatune-plugins`**: 宿主端插件运行时及服务协调器。
- **`crates/stellatune-plugin-sdk`**: 用于实现自定义插件的 SDK。
- **`crates/plugins-native`**: 官方第一方原生及 Wasm 插件（例如 ASIO 输出支持、网易云等功能集成）。
- **`tools/*`**: 插件流程使用的辅助服务（例如网易云 sidecar 等）。

---

## 参与贡献

我们欢迎任何形式和规模的贡献！无论是报告 bug、讨论新功能，还是提交代码，我们都非常感激。

- **小巧聚焦的 PR（Pull Request）** 是让您的代码被快速合并的最佳方式。
- 提交信息请遵守 **Conventional Commits（约定式提交）** 规范。
- 如果您修改了与 CI 敏感相关的代码，请在 Push 之前运行本地检查：

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

对于 Flutter UI 的更改：
```bash
cd apps/stellatune
flutter analyze
flutter build windows --debug
```

## 开源协议

[MIT License](LICENSE) （或在适用情况下查看单独 Crate 的许可协议）。
