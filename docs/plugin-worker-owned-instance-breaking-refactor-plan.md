# Plugin Worker-Owned Instance Breaking Refactor Plan

Status: Draft  
Last Updated: 2026-02-12  
Owner: `stellatune-plugins`  
Scope: **Breaking change only**, and **plugin runtime layer only**. Business-side integration errors are out of scope for this phase.

## 1. Background

当前插件系统已经具备 generation/draining/unload 基础能力，但实例执行模型仍然偏宿主包装导向：

1. 能力实例创建与调用路径在 `service` 内耦合较重。
2. 生命周期与业务调用边界不够清晰。
3. 对解码等高频路径，必须保证实例在 Worker 线程长期持有并直接运行，避免额外调度损耗。

本方案将实例执行模型明确为：**Worker 线程拥有实例，Runtime 只管理插件生命周期**。
同时将代际机制简化为：**ModuleLease + 引用计数**。

## 2. Refactor Policy (Hard Breaking)

1. 不做 V1/V2 双栈兼容，不保留旧调用语义。
2. `stellatune-plugins` 对外 API 可以直接破坏式调整。
3. 本阶段不处理业务侧（audio/backend/flutter）适配错误，只保证 plugin runtime 新契约成立。

## 3. Target Model

## 3.1 Control Plane vs Data Plane

1. Control Plane（`stellatune-plugins` runtime）负责：
   1. 插件加载/重载/禁用/卸载
   2. ModuleLease 交换（新 DLL 替换当前 lease）
   3. 追踪 lease 引用计数并在归零后卸载 DLL
   4. 向 Worker 发布生命周期控制信号（重建、销毁）
2. Data Plane（业务 Worker，例如 Decoder 线程）负责：
   1. 在所属线程创建并持有插件实例
   2. 高频方法直接调用实例（如解码 read）
   3. 在安全点执行重建/销毁

## 3.2 Threading Contract

1. 插件实例要求 `Send`（允许移动到 Worker 所在线程）。
2. 单实例只允许在所属 Worker 线程访问（`!Sync` 使用模型）。
3. Runtime 不串行代执行所有实例方法，不介入每次业务调用。

## 3.3 Lease Model

每个插件在 Runtime 里维护：

1. `current: Arc<ModuleLease>`（当前可用于新建实例的模块）
2. `enabled: bool`
3. `retired: Vec<Arc<ModuleLease>>`（可选，仅用于可观测和诊断）

`ModuleLease` 包含：

1. 动态库句柄（`Library`）
2. 模块函数表（`StPluginModule`）
3. 引用计数（由 `Arc` 自动维护）
4. 可选 `epoch`（仅用于日志与调试，不参与卸载判定）

卸载规则：

1. 当某个 retired lease 的 `Arc` 强引用计数归零时，即可卸载 DLL。
2. 不再要求 runtime 维护复杂 `generation/draining/inflight` 状态机作为主路径。

## 3.4 Worker Responsibility (Minimal)

Worker 只处理实例局部生命周期：

1. `create`
2. `recreate`（配置变更或 DLL 变化）
3. `destroy`
4. `drop lease`（随实例销毁自动释放）
5. `ack`（可选，仅用于可观测）

Worker 不负责：

1. 插件启用/禁用策略
2. lease 全局收敛判定
3. DLL 卸载时机决策
4. 跨实例协调

## 3.5 Worker-Facing Contract (Finalized)

目标：Worker 不关心插件加载/卸载细节，只做实例生命周期与业务调用。

Worker 侧只拿两样东西：

1. `factory`：用于创建当前插件实例
2. `control_rx`：接收 runtime 发来的控制消息

建议接口（命名可微调）：

1. `bind_worker_endpoint(plugin_id, capability, type_id) -> WorkerEndpoint`
2. `WorkerEndpoint { factory, control_rx }`
3. `factory.create_instance(config_json) -> PluginInstance`

`PluginInstance` 约束：

1. 对 Worker 暴露能力方法（decode/search/output...）
2. 内部隐式持有 `Arc<ModuleLease>`（Worker 不直接操作 lease）
3. `Drop` 时自动释放实例与 lease 引用

控制消息最小集合：

1. `Recreate { reason, seq }`
2. `Destroy { reason, seq }`

