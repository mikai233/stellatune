# EngineHandle Async OneShot API Migration Plan

Status: `Draft`  
Owner: `stellatune-audio` / `stellatune-backend-api` / `stellatune-ffi` / Flutter host  
Scope: 播放控制命令链路（Rust -> Flutter）

## 1. Background

当前播放控制链路存在两层包装：

1. `RuntimeHost(engine: EngineHandle)`（`crates/stellatune-backend-api/src/runtime/host.rs`）
2. `PlayerService(runtime: Arc<RuntimeHost>)`（`crates/stellatune-backend-api/src/player.rs`）

同时，大量命令仍是 fire-and-forget（`send_command`）+ 事件流回执模式。  
目标是收敛为：

1. 直接围绕 `EngineHandle` 暴露公共 API（移除两层包装）
2. 命令 API 异步化，使用 one-shot 返回命令结果（`Result<T, E>`）
3. Rust -> Flutter 推送端口保留，用于实时状态流（如播放进度）

## 2. Goals

1. 统一命令调用语义：调用方可以 `await` 明确的执行结果。
2. 对“有实质性返回值”的操作不再依赖事件流传结果。
3. 保留事件流作为状态广播通道（实时、持续变化）。
4. 控制线程单写模型不变，不引入多写状态竞争。

## 3. Non-Goals

1. 不重写音频核心状态机（`tick`/`decode`/`session` 算法不变）。
2. 不移除插件 runtime 事件协议（`plugin_runtime_events_global` 保留）。
3. 不在同一阶段改动 Library 的事件驱动模式。

## 4. Event vs OneShot Boundary

### 4.1 保留推送事件（必须保留）

以下是“持续变化”或“广播语义”数据，继续走 Rust -> Flutter 推送端口：

1. `Event::Position`（播放进度）
2. `Event::StateChanged`（播放状态变迁）
3. `Event::TrackChanged`（曲目切换）
4. `Event::PlaybackEnded`（自然结束）
5. `Event::Log`（运行期日志）
6. `LyricsEvent::*`（歌词流）
7. `PluginRuntimeEvent::*`（插件运行时事件）

### 4.2 迁移为命令返回值（优先）

以下结果型命令优先迁移为 one-shot 返回值：

1. `refresh_devices`：返回 `Vec<AudioDevice>`，不再依赖 `Event::OutputDevicesChanged`
2. `switch_track_ref` / `play` / `pause` / `seek_ms` / `stop`：返回 `Result<(), String>`（至少有明确成功/失败）
3. `set_output_*` / `set_output_sink_route` / `clear_output_sink_route`：返回 `Result<(), String>`（参数校验/应用失败直接返回）

备注：`Event::OutputDevicesChanged` 在迁移完成后可移除；迁移过渡期可双发（返回值 + 事件）以降低 Flutter 侧切换风险。

## 5. Current Constraints

1. `EngineHandle` 目前对命令通道是 `Sender<Command>`，`send_command` 无返回值（`crates/stellatune-audio/src/engine/control.rs`）。
2. 控制线程已有“查询式回包”先例（`EngineCtrl + resp_tx`），可复用模式（`crates/stellatune-audio/src/engine/messages.rs`）。
3. Flutter 侧目前部分逻辑从 `events()` 中提取结果（例如设备列表）。

## 6. Target Architecture

## 6.1 Audio Layer (`stellatune-audio`)

1. 新增 one-shot 命令请求结构，例如 `CommandRequest { command, resp_tx }`。
2. 控制线程处理命令后填充 `resp_tx`，返回 `CommandResult`（可分 `Ack` / `Payload`）。
3. `EngineHandle` 提供 async API（示例：`async fn refresh_devices(&self) -> Result<Vec<AudioDevice>, String>`）。
4. 保留 `subscribe_events()`，作为状态广播入口。

## 6.2 Backend API Layer (`stellatune-backend-api`)

1. 新建/调整面向 `EngineHandle` 的公共 API 模块。
2. 移除 `PlayerService` 和 `RuntimeHost` 包装层及 client attach/detach 管理。
3. 保留 runtime 全局入口：`prepare_hot_restart` / `shutdown` 仍可通过共享 `EngineHandle` 调用。
4. 插件控制路由改为调用新的 async 命令 API；若等待条件仍需事件确认，继续保留 wait 逻辑。

## 6.3 FFI Layer (`stellatune-ffi`)

1. `Player` 持有对象由 `PlayerService` 迁移为新的 Engine API Handle。
2. 原同步导出函数改为 async + `Result`（FRB 自动映射为 Dart `Future`）。
3. `events(player)` 推送端口保留，不做下线。

## 6.4 Flutter Layer

1. `PlayerBridge` 的结果型方法改为直接消费返回值。
2. 设备列表 provider 从事件流改为 `refreshDevices()` 的请求-响应模式（必要时本地缓存）。
3. 播放进度/状态仍通过 `events()` 订阅。

## 7. Migration Phases

## Phase A: 引入 OneShot 命令基础设施（不改上层 API）

1. 在 `audio` 内增加命令请求与响应类型。
2. 控制线程完成命令后回包；现有 `send_command` 暂时保留。
3. 为关键命令返回结构化错误（替代纯 `Event::Error`）。

Exit Criteria:

1. 控制线程命令路径已具备 one-shot 响应能力。
2. 旧调用路径仍可运行（兼容阶段）。

## Phase B: EngineHandle 暴露 async 公共 API

