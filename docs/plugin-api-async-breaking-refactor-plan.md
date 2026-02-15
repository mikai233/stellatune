# Plugin API Async Breaking Refactor Plan

Status: In Progress (Final Validation & Legacy Cleanup)  
Last Updated: 2026-02-14  
Owner: `stellatune-plugin-api` / `stellatune-plugin-sdk` / `stellatune-plugins` / `stellatune-audio` / `stellatune-backend-api` / Flutter host  
Scope: Breaking redesign only. No V1/V2 dual stack, no long-term compatibility shim.

Scope Update (2026-02-14):

1. Async ABI 范围收敛为 `SourceCatalog` / `LyricsProvider` control-plane。
2. `OutputSink` control-plane 回归同步 ABI（`list_targets/negotiate/open` 同步调用）。
3. 实时数据面（Decoder/DSP/Output write）继续保持同步。

## Execution Progress

1. `Phase 0` Completed (2026-02-14)
2. `Phase 1` Completed (2026-02-14)
3. `Phase 2` Completed (2026-02-14)
4. `Phase 3` Completed (2026-02-14)
5. `Phase 4` In Progress (2026-02-14)
6. `Phase 5` In Progress (2026-02-14)

## 1. Background

当前插件 ABI/SDK 在能力方法层基本是同步调用：

1. Source: `list_items_json` / `open_stream`。
2. Lyrics: `search_json` / `fetch_json`。
3. Output: `list_targets_json` / `negotiate_spec` / `open`。
4. 实时数据面（decode/dsp/output write）同样是同步调用。

已知问题是：部分 control-plane API（尤其 Source/Lyrics 的 IO 请求）会长时间占用调用线程，进而影响控制链路稳定性。

## 2. Goals

1. 把高延迟 control-plane API 改造成异步语义，避免阻塞引擎控制线程。
2. 保持实时数据面接口同步，避免音频路径引入额外调度抖动。
3. 给插件作者清晰的“哪些必须同步、哪些应异步”契约。
4. 以分阶段方式落地，确保每阶段可编译、可回归。

## 3. Non-Goals

1. 本阶段不引入跨进程插件沙箱。
2. 本阶段不改动业务功能语义（播放、队列、路由规则不变）。
3. 本阶段不要求对旧 ABI 做长期兼容。

## 4. API Contract Split (Must Sync vs Async)

## 4.1 必须保持同步（Hard Sync, 不允许异步化）

这些接口属于实时数据面，必须保持“直接调用 + 立即返回”：

1. `SourceStream::read/seek/tell/size`
2. `DecoderInstance::read_interleaved_f32`（`seek_ms` 仍保持同步）
3. `DspInstance::process_interleaved_f32_in_place`
4. `OutputSinkInstance::write_interleaved_f32`
5. `OutputSinkInstance::query_status/flush/reset/close/destroy`

原因：这些路径在 decode/output worker 的高频循环中执行，异步化会引入不可控 jitter。

## 4.2 应改造成异步语义（Control Plane Async）

这些接口允许 IO/网络/设备探测，应从宿主 API 语义上异步化：

1. `SourceCatalog::list_items_json`
2. `SourceCatalog::open_stream`（仅限“打开”阶段；流读仍同步）
3. `LyricsProvider::search_json`
4. `LyricsProvider::fetch_json`

说明：`OutputSink::list_targets_json/negotiate_spec/open` 在本轮收敛后保持同步 ABI，不进入 async-op 协议。

## 4.3 边界同步（可保持同步）

1. `plan/apply_config_update_json`
2. `export_state_json/import_state_json`
3. capability instance `create/destroy`

这些在安全点执行，可保持同步，但实现必须避免无界阻塞。

## 5. Architecture Decision

采用“两层异步化”策略：

1. Phase 1 先做宿主侧异步化：
   1. 保持当前插件 ABI 同步函数不变。
   2. 在 runtime query worker 中执行这些同步 FFI 调用。
   3. 对 Engine/Backend/Flutter 暴露异步 Future 语义。
2. Phase 2 再做 ABI 破坏升级（可选但推荐）：
   1. 升级 `STELLATUNE_PLUGIN_API_VERSION`（建议 `8 -> 9`）。
   2. 为 control-plane 能力引入异步操作句柄协议（begin/poll/cancel/wait）。
   3. 删除旧同步 control-plane vtable 方法。

