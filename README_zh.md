# StellaTune

[English](README.md) | 简体中文

> 一款为发烧友和极客打造的新一代跨平台音乐播放器。
>
> 当前状态：早期开发 / WIP。StellaTune 正在积极开发中，API、插件接口和面向用户的功能都可能频繁调整。
> 当前功能适配优先级：Windows 优先，其次是其余桌面平台，最后是移动端。

StellaTune 不仅仅是一个音乐播放器；它是一个开源、可扩展的音频平台，拥有由 **Flutter** 构建的精美用户界面，以及由 **Rust** 编写的极速、内存安全的核心引擎。无论您是管理庞大的本地音乐库，还是通过自定义插件进行流媒体播放，StellaTune 都能为您带来不妥协的音频体验。

---

## 核心特性

- **发烧级音频体验**：追求真正的纯保真与低延迟播放。基于 Rust 构建的音频管线确保您的音乐能完好无损地呈现。
- **卓越的跨平台能力**：从底层开始为真正的跨平台而生。在桌面端（Windows、macOS、Linux）和移动端都能享受流畅、响应迅速且具有原生体验的
  UI，且所有平台统一运行在稳定强健的 Rust 核心之上。
- **强大的插件生态系统**：TypeScript 插件负责音源解析、搜索、认证、歌词等控制面能力；解码、DSP 和输出保留在原生 Rust 数据面。ASIO 等受许可约束的集成通过独立分发的 native sidecar 接入。
- **现代化的本地音乐库**：为现代音乐爱好者精心打造，美观与实用并存的用户体验，让您能轻松构建、整理和欣赏您的本地收藏。

## 界面预览

![StellaTune Player Interface](docs/assets/app-screenshot.png)

---

## 快速开始

### 面向普通用户
*（适用于 Windows、macOS 和 Linux 平台的预编译文件和安装包很快就会在 Releases 页面发布。）*

### 面向开发者

StellaTune 非常欢迎开发者们在其核心基础之上进行构建，或者为社区开发令人惊叹的新插件。

#### 环境要求

要从源码构建 StellaTune，您需要：
- [Flutter SDK](https://flutter.dev/docs/get-started/install) 3.47.2 / Dart 3.13.2（也可使用 FVM）
- [Rust toolchain](https://rustup.rs/) 1.98.0
- [Node.js 20](https://nodejs.org/) （用于特定插件打包以及 Sidecar 服务）

安装代码生成器以准备开发环境：

```bash
cargo install flutter_rust_bridge_codegen --version 2.13.0 --locked
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

StellaTune 插件是由共享 Node runner 加载的预打包 TypeScript 模块，负责返回声明式音源计划和其他控制面结果；媒体字节和 PCM 不经过 Node。解码与 DSP 由原生 Rust stage 执行，可选外部原生进程通过 sidecar 协议接入。

想要构建自己的扩展插件？请查看我们的开发指南（英文）：
- [TypeScript 插件快速入门](docs/typescript-plugin-quickstart.md)
- [插件与播放运行时架构](docs/stellatune-audio-architecture.md)

---

## 架构与 Monorepo 代码库

StellaTune 采用 Monorepo 库结构，通过精心设计的结构分离关注点，同时使开发变得简单直接：

- **`apps/stellatune`**: 面向用户的主要应用程序（Flutter 桌面端/移动端）。
- **`apps/stellatune-tui`**: 终端用户界面（Rust TUI 版本），复用同一核心。
- **`crates/stellatune-audio*`**: 核心音频运行时、音频管线及播放适配器。
- **`crates/stellatune-plugins`**: TypeScript 插件包管理与进程运行时。
- **`crates/plugins-native`**: 原生协议与独立分发的 sidecar。
- **`tools/typescript-plugin-runtime`**: 共享 Node runner 与 TypeScript 插件 SDK 类型。

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