1. 新增 `EngineHandle` async 方法（先覆盖结果型命令）。
2. 后端内部调用改为优先使用 async 方法。
3. 对同名旧接口标记 `deprecated`（仅过渡期）。

Exit Criteria:

1. 结果型命令都可通过 `await` 获得返回。
2. 现有测试通过。

## Phase C: Backend 去包装（RuntimeHost / PlayerService）

1. 新增轻量共享句柄管理（全局 `EngineHandle` 初始化 + 复用）。
2. 迁移 `prepare_hot_restart` / `shutdown` 到新入口。
3. 删除 `RuntimeHost` client generation/attach/detach 逻辑与相关测试。

Exit Criteria:

1. backend-api 不再依赖 `RuntimeHost` / `PlayerService`。
2. runtime API 功能等价。

## Phase D: FFI 与 Flutter 迁移

1. FFI 导出切换到新 async API，重新生成 FRB 绑定。
2. Flutter `PlayerBridge` 对齐新返回值模型。
3. 保留 `events()` 订阅，仅移除“结果型事件消费”。

Exit Criteria:

1. Flutter 播放控制、设置页、设备刷新功能正常。
2. 进度/状态事件流持续工作。

## Phase E: 删除兼容路径与收敛

1. 删除 `send_command` 旧路径（若确认无调用）。
2. 删除 `Event::OutputDevicesChanged`（若已完全切换为返回值）。
3. 清理文档和 dead code。

Exit Criteria:

1. 代码中不存在旧包装和兼容分支。
2. API 语义单一、可维护。

## 8. Compatibility Strategy

1. 迁移期间允许“双轨短暂并存”：新 one-shot 返回 + 旧事件发布。
2. 每完成一个模块的消费迁移后，再移除对应旧事件依赖。
3. 通过 feature flag 或 compile-time 切分阶段性行为，避免一次性大爆炸提交。

## 9. Risk List

1. 插件控制路由当前依赖 player event 来判定完成时机，直接移除事件可能导致 timeout。
2. Flutter 侧历史上将 `Event::Error` 作为通用错误来源，迁移后需要统一异常显示入口。
3. FRB 改动会影响生成代码与 Dart API 签名，需要同步更新调用点。

## 10. Verification Checklist

1. 命令调用：
1. `switch_track_ref/play/pause/seek/stop` 返回值正确。
2. `refresh_devices` 返回设备列表，设置页可正常展示。
2. 事件流：
1. `Position` 连续上报，无回退/串流污染。
2. `StateChanged/TrackChanged/PlaybackEnded` 行为不变。
3. Runtime：
1. `prepare_hot_restart/shutdown` 正常收敛。
2. 插件 runtime 事件链路正常。
4. 端到端：
1. 桌面端启动、播放、切歌、拖动进度条、切设备均可用。
2. 无新增 control-thread deadlock 或 panic。

## 11. Rollout Notes

1. 建议按 Phase 拆 PR，每个 PR 控制在单一主题。
2. 每个 Phase 完成后更新本文档状态与勾选项。
3. 完成 Phase D 后再安排 Phase E 清理，避免影响联调效率。

## 12. Progress Tracking

- [x] Phase A: OneShot 基础设施
- [x] Phase B: EngineHandle async API
- [x] Phase C: backend 去包装
- [x] Phase D: FFI + Flutter 迁移（保留事件推送端口）
- [x] Phase E: 兼容路径清理

### Latest Update

1. 2026-02-12: Phase A completed（控制命令通道已支持 one-shot 回包；命令处理路径已返回结构化 `Result`；`refresh_devices` 已具备返回设备列表能力；原事件推送语义保持兼容）。
2. 2026-02-12: Phase B completed（`EngineHandle` 已提供 async one-shot 命令 API，可 `await` 获取命令结果；同步兼容入口与推送事件端口保留）。
3. 2026-02-12: Phase C completed（`RuntimeHost` / `PlayerService` 已移除；backend 公共层直接基于共享 `EngineHandle`）。
4. 2026-02-12: Phase D in progress（FFI `Player` 已改为直接持有 `EngineHandle + LyricsService`；播放控制/输出控制命令已切到 async one-shot 返回；Rust->Flutter 事件推送端口保持不变）。
5. 2026-02-12: Phase D progress（Flutter `PlayerBridge.refreshDevices()` 已改为返回 `Future<List<AudioDevice>>`；`audioDevicesProvider` 已改为请求式 `FutureProvider`，不再从 `Event::OutputDevicesChanged` 提取设备列表；设置页已接入 provider refresh 流程）。
6. 2026-02-12: Phase E progress（`refresh_devices` 已停止发布 `Event::OutputDevicesChanged`；插件 runtime 控制路由中 `RefreshDevices` 等待条件已改为 `Immediate`，不再依赖设备变更事件完成）。
7. 2026-02-12: Phase E progress（`Event::OutputDevicesChanged` 类型定义已删除；FRB + Freezed 已重新生成并通过检查，Flutter 事件匹配已完成收敛）。
8. 2026-02-12: Phase E completed（`EngineHandle::send_command` 旧 fire-and-forget 入口已移除；runtime/plugin 路由改为阻塞 one-shot 命令分发；热重启与关闭流程改为命令执行结果可观测）。
9. 2026-02-12: Phase E completed（FFI/Flutter 已去除 `Player` 句柄模型：导出 API 改为无 `player` 参数，Flutter 侧不再持有 `Player` opaque，只通过全局 API 调用）。
