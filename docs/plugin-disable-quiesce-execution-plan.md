# Plugin Disable Quiesce Execution Plan

Status: In Progress  
Last Updated: 2026-02-11  
Owner: `stellatune-plugins` / `stellatune-audio` / `stellatune-backend-api` / `stellatune-ffi` / Flutter settings host  
Scope: Breaking redesign accepted; plugin state source-of-truth moves to Rust.

## 1. Background

当前插件禁用流程主要由 Flutter UI 拼接：

1. Flutter 持久化 disabled 列表
2. 调用 `plugins_reload_with_disabled`
3. 部分路径再清理输出路由/侧车

这个流程在 Hot Restart 后时序更敏感，容易出现：

1. 插件已禁用但运行时仍有活跃实例
2. 插件路由仍指向已禁用能力，触发能力查询失败
3. Flutter 调试连接断开（`Lost connection to device`），但进程仍可继续运行

问题本质是：缺少宿主统一的“禁用前排空（quiesce）”和“禁用屏障（freeze）”机制。

## 2. Problem Statement

当前禁用/卸载没有统一收敛语义，主要风险：

1. 禁用窗口内仍能创建新实例
2. 停播、清路由、刷新实例缓存分散在多层实现，顺序不稳定
3. draining generation 回收依赖外部触发时机
4. UI 侧需要理解太多运行时细节

## 3. Goals

建立插件生命周期统一语义，目标如下：

1. “禁用”是原子行为：冻结新建 + 排空旧实例 + 代际收敛
2. UI 不再拼接禁用步骤，只调用一个宿主 API
3. 在 Hot Restart/插件切换场景中消除连接中断高风险窗口
4. 将 shadow copy 清理纳入统一收口点

Non-goals:

1. 不在本阶段做跨进程插件沙箱
2. 不改变插件 ABI（只调整宿主编排）

## 4. Architecture Decision (Breaking)

执行以下破坏性决策：

1. 插件启用/禁用/安装状态由 Rust 持久化并作为唯一真相源。
2. Flutter 不再持久化 disabled plugin 列表。
3. `plugins_reload_with_disabled` 在迁移后移除，替换为“按 Rust 状态 reload”。
4. 早期阶段接受一次性状态重置（不做旧设置兼容迁移）。

## 5. Target API and Contract

## 5.1 Plugin State Storage (Rust-owned)

在 Rust 管理的 SQLite 中新增表（字段可微调）：

1. `plugin_state(plugin_id TEXT PRIMARY KEY, enabled INTEGER NOT NULL, install_state TEXT NOT NULL, disable_in_progress INTEGER NOT NULL DEFAULT 0, last_error TEXT, updated_at_ms INTEGER NOT NULL)`

约束：

1. `enabled=0` 表示禁用，不参与 active capability 加载。
2. `disable_in_progress=1` 期间拒绝创建新实例（freeze guard）。
3. 所有状态变更与禁用编排步骤按事务或阶段日志收敛。

## 5.2 Backend Runtime API (Rust)

新增统一入口（命名可微调）：

1. `plugin_runtime_disable(plugin_id: String, timeout_ms: u64) -> DisableReport`
2. `plugin_runtime_enable(plugin_id: String) -> EnableReport`（可选，通常由 reload 驱动）
3. `plugin_runtime_reload_from_state() -> ReloadReport`

`DisableReport` 至少包含：

1. `plugin_id`
2. `phase`（成功结束时固定为 `completed`）
3. `deactivated_generation: Option<u64>`
4. `unloaded_generations: usize`
5. `remaining_draining_generations: usize`
6. `timed_out: bool`
7. `errors: Vec<String>`

## 5.3 FFI / Flutter Breaking Changes

1. 移除 `disabledIds` 入参链路（`create_library` / `plugins_reload_with_disabled`）。
2. 新增 `pluginDisable(pluginId, timeoutMs)`、`pluginEnable(pluginId)`、`pluginReloadFromState()`。
3. Flutter 设置页不再维护 disabled 集合，只显示 Rust 返回状态。

## 5.4 Host-side Disable Semantics

统一禁用语义为四阶段：

1. `Freeze`: 标记插件为禁用中，拒绝新建实例（所有 capability）。
2. `Quiesce`: 停止相关活动路径（播放、输出路由、runtime query 缓存实例、插件事件流引用）。
3. `Deactivate+Collect`: `deactivate_plugin` + 循环 `collect_ready_for_unload` 直到完成或超时。
4. `Cleanup`: 执行 shadow copy 清理并输出结构化报告。

