# Plugin Runtime Load/Unload Breaking Refactor Plan

Status: In Progress  
Last Updated: 2026-02-11  
Owner: `stellatune-plugins` / `stellatune-audio` / `stellatune-backend-api` / `stellatune-ffi` / Flutter host  
Scope: **Breaking change only**. No backward compatibility layer, no dual-path rollout.

## 1. Background

当前插件加载链路存在两个结构性问题：

1. 控制线程会同步执行插件扫描/拷贝/动态库加载，影响实时音频路径。
2. 安装/启用/禁用流程存在重复 reload，导致无变化插件也被全量重载。

在 ASIO 播放中，这会放大为控制线程卡顿和调试连接不稳定（例如 `Lost connection to device`）。

## 2. Refactor Policy (Hard Breaking)

本计划明确采用破坏性改造，不做兼容：

1. 删除旧的“双入口 reload”语义，不保留旧 API 兼容分支。
2. Flutter 不再驱动 reload 编排顺序，只提交期望状态变更意图。
3. Runtime 以单一 orchestrator 执行加载/卸载事务，不接受旁路调用。
4. 不引入 transitional flag，不长期保留 legacy code path。

## 3. Target Architecture

### 3.1 Single Orchestrator

新增 `PluginRuntimeCoordinator`（Rust），作为唯一生命周期入口：

1. `apply_plugin_state(desired_state)`：唯一提交入口。
2. 串行执行计划，保证同一时刻仅一个生命周期事务在执行。
3. 后台线程完成重 I/O，控制线程只处理结果提交。

### 3.2 Two-Layer State Model

定义两层状态并持续收敛：

1. `desired_state`: 已安装插件集合 + enabled/disabled + uninstall pending。
2. `runtime_state`: active generation、draining generation、实例引用计数。

所有操作先写 `desired_state`，再由 coordinator 做 `diff -> actions -> commit`。

### 3.3 Incremental Diff Planner

禁止全量 reload，改为增量动作：

1. `LoadNew`: 新装但未激活插件。
2. `ReloadChanged`: 指纹变化插件（manifest/hash/mtime）。
3. `DeactivateMissingOrDisabled`: 已移除或已禁用插件。
4. `CollectDraining`: 回收可卸载 generation。

未变化且仍启用的插件必须保持 active，不允许被 reload。

### 3.4 Async Loader/Unloader Pipeline

生命周期重活全部移到后台 worker：

1. 扫描目录、shadow copy、`LoadLibrary`、FFI metadata 探测。
2. 产出 `PreparedPluginGeneration`。
3. 控制线程仅执行快速 swap/activate/deactivate/refresh。

## 4. Breaking API Changes

## 4.1 Remove

移除或禁止以下语义（命名可微调）：

1. `library.plugins_reload_from_state()` 作为实际加载入口。
2. Flutter 组合式 `_reloadPluginsWithCurrentDisabled()`。
3. 任意直接触发“全量 reload all enabled plugins”的 API。

## 4.2 Add / Keep

统一为意图型接口：

1. `plugin_install(artifact)` -> 更新 desired state（installed + enabled default policy）。
2. `plugin_set_enabled(plugin_id, enabled)` -> 更新 desired state。
3. `plugin_uninstall(plugin_id)` -> 更新 desired state。
4. `plugin_apply_state()` -> 触发 coordinator 执行一次增量收敛。

如果产品接受自动收敛，可进一步将 `apply` 内聚到上述命令中，外部不暴露显式 reload。

## 5. Execution Plan

## Phase A: Lifecycle Contract Freeze (3-4 days)

实现项：

1. 文档化并冻结新生命周期契约（单入口、增量、异步）。
2. 标记旧 API 为删除目标，直接修改调用方到新入口。
3. 定义 `desired_state` 与 `runtime_state` 数据结构。

退出条件：

1. 代码中不存在新的旧路径调用。
2. 旧路径仅保留到迁移完成当次 PR，随后删除。