说明：`extern "C"` 不能直接表达 Rust `Future`，因此 ABI 层必须用“异步操作句柄”表达异步。

## 5.1 SourceCatalog 单 Owner + StreamLease 生命周期（新增约束）

为避免多实例扩散、并保持音频数据面同步，本方案增加以下强约束：

1. `SourceCatalog` 按 `(plugin_id, type_id, lease_id)` 在宿主侧仅允许一个 owner 实例。
2. owner 必须运行在 Tokio worker 上，不再新开独立 `std::thread` 查询线程。
3. `list_items/open_stream` 通过 owner actor 异步请求执行。
4. `decoder` 仅在“打开轨道边界”对 owner 发起阻塞请求拿流句柄；进入解码后不再经 actor。
5. `SourceStream::read/seek/tell/size` 仍由 decode 线程同步直调。

## 5.2 StreamLease 模型

每个 `open_stream` 返回一个 `StreamLease`（逻辑结构）：

1. `stream_id`
2. `plugin_id/type_id`
3. `lease_id`（关联插件代际）
4. `io_vtable/io_handle`
5. `drop_close_token`（用于 RAII close）

owner actor 维护：

1. `current_catalog`
2. `retired_catalogs`
3. `open_streams: stream_id -> lease_id`

生命周期规则：

1. `OpenStream`：在 `current_catalog` 上打开流，登记 `open_streams`，返回 `StreamLease`。
2. `Decode`：decode 线程仅使用 `io_vtable/io_handle` 同步读流。
3. `Drop StreamLease`：向 owner 提交 `CloseStream(stream_id, io_handle, lease_id)`，由对应代际 catalog 执行 `close_stream`。
4. `Reload/Disable`：先 `freeze` 拒绝新 `open/list`，将 current 退役；待对应 `lease_id` 的 `open_streams==0` 后再销毁 catalog / 卸载 DLL。

## 5.3 ABI/Runtime 影响（Source 特化）

1. Source control-plane ABI 目标是异步操作句柄（Tokio worker 驱动）。
2. Source data-plane ABI（`StIoVTable`）保持同步。
3. 后续 ABI 可选增强：为 stream close 提供与 catalog 解耦的显式 close 回调，降低跨代 catalog 引用耦合。

## 6. Phase Plan

## Phase 0: Contract Freeze (1-2 days)

实现项：

1. 冻结本文件中的接口分类（Hard Sync / Control Async / Boundary Sync）。
2. 在 `plugin-api` / `plugin-sdk` 文档中同步契约。
3. 标记将删除的旧同步 control-plane API。

退出条件：

1. 团队对“哪些必须同步”达成一致。
2. 后续实现只按本契约推进。

进度（2026-02-14）：

1. 已冻结 Hard Sync / Control Async / Boundary Sync 分类。
2. 本文档已作为执行基线，后续实现按阶段推进。

## Phase 1: Host-side Async Orchestration (3-5 days)

实现项：

1. 在 `stellatune-audio` 将 runtime query 调用迁移到专用 worker 执行（不在 control thread 直接跑 IO）。
2. 为 Source/Lyrics/Output control query 增加并发上限、超时、取消。
3. 保持插件 ABI 不变，先通过线程模型消除阻塞传播。

退出条件：

1. `source_list_items/lyrics_search/fetch/output_list_targets` 不再阻塞控制线程。
2. 压测下控制线程 tick 抖动显著下降。

进度（2026-02-14）：

1. `stellatune-audio` 新增独立 runtime query worker（`stellatune-runtime-query`），控制面查询由 control thread 改为投递执行。
2. query worker 使用有界队列（cap=128）防止无界堆积；队列满/断连返回结构化错误。
3. `EngineCtrl` 的 `SourceListItemsJson/LyricsSearchJson/LyricsFetchJson/OutputSinkListTargetsJson` 已切换为异步回包路径。
4. `EngineHandle::send_engine_query_request` 增加 12 秒超时保护，避免调用方无限等待。
5. disable/reload 触发的 runtime query cache clear 已同步覆盖 worker 缓存（全量清理与按插件清理）。
6. 为减少重复实例，`EngineState` 已移除 `source_instances/lyrics_instances`，查询实例缓存收敛到 runtime query worker 单一持有；`output_sink_instances` 暂保留用于播放链路协商。
7. `output_sink_list_targets` 已回退到 control-thread owner（不经 runtime query worker），避免把音频相关能力查询迁移到额外线程，并减少 output capability 的重复实例来源。