语义约束：

1. `Recreate`：Worker 在安全点销毁旧实例并用 `factory` 重建
2. `Destroy`：Worker 在安全点销毁实例并停止该插件路径
3. Worker 不参与 DLL 卸载判定；只要 drop 实例，runtime 自行按 lease 引用计数回收
4. 允许消息合并；Worker 只需处理“最新状态”即可
5. `ack` 为可选可观测能力，不作为正确性前提

## 4. Simplified Lifecycle

## 4.1 Runtime Plugin Slot

1. `Enabled + CurrentLease`
2. `Disabled + OptionalCurrentLease`
3. `RetiredLease(n)`（0..N）

Runtime 关键动作：

1. `reload`：创建新 lease 并替换 `current`，旧 lease 进入 retired 集合。
2. `disable`：禁止新建实例，可选择清空 `current`，等待旧实例自然退出。
3. `gc`：扫描 retired leases，强引用计数归零即卸载并移除。

## 4.2 Worker-local Instance Slot

1. `Running`
2. `PendingRecreate(reason)`
3. `PendingDestroy(reason)`
4. `Destroyed`

状态切换只在 Worker 安全点执行，避免打断实时数据路径。

## 5. Core Flows

## 5.1 Config Update

1. Worker 对当前实例执行 `plan_config_update_json`。
2. `HotApply`：同实例内立即应用。
3. `Recreate`：仅标记 `PendingRecreate`，在安全点销毁旧实例并创建新实例。
4. 可选执行 `export_state/import_state` 迁移。

## 5.2 DLL Changed (Reload via Lease Swap)

1. Runtime 加载新 DLL，创建 `new_lease`。
2. Runtime 执行 `current = new_lease`（原子替换）。
3. 旧 lease 加入 retired 集合，不再用于新实例创建。
4. Worker 在安全点重建时会使用新的 `current`（或收到显式重建通知）。
5. 当旧 lease 不再被任何实例持有（`Arc` 计数归零）时自动卸载 DLL。

## 5.3 Plugin Disabled

1. Runtime 先设置插件为 disabled（拒绝新建实例）。
2. Runtime 可广播 Worker 销毁意图（`PendingDestroy`）。
3. Worker 在安全点销毁实例并释放 lease。
4. 所有历史 lease 引用归零后，Runtime 完成 DLL 卸载。

## 6. Breaking API Direction (`stellatune-plugins`)

本阶段目标 API 方向（命名可微调）：

1. `load_or_reload_plugins(...) -> RuntimeSyncReport`
2. `set_plugin_enabled(plugin_id, enabled)`
3. `acquire_current_lease(plugin_id) -> Arc<ModuleLease>`
4. `retire_old_leases(plugin_id)`（可选后台 GC）
5. `list_plugin_runtime_leases(plugin_id)`（观测用途）

约束：

1. `stellatune-plugins` 不再假设自己拥有实例执行线程。
2. 能力实例具体方法调用不应强依赖 runtime actor 串行执行。
3. runtime 对外提供的是生命周期控制与状态查询，不是业务调用代理。
4. 以 lease 引用计数作为卸载主判定，不再以复杂代际状态机为核心。

## 7. Refactor Steps (Plugin-only)

## Phase A: Contract Freeze

1. 冻结 Worker-owned instance + lease 模型契约（本文件）。
2. 标记现有 generation/draining 显式流程为降级对象。
3. 确认 `service` 内仅保留 lifecycle 与 lease 管理语义。

Exit:

1. 契约被确认并作为后续实现唯一依据。

## Phase B: Service Decoupling

1. 将 `service` 内“生命周期控制”与“实例包装调用”拆分。
2. 引入 `ModuleLease` 持有与交换逻辑。
3. 清理以 generation 状态机为核心的冗余路径。
4. 清理重复 API（`Service` 与 `Handle` 的同构膨胀路径）。

Exit:

1. Runtime API 聚焦 lifecycle，不再承担业务方法执行职责。

## Phase C: Lease-driven Worker Hooks

1. 为 Worker 提供最小钩子协议：
   1. acquire current lease
   2. request recreate
   3. request destroy
2. Runtime 通过 lease 引用计数推进旧 DLL 回收。