## 5.5 Error Policy

1. 不因单个步骤失败直接 panic。
2. 对外返回结构化错误和剩余状态，让 UI 可重试。
3. 若超时：返回 `timed_out=true`，并附当前 draining 信息。

## 6. Execution Design

## Phase 0: Rust State Source-of-Truth

实现项：

1. 在 Rust SQLite 增加 `plugin_state` 表与访问层。
2. 启动时插件发现结果与 `plugin_state.enabled` 合并得到最终可加载集合。
3. 删除/废弃 `disabledIds` 参数穿透（先标记 deprecated，再移除）。

退出条件：

1. 启动/重载不再依赖 Flutter 传入 disabled 列表。
2. `plugins_reload_with_disabled` 不再被调用。

## Phase A: Disable Guard (Freeze)

实现项：

1. 在 `stellatune-plugins` 引入 `disabled_or_disabling` 集合（读写锁）。
2. `create_*_instance` 前统一检查；命中则返回一致错误（例如 `plugin disabled: <id>`）。
3. `reload_dir_filtered` 仅按 Rust 状态执行能力更新，不再接收 UI disabled 集合。

退出条件：

1. 禁用开始后，不再产生该插件新实例 ID。

## Phase B: Quiesce Hooks in Audio Runtime

实现项：

1. 在 `stellatune-audio` 暴露单入口 `quiesce_plugin_usage(plugin_id)`。
2. 统一执行：
   1. `Stop`
   2. 如当前 output sink route 指向该插件，先 `ClearOutputSinkRoute`
   3. 清理 `source_instances` / `lyrics_instances` / `output_sink_instances` 中该插件条目
   4. 清理 negotiation cache 中对应路由
3. 该入口在禁用 API 内由 Rust 宿主调用，不依赖 Flutter 手工顺序。

退出条件：

1. 禁用流程中不再出现“route 指向已禁用 capability”路径。

## Phase C: Disable Orchestrator in Backend API

实现项：

1. 在 `stellatune-plugins` / `stellatune-backend-api` 增加通用收敛原语：
   1. `deactivate_plugin`
   2. 轮询 `collect_ready_for_unload`
   3. `cleanup_shadow_copies_now`
2. 上述原语先不绑定 Flutter 入参，仅保证“单插件可收敛”的底层能力完整。

退出条件：

1. 对任一插件可完成“停用 -> 回收 -> 清理”的基础流水线。

## Phase D: Backend Orchestrator by Rust State

实现项：

1. 在 `stellatune-backend-api::runtime` 实现：
   1. `plugin_runtime_disable`（更新 `plugin_state.enabled=0` + quiesce + deactivate/collect + cleanup）
   2. `plugin_runtime_enable`（更新 `enabled=1` + reload）
2. 统一暴露 `plugin_runtime_reload_from_state`。

退出条件：

1. 禁用/启用操作都以 Rust 状态为中心，无需 UI 拼 reload 参数。

## Phase E: FFI + Flutter Integration

实现项：

1. 暴露 FRB API：`pluginDisable` / `pluginEnable` / `pluginReloadFromState`。
2. Flutter `settings_page` 改为：
   1. 调用 `pluginDisable` 或 `pluginEnable`
   2. 调用 `pluginReloadFromState`
3. 删除 Flutter 本地 disabled 持久化键和相关逻辑。
4. 删除 UI 侧分散的“禁用后手动清路由/停侧车”顺序控制代码。

退出条件：

1. Flutter 只表达用户意图，不再拥有插件状态真相。

## 7. Verification Plan

只做短轮次验证（不要求 50 次）：

1. 场景 S1: Hot Restart 后禁用 ASIO，再启用  
预期：无 `Lost connection to device`，应用仍可继续控制播放与设备。
2. 场景 S2: Hot Restart 后禁用 Netease Source，再启用  
预期：source list/query 不崩，恢复正常。
3. 场景 S3: 禁用时存在播放中会话  
预期：禁用返回成功或超时报告，不出现 panic。
4. 场景 S4: 重复禁用/启用 4-5 轮  
预期：`plugin-shadow` 不持续增长，draining 可收敛。
5. 场景 S5: 重启 APP 后禁用状态保持  
预期：无需 Flutter 传参，Rust 按 `plugin_state` 加载。

建议命令：

```powershell
pwsh tools/hot-restart-stress/run-windows.ps1 `
  -Iterations 5 `
  -RestartIntervalMs 4000
```

## 8. Acceptance Criteria

全部满足才算完成：