## Phase 2: SDK Async Trait Surface (2-4 days)

实现项：

1. 在 `stellatune-plugin-sdk` 增加 Async 控制面 trait（Source/Lyrics control methods）。
2. 旧同步 trait 标记为 breaking-deprecated（同版本内删除）。
3. 为插件作者提供迁移模板（Netease Source 先迁移）。

退出条件：

1. 内置插件至少 1 个完成 async trait 迁移并通过回归。

进度（2026-02-14）：

1. `plugin-sdk` trait 面已完成收敛：
   1. `SourceCatalogInstance` / `LyricsProviderInstance` 使用 async trait。
   2. `OutputSinkInstance` / `DecoderInstance` / `DspInstance` 保持同步。
2. `ConfigUpdatable` 已回归同步接口（`plan/apply/export/import` 同步）。
3. 内置插件侧已对齐该签名收敛并通过编译回归。

## Phase 3: ABI v9 Async Control Ops (5-8 days)

实现项：

1. `plugin-api` 新增异步 control-op ABI（begin/poll/cancel/wait）。
2. Source/Lyrics 的 control-plane 方法切换到 async op（`begin/poll/wait/cancel/take/destroy`）。
3. Output 的 control-plane 回归同步 vtable 字段（不使用 async op）。
4. 删除旧同步（Source/Lyrics）或旧异步（Output 试验形态）控制面字段。
5. Source 落地 `StreamLease` 协议与 `CloseStream` 收敛语义（跨 reload/disable 可回收）。

退出条件：

1. `STELLATUNE_PLUGIN_API_VERSION` 升级完成。
2. Host + SDK + 内置插件在新 ABI 下全量通过编译。

进度（2026-02-14）：

1. `plugin-api` 已引入通用 `StAsyncOpState`，并完成 Source/Lyrics async control-op ABI：
   1. Source: `StSourceListItemsOpRef/VTable`, `StSourceOpenStreamOpRef/VTable`
   2. Lyrics: `StLyricsJsonOpRef/VTable`
2. Output ABI 已收敛回同步字段：`list_targets_json_utf8/negotiate_spec/open`。
3. Source/Lyrics 的实例 vtable 已切换为 `begin_*` control-plane 字段。
3. `STELLATUNE_PLUGIN_API_VERSION` 已从 `8` 升级到 `9`。
4. `plugin-sdk` 已完成分层导出：
   1. Source/Lyrics: async op 导出协议。
   2. Decoder/DSP/Output: 同步导出协议。
5. `stellatune-plugins` 已完成 capability wrapper 与 worker endpoint 对齐：
   1. Source/Lyrics: `begin -> wait -> take/finish -> destroy`。
   2. Output: 直接同步 vtable 调用。
6. `StHostVTable` 与 module factory 已切换到 `begin_*` 语义并在宿主侧对齐。
7. 当前验证结果：`stellatune-plugin-api` / `stellatune-plugin-sdk` / `stellatune-plugins` / 内置插件编译通过。

## Phase 4: Runtime/Backend/Flutter Integration (3-5 days)

实现项：

1. `stellatune-plugins` capability wrapper 全量对齐新 ABI（Source/Lyrics async-op，Output 同步）。
2. `stellatune-backend-api` 与 `stellatune-ffi` 接口统一为 async await。
3. Flutter bridge 保持 Future API，不再承担额外同步桥接。
4. Source owner actor 统一迁移到 Tokio worker 执行模型（不再额外开查询线程）。

退出条件：

1. Flutter 端所有插件 control 调用链为纯异步语义。
2. 不再出现同步 control API 残留调用。

进度（2026-02-14）：

1. `stellatune-plugins` capability wrapper 已按新分层完成迁移。
2. `worker_endpoint` 已切换到 `begin_create_*` 异步实例创建协议。
3. `stellatune-audio` 的 Source/Lyrics control owner 已迁移为「每 `(plugin_id,type_id)` 单 Tokio task」模型，移除单查询线程模型。
4. Source owner 已落地 `StreamLease` 生命周期基础模型：
   1. `RuntimeSourceStreamLease` 包含 `lease_id`。
   2. owner 维护 `current + retired catalogs`，按 `stream_id -> lease_id` 回收流。
   3. 配置切换时若旧 lease 仍有活跃流，旧实例进入 `retired`，在边界 close 后回收。