Exit:

1. disable/reload/unload 全流程可在 plugin runtime 内自洽推进，且不依赖复杂代际显式状态推进。

## Phase D: Legacy Path Deletion

1. 删除旧实例执行耦合路径。
2. 删除仅为重代际状态机保留的辅助接口和桥接代码。
3. 保持 `stellatune-plugins` 内部代码以 lifecycle + lease 为中心。

Exit:

1. `stellatune-plugins` 中不存在旧执行模型主路径。

## 8. Acceptance Criteria (This Phase)

仅验证 plugin runtime 侧能力，不要求业务集成通过：

1. Runtime 在 reload 后不再向旧 lease 创建新实例。
2. 插件禁用后拒绝新实例注册。
3. 旧 lease 在最后一个实例销毁后可被回收并触发卸载。
4. DLL 变化场景下，旧 lease 在仍有实例持有时不会被卸载。
5. 无需依赖 `inflight_calls` 全局判定即可满足卸载安全性。

## 9. Out of Scope

1. `stellatune-audio` / `stellatune-backend-api` / FFI / Flutter 的适配与修复。
2. UI 行为一致性与端到端业务验证。
3. 跨进程沙箱或崩溃隔离。

## 10. Notes

1. 本文档只定义 `stellatune-plugins` breaking refactor 方向。
2. 业务侧将基于该契约在后续阶段逐步接入。
3. `epoch/generation` 可作为可观测字段保留，但不再作为卸载主控制面逻辑。

## 11. Crate Architecture Optimization

本节目标：在 `ModuleLease + 引用计数` 模型下，让 `stellatune-plugins` 保持易读、可维护、简洁。

## 11.1 Module Layout (Recommended)

建议将当前大体量 `service.rs` 拆分为以下模块（命名可微调）：

1. `src/runtime/model.rs`
2. `src/runtime/registry.rs`
3. `src/runtime/lifecycle.rs`
4. `src/runtime/actor.rs`
5. `src/runtime/handle.rs`

职责约束：

1. `model.rs` 只放核心结构体，不写流程逻辑。
2. `lifecycle.rs` 只放 `reload/enable/disable/gc` 生命周期流程。
3. `actor.rs` 只放串行控制面执行与命令派发。
4. `handle.rs` 只做线程安全薄封装，不重复业务逻辑。

## 11.2 Core Runtime Model

核心状态建议统一为：

1. `PluginSlot { enabled, current, retired }`
2. `ModuleLease { plugin_id, epoch, module, library }`
3. `RuntimeState { plugins: HashMap<PluginId, PluginSlot> }`

设计规则：

1. 卸载主判定只认 lease 引用计数，不引入第二套并行主判定。
2. `epoch` 仅用于日志、调试、追踪，不参与安全判定。
3. `retired` 可按需裁剪，仅保留必要可观测信息。

## 11.3 API Surface Minimization

`stellatune-plugins` 对外 API 收敛为生命周期控制与状态查询：

1. `reload_plugin` / `reload_all`
2. `set_plugin_enabled`
3. `acquire_current_lease`
4. `collect_garbage`（全局或按插件）
5. `list_runtime_state`

约束：

1. 不提供能力实例业务方法代理（decode/search/output）。
2. 不把 Worker 调度策略写入本 crate。
3. 不把业务缓存策略耦合进 runtime。

## 11.4 Capability Boundary

`src/capabilities/*` 仅保留 FFI 包装与能力类型适配：

1. unsafe 边界收口在能力层和 `util/common`。
2. 生命周期编排不进入能力实现文件。
3. 重复代码（plan/apply/export/import/drop）统一下沉公共 helper。

## 11.5 Event and Global State

建议将事件总线去全局化：

1. 避免跨 runtime 的全局 `OnceLock` 共享状态。
2. 由 `RuntimeHandle`/`RuntimeState` 显式持有事件通道。
3. 测试中可按实例构建独立 runtime，降低测试污染和耦合。

## 11.6 Concurrency Strategy (Simple by Default)

并发策略保持最小复杂度：

1. 控制面串行（actor 单线程或单写路径）。
2. 读多写少结构优先 `RwLock`。
3. 避免引入额外复杂并发结构作为第一实现。

