# Plugin Runtime 简化待办（Breaking）

Status: Active  
Last Updated: 2026-02-12  
Owner: `stellatune-plugins` / `stellatune-audio` / `stellatune-library` / `stellatune-backend-api`  
Scope: Breaking redesign, no backward compatibility required.

## 1. 目标

1. 把插件运行时状态收敛到最小，减少并行状态源。
2. 把运行时生命周期控制统一到单一 Actor 线程。
3. 后续将实例执行模型收敛为强 Owner（每实例单 owner 线程/actor）。
4. 保证 DLL 卸载安全：不在 live instance / in-flight call 存在时卸载。

## 2. 已完成（防遗忘）

- [x] 代际状态简化（第一阶段）
  - 移除 `LifecycleStore` / `PluginSlotLifecycle` 并行生命周期层。
  - 代际切换与回收统一由 `service.rs` 内 `slots` 驱动。
  - `GenerationGuard` 仅保留实例计数、in-flight 计数、状态标记职责。

- [x] Plugin Runtime Actor 化（第二阶段）
  - `shared_runtime_service()` 不再暴露 `Arc<Mutex<PluginRuntimeService>>`。
  - 改为返回 `PluginRuntimeHandle`，通过请求/响应通道访问 runtime。
  - 新增 runtime actor 事件接口：注册 sender / 订阅事件。

- [x] 调用方迁移到 Actor Handle
  - `stellatune-audio`、`stellatune-library`、`stellatune-backend-api` 迁移完成。
  - 去除调用方直接 `.lock()` runtime 的路径。
  - backend runtime router 已接入 actor 事件并转发到 runtime event hub。

- [x] 收敛 capability 状态源（第二阶段补充）
  - `CapabilityRegistry` 全局索引已移除，capability 描述收敛到 generation 内部。
  - `resolve_active_capability` / `list_active_capabilities` 直接读取 active generation。
  - 删除 `CapabilityId`，实例注册改为基于 capability descriptor + active generation 校验。

## 3. 待办（下一阶段）

- [ ] 强 Owner 实例模型（第三阶段，核心）
  - 明确定义：每个插件实例只允许由其 owner 执行线程调用。
  - 控制面（load/reload/unload/config）仍走 runtime actor。
  - 数据面热路径（decode/process/write）不做每块 RPC，避免实时抖动。
  - 为实例创建 owner 边界检查（debug/assert + release 下错误返回）。
  - 进展（2026-02-12）：已加 owner 线程运行时检查（debug panic + release error log），用于提前发现跨线程误用。
  - 进展（2026-02-12）：插件实例类型已改为 `!Send/!Sync`，编译期阻止跨线程移动。
  - 进展（2026-02-12）：实例创建改为“两段式”（actor 线程准备上下文，调用线程执行实际 create）。
  - 进展（2026-02-12）：预加载链路不再跨线程传递 `EngineDecoder`；output sink 改为在 worker 线程内创建实例。

- [ ] 收敛实例状态源
  - 评估 `InstanceRegistry` 是否可删除或最小化。
  - 目标是让实例与 generation 关系由对象持有关系表达，而非全局表反查。
  - 进展（2026-02-12）：`InstanceRegistry` 已最小化为 `instance_id -> generation_guard`，移除 `plugin_id/type_id/kind` 冗余记录。
  - 进展（2026-02-12）：`InstanceUpdateCoordinator` 已移除 `in_flight/last_result` 全局表，仅保留代际号分配与结果构造。

- [ ] 完善 runtime actor 事件协议
  - 统一 actor runtime 事件 payload schema（topic / event / fields）。
  - 补齐文档与消费端解析约定（Settings debug、FFI、日志）。
  - 进展（2026-02-12）：新增 `PluginRuntimeCommand` + `CommandCompleted(request_id, owner, outcome)` 协议骨架，支持 API 侧“命令提交”和订阅侧“完成事件”解耦。
  - 进展（2026-02-12）：`stellatune-audio` 的 `ReloadPlugins` 已迁移为 Actor 命令流（控制线程发 `ReloadDirFromState`，完成事件回流 owner 线程处理），移除直接开线程调用 runtime 的旧路径。
  - 进展（2026-02-12）：新增 owner 定向事件订阅（`subscribe_owner_runtime_events(owner)`），`audio.control` 与 `audio.decode` 线程均已改为 owner mailbox，去除“全局广播后本地过滤”的桥接路径。
  - 进展（2026-02-12）：`output sink worker` 已接入 owner mailbox（`audio.output_sink`），在 worker 线程内直接消费 `PluginsReloaded/PluginUnloaded/PluginEnabledChanged/RuntimeShutdown` 并执行重建或退出。

- [ ] 退出与回收路径统一
  - 统一 hot-restart / shutdown / disable 时的 quiesce + collect + cleanup 流程。
  - 明确超时、剩余 draining generation 的可观测与重试策略。

## 4. 验收标准

- [ ] 代码中不再出现 runtime 全局锁访问路径（`shared_runtime_service().lock()`）。
- [ ] runtime 生命周期状态源不超过一处（单一真相来源）。
- [ ] 插件 disable/unload 在压力场景下可收敛（draining generation 不无限增长）。
- [ ] 强 Owner 模型下无跨线程并发调用同一实例。
- [ ] `stellatune-plugins`、`stellatune-audio`、`stellatune-library`、`stellatune-backend-api` 编译通过并关键测试通过。

## 5. 风险与注意事项

1. 不能把音频热路径改成“每帧/每块跨线程 RPC”，否则会引入延迟抖动。
2. 强 Owner 落地前，仍需保留 in-flight 保护，避免卸载竞态。
3. 迁移期间保持 runtime 事件可观测，便于定位 draining 不收敛问题。