## Phase B: Diff Planner + Fingerprint (4-5 days)

实现项：

1. 实现插件指纹计算（manifest + artifact hash/mtime）。
2. 实现 `diff(desired_state, runtime_state) -> ActionPlan`。
3. 将安装场景替换为 `LoadNew`，禁用/卸载替换为 `Deactivate*`。

退出条件：

1. 安装新插件时，未变化已启用插件 reload 次数为 0。

## Phase C: Background Load/Unload Worker (5-7 days)

实现项：

1. 新增 coordinator worker 线程和结果回传消息。
2. 所有动态库加载和目录 I/O 移出控制线程。
3. 支持 in-flight 合并策略（coalesce）和排队策略（last-write-wins）。

退出条件：

1. 控制线程无同步 `LoadLibrary`/扫描/拷贝行为。
2. ASIO 播放期间插件安装不造成可观测卡顿峰值。

## Phase D: Engine/Library/FFI Integration (3-4 days)

实现项：

1. `stellatune-audio` 只接收 `PluginPlanApplied`/`PluginPlanFailed` 等结果消息。
2. `stellatune-library` 仅负责状态存储，不再做运行时 reload。
3. `stellatune-ffi` 与 Flutter 迁移到新意图接口。

退出条件：

1. 不再存在 library + player 双重 reload。

## Phase E: Legacy Path Deletion (2 days)

实现项：

1. 删除旧 API、旧桥接函数、旧 UI 编排逻辑。
2. 删除兼容代码、迁移注释和 dead code。

退出条件：

1. 仓库内不可达旧加载模型代码为 0。

## 5.1 Progress Sync

2026-02-11 当前实现进度：

1. `Phase A` In Progress
2. `Phase B` In Progress
3. `Phase C` In Progress
4. `Phase D` In Progress
5. `Phase E` In Progress

本轮已完成：

1. `stellatune-audio` 插件 reload 改为后台线程执行，控制线程不再同步调用 `reload_dir_from_state`。
2. 增加 reload in-flight 合并机制：进行中的 reload 期间新请求只保留最后一次目录并排队执行。
3. `stellatune-library::plugins_reload_from_state` 改为仅同步 `disabled_plugin_ids` 运行时状态，不再执行实际 DLL reload（移除双重 reload 关键路径）。
4. 安装路径从“安装即加载”改为“安装写入后由 `plugin_apply_state` 统一收敛执行”，避免入口分散。
5. Flutter 安装流程不再调用 `player.pluginsReload()`，改为仅同步状态并刷新 UI。
6. `plugin_runtime_enable` 收敛为“仅状态变更”，加载行为由统一 `plugin_apply_state` 事务执行。
7. Flutter 启用/禁用/卸载流程移除 `_reloadPluginsWithCurrentDisabled()` 调用链，不再触发 `player.pluginsReload()` 全量扫描。
8. `stellatune-plugins` 新增差异规划执行：`sync_dir_from_state` + `plan_sync_actions`，`reload_dir_from_state` 从全量重载改为按 `LoadNew / ReloadChanged / DeactivateMissingOrDisabled` 执行。
9. 新增 source library 指纹（路径 + 文件大小 + 修改时间）用于判断 `ReloadChanged`，未变化插件在 reconcile reload 中保持 active、不触发 reload。
10. Flutter bridge 移除未使用的 `pluginsReload` 包装入口，继续收敛旧路径暴露面。
11. 删除 backend/FFI 旧 `plugins_reload` 对外入口，并通过 FRB 重新生成 Rust/Dart 绑定。
12. 删除 `library_plugins_reload_from_state` / `plugin_runtime_reload_from_state` 旧链路（Flutter bridge + FFI + backend API），并通过 FRB 重新生成绑定。
13. 清理旧链路遗留死代码（`clear_plugin_worker_caches`），保持构建无告警退化。
14. 新增 `plugin_runtime_apply_state` / `library_plugin_apply_state` API，并将设置页安装/启用/禁用/卸载后统一调用 `pluginApplyState()`。
15. `plugin_apply_state` 增加串行执行锁与状态快照（`idle/applying/applied/failed`），并新增状态查询接口 `plugin_apply_state_status_json`（FFI/Bridge 已贯通）。
16. 新增独立协调模块 `runtime/apply_state.rs`，将 apply 事务状态管理从 `runtime/mod.rs` 拆出，并落地请求合并策略（coalesce 到最新 request）。
17. `stellatune-plugins` 新增 detailed sync report：包含 plan 计数（LoadNew/ReloadChanged/Deactivate）、action outcome 列表、`plan_ms/execute_ms/total_ms`。
18. `ApplyStateReport` 增加 structured 字段（plan、timing、action outcomes、coalesce 统计），`plugin_apply_state_status_json` 同步输出这些快照字段。
19. 全仓 `cargo check` + 局部 `dart analyze` 通过。
20. 调整 `stellatune-backend-api::PlayerService` 查询锁粒度：`source_list_items_json / lyrics_provider_search_json / lyrics_provider_fetch_json / output_sink_list_targets_json` 不再在 `with_runtime_service` 临界区内执行插件实例调用，仅在锁内创建实例；减少慢查询（如 sidecar 健康检查）对全局 runtime 锁的占用时间。
21. Flutter 设置页安装流程增加 `FilePicker` 显式异常捕获与错误提示，避免“安装弹窗打不开”时静默失败，便于区分平台通道异常与业务状态问题。
22. 为 `stellatune-plugin-sdk` 的 OutputSink FFI 导出层增加 `catch_unwind` 防护（`output_modules`）：将插件 panic 转为 `StStatus` 错误并记录 panic + backtrace，避免 panic 穿越 `extern "C"` 直接触发进程级 abort（针对 `Lost connection to device` 的 ASIO 路径排查加固）。