## 11.7 Test Strategy

本 crate 的核心测试只覆盖生命周期不变量：

1. reload 后旧 lease 不再接新实例。
2. disable 后拒绝新实例创建。
3. 最后实例销毁后旧 lease 可卸载。
4. DLL 文件清理失败时可重试且不会破坏运行时状态。

## 11.8 Code Review Checklist

每次改动都应自检：

1. 是否把业务调用逻辑误放进 runtime 控制面。
2. 是否新增了第二套生命周期主判定。
3. 是否让 `handle` 与 `service/actor` 出现重复逻辑。
4. 是否引入不可观测的全局共享状态。
5. 是否破坏了“实例销毁即释放 lease”的核心约束。

## 12. Step-by-Step Execution Plan

本节用于实际施工顺序。要求严格按顺序推进，每步都可单独提交与回滚。

## 12.0 Progress Snapshot (2026-02-12)

1. `Step 0` Completed
2. `Step 1` Completed
3. `Step 2` Completed
4. `Step 3` Completed
5. `Step 4` Completed
6. `Step 5` Completed
7. `Step 6` Completed
8. `Step 7` Completed
9. `Step 8` Completed

Current landed changes:

1. Runtime 类型已从 `service.rs` 拆出到 `runtime/model.rs`、`runtime/actor.rs`（第一批）。
2. 模块槽位结构已拆出到 `runtime/registry.rs`（`PluginModuleLeaseSlotState`），旧 `PluginSlotState` 已删除。
3. `service.rs` 内部模块槽位语义已改为 `current/retired`（命名与 lease 模型对齐）。
4. `ModuleLease` 已上提为 runtime 模型类型，并增加 `current_module_lease_ref` 查询接口。
5. 实例运行时上下文已持有 `_module_lease`，实例生命周期可绑定 lease 引用生命周期。
6. retired lease 回收已切换为引用计数主路径：`collect_retired_module_leases_by_refcount` / `gc_plugin_retired_leases` 仅按 lease 强引用计数判定。
7. 新增显式回收入口 `collect_retired_module_leases_by_refcount`（service/handle），用于逐步替换旧代际回收触发路径。
8. `reload/unload/shutdown` 末尾已接入 retired lease 引用计数回收与 shadow cleanup 联动，减少对旧代际回收触发点的依赖。
9. 旧 draining 指标链路已下线（`runtime/observability.rs` 已删除），避免控制面继续暴露 generation/draining 语义。
10. `RuntimeActorState` / `RuntimeActorTask` / actor loop 已从 `service.rs` 抽离到 `runtime/actor.rs`，service 仅保留 handle 侧调用。
11. `PluginRuntimeHandle` 已抽离到 `runtime/handle.rs`，并通过 `service`/`lib` 保持兼容导出。
12. 旧 generation 回收兼容兜底路径已删除，卸载仅按 module lease 引用计数判定。
13. 已移除 `active_generation/slot_snapshot` 查询链路，以及 `PluginUnloaded.remaining_draining_generations` 事件字段。
14. 已移除 `handle` 对外 `deactivate_plugin/collect_ready_for_unload`，`service` 内部改为 `disable_plugin_slot/gc_plugin_retired_leases` 私有 lease 语义流程。
15. `service.rs` 中实例创建/能力查询代理路径（`create_*`、`prepare_create_context`、`decoder_candidates_for_ext` 等）已删除，runtime 主路径仅保留 lifecycle/control-plane 逻辑。
16. `lib.rs` 已取消 `pub use service::*`，对外导出面收敛为 `runtime/events/load`。
17. 事件总线已去全局化：`PluginEventBus` 由 `PluginRuntimeService` 实例持有；共享入口改为转发到 `shared_runtime_service()`，主路径不再依赖全局 `OnceLock<PluginEventBus>`。
18. `RuntimeLoadReport` 及 runtime actor 事件字段中的 `unloaded_generations` 已统一更名为 `reclaimed_leases`，控制面语义从 generation 术语切换到 lease 回收语义。
19. 旧 `types.rs` 中 generation/capability 描述结构（`CapabilityDescriptorRecord`/`PluginGenerationInfo`/`ActivationReport`）已移除，不再作为插件 crate API 面的一部分。
20. `service` 测试已改为 lease 控制面最小不变量验证（generation id 单调、未知插件 disable 行为、空回收 no-op）。
21. 已执行 `cargo check --workspace`：当前失败均来自业务侧对旧插件 API 的引用（如 `create_*`/`resolve_active_capability`/`active_generation`/`PluginRuntimeEvent` root 导出、`RuntimeLoadReport.unloaded_generations`），符合本阶段“仅重构 plugin runtime，不处理业务适配”范围。
22. 已移除 capability descriptor 代际链路：`LoadedModuleCandidate.capabilities`、`capability_records_for_generation`、`ActivationReport/CapabilityDescriptorRecord` 已删除，runtime 不再把 capability 描述绑定到 generation 控制面状态。
23. generation 元数据已从 slot 结构剥离，统一改由 `ModuleLease.metadata_json` 承载。
24. 已再次执行 `cargo check -p stellatune-plugins` 与 `cargo test -p stellatune-plugins` 均通过；`cargo check --workspace` 失败项仍为业务侧旧 API 引用，符合当前阶段范围。
25. 已进一步删除 `types.rs` 模块及 `lib.rs` 对其导出，彻底移除未被 runtime 使用的旧 capability 输入转换 API 面。
26. 已再次执行 `cargo check --workspace`，失败类型未变化：仍集中在业务侧对旧 root 导出与 `create_*`/`resolve_active_capability`/`active_generation`/`unloaded_generations` 等已移除 API 的引用。
27. 已移除 `gc_plugin_retired_leases_without_modules` 兼容分支：`gc_plugin_retired_leases` 仅按 module lease 回收主路径执行，不再对“缺少 module slot”的 plugin 走旧 draining 回收逻辑。
28. `GenerationCallGuard` 已移除，`InstanceRuntimeCtx::begin_call` 仅保留线程归属断言。
29. `service` 已删除 `slots` 状态与 `mark_draining_generations_unloaded` 链路；`list_active_plugins/active_plugin_ids/disable` 全部改为直接基于 `modules.current/retired`。
30. `ModuleLease` 新增 `metadata_json`，`list_active_plugins` 不再依赖 generation 槽位元数据。
31. `GenerationGuard` 已删除；`runtime/lifecycle.rs` 仅保留 `GenerationId`。`InstanceRegistry` 改为只追踪实例存活集合，不再关联 generation。
32. 已移除 `ModuleLease/ModuleLeaseRef` 上的 `generation` 字段，lease 模型进一步收敛为“模块引用 + 元数据”最小集合。
33. `service` 已删除 `next_generation/activate_generation` 链路，`activate_loaded_module` 不再接收 generation 参数；对应代际单调测试已下线。
34. `runtime/lifecycle.rs` 与 `runtime::lifecycle` 导出已删除，避免遗留“生命周期模块仅承载 GenerationId”空壳结构。
35. 已执行 `cargo check -p stellatune-plugins` 与 `cargo test -p stellatune-plugins`：当前 10 个单测全部通过。
36. 已新增 `runtime/worker_endpoint.rs`（Decoder 首版）：`bind_decoder_worker_endpoint(plugin_id, type_id)` 返回 `factory + control_rx`；Worker 可仅通过 `factory.create_instance(...)` 建立实例，不再关心加载/卸载细节。
37. `service/handle` 已新增当前 lease 获取主路径：`acquire_current_module_lease`，并在插件 disabled 时拒绝新建实例（返回 `None`）。
38. 控制消息桥接已接入 `PluginRuntimeEvent -> WorkerControlMessage`：当前支持 `Recreate`（lease 变更）与 `Destroy`（disable/unload/shutdown）两类信号。
39. `PluginRuntimeHandle` 已移除对外 `subscribe_runtime_events/subscribe_owner_runtime_events` 订阅入口，runtime actor 事件订阅不再作为主接入路径。
40. `events` 总线已新增 `plugin -> backend` 控制请求/响应通道：`send_control_json_utf8` 改为同步请求 backend 并返回真实响应；无 handler/超时/handler 丢弃响应时返回明确错误状态。
41. `events.rs` 中旧 `shared_runtime_service` 全局转发 helper（`drain/push/broadcast shared runtime events`）已删除，避免继续暴露旧 runtime 事件汇聚主路径。
42. `runtime/actor.rs` 已删除 `PluginRuntimeCommand/PluginRuntimeCommandOutcome/PluginRuntimeEvent` 及 owner/global subscriber 机制，actor 仅保留 lifecycle 状态访问与 `WorkerControlMessage` 分发职责。
43. `runtime/handle.rs` 已删除 `send_command`，并移除 lifecycle API 中对 `PluginRuntimeEvent` 的发射逻辑，控制面收敛为 `WorkerControl + backend_control_request + lifecycle query/mutation`。
44. `events.rs` 已删除 `plugin_to_host` 队列与 `push_plugin_event/drain_plugin_events`；插件 runtime 不再维护 `PluginRuntimeEvent` 出站缓冲。
45. `service.rs` 与 `runtime/handle.rs` 已删除 `push_runtime_notify_json` 与 `drain_runtime_events`，控制面 API 不再暴露 runtime event drain/notify 语义。
46. host vtable 已不再提供 `emit_event_json_utf8` 回调（置为 `None`），插件侧仅保留 `poll_host_event_json_utf8` 与 `send_control_json_utf8` 两条交互通道。
47. 新增 `runtime/worker_controller.rs` 通用 Worker 实例控制器：统一封装 `config update + pending recreate/destroy + control seq 去重 + 安全点 apply_pending`。
48. `runtime/worker_endpoint.rs` 已为 Decoder 接入通用控制器 trait（`WorkerConfigurableInstance` / `WorkerInstanceFactory`），并提供 `DecoderWorkerController` 别名与 `into_controller` 便捷入口。
49. `runtime/worker_endpoint.rs` 已补齐 `DSP / SourceCatalog / LyricsProvider / OutputSink` 的 `factory + endpoint + controller`，并为 `PluginRuntimeHandle` 增加 `bind_*_worker_endpoint` 对应入口。
50. `runtime/worker_endpoint` 已目录化拆分：`mod.rs` 保留统一类型声明，`decoder/dsp/source/lyrics/output/common` 分文件承载各 factory 实现与共享 helper，降低单文件复杂度。
51. crate 内联测试已迁移到独立测试文件：`src/tests/*` 与 `src/runtime/tests/*`，测试结构与生产代码解耦。
52. 新增真实动态库生命周期集成测试 `tests/lifecycle_dynamic_lib.rs`：覆盖 reload 后功能可用、disable 拒绝新建、旧 lease 引用保护与回收路径。
53. 新增多插件 worker 接入流测试 `tests/multi_plugin_runtime_flow.rs`：覆盖外部接入形态（factory + control_rx）、后台 worker 线程、hot apply/reload/disable 联动。
54. 新增多轮压力回归 `multi_plugin_reload_disable_hot_apply_stress_rounds`（12 轮）：交替 DLL 版本切换、反复 enable/disable、持续业务调用与状态断言。
55. 新增 DLL 清理失败可重试测试 `cleanup_failure_can_retry_after_handle_released`（Windows）：首次删除失败后在释放句柄后可重试成功。
56. 已执行并通过 `cargo test -p stellatune-plugins`（当前 18 unit + 4 integration），plugin runtime 本阶段测试矩阵闭环完成。
57. 已再次执行 `cargo check --workspace`：失败仍集中在业务侧旧 API 引用与类型不匹配，符合本阶段 out-of-scope 约束，不影响 plugin runtime 收口结论。