5. disable/unload 边界已接入 `freeze`：
   1. clear cache for plugin/all 会先向 Source/Lyrics owner 发送 freeze（等待 ack）。
   2. Source 若仍有活跃流则保持 frozen 并拒绝新 control 请求，待流清零后销毁。
6. Source/Lyrics control-op wrapper 增加 `wait slice + timeout + cancel` 语义，避免无限阻塞。
7. backend-api / ffi / Flutter bridge 仍待按 Phase 4 继续收口。
8. `stellatune-audio` 已完成 owner lifecycle 清理：
   1. 删除 Source 旧 dead path（旧 `apply_or_recreate_source_instance` 路径）。
   2. `runtime_query` 中 `V2` 临时命名已统一收口。
   3. `CachedSourceInstance` 旧冗余字段已移除。

## Phase 5: Legacy Deletion (1-2 days)

实现项：

1. 删除旧同步 control-plane ABI/SDK/host 代码路径。
2. 删除临时兼容适配层。
3. 更新开发文档与插件模板。

退出条件：

1. 仓库内无旧 control-plane 同步 ABI 可达代码。

## 7. File-level Impact (Expected)

1. `crates/stellatune-plugin-api/src/source.rs`
2. `crates/stellatune-plugin-api/src/lyrics.rs`
3. `crates/stellatune-plugin-api/src/output.rs`
4. `crates/stellatune-plugin-api/src/lib.rs`（ABI version bump）
5. `crates/stellatune-plugin-sdk/src/instance.rs`
6. `crates/stellatune-plugin-sdk/src/export/plugin_macro/source_modules.rs`
7. `crates/stellatune-plugin-sdk/src/export/plugin_macro/lyrics_modules.rs`
8. `crates/stellatune-plugin-sdk/src/export/plugin_macro/output_modules.rs`
9. `crates/stellatune-plugins/src/capabilities/source.rs`
10. `crates/stellatune-plugins/src/capabilities/lyrics.rs`
11. `crates/stellatune-plugins/src/capabilities/output.rs`
12. `crates/stellatune-audio/src/engine/control/runtime_query.rs`
13. `crates/stellatune-backend-api/src/player.rs`
14. `crates/stellatune-ffi/src/api/player/mod.rs`
15. `apps/stellatune/lib/bridge/bridge.dart`

## 8. Verification Plan

1. Scenario A: ASIO 播放中加载 Netease Source，连续触发 source list + lyrics search/fetch。
2. Scenario B: 插件 enable/disable + apply state 期间并发触发上述 control API。
3. Scenario C: 重复热重启 + 插件 control API 压测。
4. Scenario D: 实时数据面回归（decode/dsp/output write）延迟与稳定性。

关键指标：

1. 控制线程插件调用期间 p99 阻塞时间。
2. control API 超时率。
3. `Lost connection to device` 复现率。
4. 音频 underrun 指标无回归。

自动化验证进度（2026-02-14）：

1. `cargo check -p stellatune-audio -p stellatune-plugins -p stellatune-backend-api -p stellatune-ffi` 通过。
2. `cargo test -p stellatune-audio -p stellatune-plugins --lib` 通过（`stellatune-plugins` 18 tests passed，`stellatune-audio` 当前无 lib tests）。
3. ASIO + Netease 手工场景验证仍需按 Scenario A/B/C/D 执行。

## 9. Acceptance Criteria

全部满足才算完成：

1. Control-plane API 异步化范围（Source/Lyrics）完成，Output 保持同步并完成新 ABI 对齐。
2. Hard Sync 列表接口保持同步且性能无回归。
3. 旧同步 control-plane ABI/SDK/Host 路径已删除。
4. Netease Source 在新契约下稳定运行。

## 10. Open Decisions

1. Phase 2/3 是否同一个大版本一次完成，还是先落地 Phase 1 观察一轮。
2. Async control-op 协议是统一通用句柄，还是按 capability 分别定义。
3. 超时默认值与取消语义（host cancel 是否必须由插件实现可中断）。