本轮未完成（下一阶段）：

1. 将当前 `service` 内部 planner 提升为独立 coordinator 层（`desired_state` 持久化输入，`action plan` 统一出口）。
2. 将 structured report 持久化到可查询历史（例如最近 N 次 apply 记录），并补充压力测试断言。

## 6. Verification Plan

核心验证场景：

1. ASIO 播放中安装 Netease Source：仅新插件加载，旧插件不 reload。
2. ASIO 播放中启用/禁用切换：控制线程保持稳定，音频不中断。
3. 连续安装/卸载/启用/禁用压力测试：generation 能收敛，shadow copy 不增长失控。
4. Hot Restart + 插件操作混合压力：无 `Lost connection to device`。

关键指标：

1. 控制线程插件事务期间 p99 阻塞 `< 2ms`。
2. 插件安装事务中 `reload_unchanged_plugins_total == 0`。
3. `plugin_apply_state_duration_ms` 可观测且稳定。
4. `draining_generations` 在超时窗口内可收敛或可报告重试。

## 7. Risks

1. 指纹策略不稳定导致误判 reload。  
   Mitigation: 指纹字段固定并加回归测试。
2. 后台加载结果与最新 desired state 竞态。  
   Mitigation: generation token + last-write-wins 丢弃过期结果。
3. 一次性删除旧路径带来集成风险。  
   Mitigation: 按 Phase 合并，每阶段必须有可回滚 PR 边界（代码回滚，不是运行时兼容）。

## 8. Acceptance Criteria

全部满足才完成：

1. 插件加载/卸载模型完全切换到单 orchestrator。
2. 安装新插件时，不再重载整个已启用插件库。
3. 实时播放（尤其 ASIO）中不再因插件加载导致控制线程长阻塞。
4. 旧 reload 路径和兼容分支全部删除。

## 9. Deliverables

1. 架构文档与状态机文档（本文件 + 细化设计）。
2. coordinator + diff planner + async worker 实现。
3. FFI 与 Flutter 新接口改造。
4. 自动化回归用例与压力测试脚本。