## Step 0: Freeze Baseline

目标：

1. 锁定当前行为基线，避免重构过程中误判回归。

改动范围：

1. 不改业务逻辑。
2. 仅补充/确认测试入口和日志开关。

验证：

1. `cargo check -p stellatune-plugins`
2. `cargo test -p stellatune-plugins`

退出条件：

1. 当前主干构建与测试可重复通过。

## Step 1: File Split Without Behavior Change

目标：

1. 拆分 `service.rs`，先做代码平移，不做语义调整。

改动范围：

1. 新增 `src/runtime/model.rs`
2. 新增 `src/runtime/registry.rs`
3. 新增 `src/runtime/lifecycle.rs`
4. 新增 `src/runtime/actor.rs`
5. 新增 `src/runtime/handle.rs`
6. 调整 `src/runtime/mod.rs` 导出
7. `src/service.rs` 仅保留 facade 和迁移期转发

验证：

1. `cargo check -p stellatune-plugins`
2. 现有测试全绿，行为无变化

退出条件：

1. `service.rs` 明显瘦身，核心逻辑已迁移到 `runtime/*`。

## Step 2: Introduce ModuleLease Model

目标：

1. 引入 `ModuleLease` 与 `PluginSlot { enabled, current, retired }`。

改动范围：

1. `src/runtime/model.rs` 定义核心结构
2. `src/runtime/registry.rs` 提供 `acquire_current_lease`
3. `src/runtime/lifecycle.rs` 增加 lease swap 和 retired 管理