1. Hot Restart 后禁用/启用 ASIO 不再触发 `Lost connection to device`。
2. 禁用流程中不再创建该插件新实例。
3. 禁用后在超时窗口内，`remaining_draining_generations` 可观测且可重试收敛。
4. 插件 shadow copy 文件数量无持续增长趋势（至少 4-5 次循环稳定）。
5. `plugins_reload_with_disabled` 已移除或不可达。
6. Flutter 不再持久化 disabled plugin 列表。

## 9. Observability

增加结构化日志：

1. `plugin_disable_begin/end`（含 plugin_id, timeout_ms, elapsed_ms）
2. `plugin_disable_phase`（freeze/quiesce/deactivate/collect/cleanup）
3. `plugin_disable_timeout`（含 remaining_draining_generations）
4. `plugin_shadow_cleanup_*`（沿用已存在字段）

建议指标：

1. `plugin_disable_requests_total`
2. `plugin_disable_failures_total`
3. `plugin_disable_timeouts_total`
4. `plugin_disable_duration_ms`
5. `plugin_state_updates_total`

## 10. Rollout Strategy

1. 先完成 Rust 状态源与禁用编排，再接入 FFI/UI。
2. 完成 4-5 轮场景验证后，移除旧 API 与 Flutter disabled 逻辑。
3. 由于项目早期，允许直接切断旧路径，不保留长期 fallback。

## 11. Implementation Checklist

- [x] P0-1. Rust SQLite 增加 `plugin_state` 表与访问层
- [x] P0-2. 启动/reload 改为基于 Rust `enabled` 状态
- [x] A1. `stellatune-plugins` 增加 freeze guard 并接入实例创建路径
- [x] B1. `stellatune-audio` 增加 `quiesce_plugin_usage(plugin_id)` 统一收敛入口
- [x] C1. `stellatune-backend-api` 增加 `plugin_runtime_disable` / `plugin_runtime_enable`
- [x] C2. 禁用流程尾部接入 `cleanup_shadow_copies_now`
- [x] D1. 暴露 FRB `pluginDisable` / `pluginEnable` / `pluginReloadFromState`
- [x] E1. Flutter settings 移除 disabled 持久化，改为调用统一 API
- [x] E2. 删除 `plugins_reload_with_disabled` 旧调用链
- [ ] E3. 完成 S1-S5 验证并记录日志/summary

## 12. Current Transition Notes (2026-02-11)

1. 已新增 `plugin_state` 迁移（`0007_plugin_state.sql`），并在 library 启动时从 Rust DB 恢复 disabled 状态。
2. `PluginRuntimeService` 已持有 disabled 状态；`create_*_instance` 在 disabled 命中时直接拒绝（freeze guard）。
3. `EngineCtrl` 已新增 `quiesce_plugin_usage(plugin_id)` 同步入口，统一执行：
   1. 停播并释放当前会话
   2. 若输出路由指向该插件则清路由
   3. 清理 runtime query 缓存中的该插件实例与协商缓存
4. `stellatune-backend-api::runtime` 已实现：
   1. `plugin_runtime_disable`（freeze + quiesce + deactivate/collect + shadow cleanup）
   2. `plugin_runtime_enable`（更新 enabled 状态）
   3. `plugin_runtime_reload_from_state`（统一 runtime reload 入口）
5. 已暴露 FRB/Bridge API：`libraryPluginDisable` / `libraryPluginEnable` / `libraryPluginsReloadFromState` / `libraryListDisabledPluginIds`。
6. Flutter settings 已改为：
   1. 开关操作调用 Rust API（不再写本地 disabled 集合）
   2. disabled 状态显示由 Rust `plugin_state` 返回
   3. 移除“禁用后手工清输出路由/重置设备”的 UI 侧顺序控制
7. `plugins_reload_with_disabled` 旧链路已从 Rust/FFI/Dart 代码路径移除。
8. `create_library/start_library` 的 `disabledIds` 入参链路已移除，启动阶段不再依赖 Flutter 传参。
9. 仍待完成：
   1. 完成 S1-S5（4-5 轮）验证并沉淀日志/summary
10. `plugin_state` 读写链路已从 `sync + block_on` 改为 `async` 传递到 FRB：
   1. `LibraryHandle::plugin_set_enabled/list_disabled_plugin_ids/plugins_reload_from_state` 改为 `async`
   2. `backend-api::runtime` 的 disable/enable/reload orchestrator 改为 `async`
   3. FRB `library_plugin_disable/library_plugin_enable/library_plugins_reload_from_state/library_list_disabled_plugin_ids` 改为 `async` 导出