验证：

1. 可以通过 runtime API 获取当前 lease
2. reload 后 `current` 已切换，旧 lease 进入 retired

退出条件：

1. 新实例创建路径已依赖 `current lease`。

## Step 3: Attach Lease to Instance Wrappers

目标：

1. 所有插件实例包装对象持有 `Arc<ModuleLease>`。

改动范围：

1. `src/capabilities/*.rs` 实例结构增加 lease 字段
2. 创建实例流程把 lease 注入实例
3. drop 路径保证 lease 随实例释放

验证：

1. 实例销毁后 lease 引用计数减少
2. 无悬挂强引用导致 retired 无法清理

退出条件：

1. “实例生命周期驱动 lease 生命周期”成立。

## Step 4: Switch Lifecycle Main Path to Lease GC

目标：

1. 卸载判定主路径切换为 retired lease 引用计数归零。

改动范围：

1. `src/runtime/lifecycle.rs` 实现 `collect_garbage`
2. 原 generation/draining 判定降级为观测字段或迁移期兼容逻辑
3. 清理 `inflight` 作为卸载硬条件的主路径依赖

验证：

1. 最后实例销毁后可自动触发 DLL 卸载
2. 实例仍存活时不会提前卸载

退出条件：

1. 卸载安全性由 lease 引用计数单一路径保障。

## Step 5: API Surface Cleanup

目标：

1. 对外 API 收敛为 lifecycle 控制和状态查询。

改动范围：

1. `src/runtime/handle.rs` 删除重复转发和膨胀接口
2. `src/service.rs` 删除实例业务调用代理型接口
3. `src/lib.rs` 导出面同步收敛

验证：

1. API 只保留：reload/enable/acquire lease/gc/state
2. 无业务方法调用代理残留

退出条件：

1. crate 职责边界清晰，不再承担 data-plane 调用执行。

## Step 6: Event Bus De-globalization

目标：

1. 事件总线从全局状态改为 runtime 实例持有。

改动范围：

1. `src/events.rs` 去掉跨 runtime 全局共享单例
2. 事件通道挂载到 `RuntimeState/RuntimeHandle`

验证：

1. 多 runtime 测试互不污染
2. reload/disable 事件路径保持可观测

退出条件：

1. 事件系统不依赖全局 `OnceLock` 主路径。

## Step 7: Delete Legacy Generation-centric Paths

目标：

1. 删除重代际状态机主流程和仅为其存在的代码。

改动范围：

1. 移除旧 generation/draining 主控制逻辑
2. 删除冗余结构与接口
3. 保留 `epoch/generation` 仅作观测字段（若仍有价值）

验证：

1. 代码中不存在“双主模型”并存
2. 文档术语和代码命名一致

退出条件：

1. lease 模型成为唯一生命周期实现。

## Step 8: Final Test Matrix and Docs Sync

目标：

1. 补齐不变量测试并完成文档闭环。

改动范围：

1. 增加或更新生命周期测试：
2. reload 后旧 lease 不接新实例
3. disable 后拒绝新建
4. 最后实例 drop 后自动卸载
5. DLL 文件清理失败可重试
6. 更新 `docs` 与代码注释，去掉迁移期说明

验证：

1. `cargo test -p stellatune-plugins`
2. `cargo check --workspace`（允许业务侧临时失败时跳过，但需记录）

退出条件：

1. plugin runtime 重构闭环完成，可进入业务侧接入阶段。
