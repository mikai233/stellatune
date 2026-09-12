# 错误反馈与应用日志

业务错误统一走 `AppError → DiagnosticsService.report`。播放器异步失败、歌词和音乐库事件与 FFI 命令使用同一错误结构。内部 crate 的领域错误继续保留；FFI 根据类型映射类别，在诊断服务记录完整错误链和 Rust 堆栈，并返回操作标识、诊断 ID、根因指纹和上下文。Flutter 不解析 `AnyhowException.toString()`。

Controller 先完成回滚和过期请求检查，再上报错误。全局提示只包含本地化短文案与“查看日志”。主动操作失败保留提示；后台加载、事件流、全局异步异常和普通 ERROR 日志默认只记录，不弹通知。`failureMessage()` 默认静默，主动操作通过 `notify: true` 或 `report()` 明确请求提示。相同类别、根因和上下文在两秒内合并通知，命令与事件的日志记录仍分别保留。输出切换期间的异步设备错误由命令结果统一通知，保留原输出恢复与失败设备置灰。

## 全屏日志页

- 日志入口保留在设置页、启动失败页和操作失败提示中；播放栏不再展示日志按钮和未读错误数。
- 日志在 APP 内独立全屏显示，返回按钮或 Escape 关闭后回到原页面，保留滚动位置和播放状态。宽屏并列显示列表与详情，窄屏进入详情后可返回列表；关闭后日志更新不重建主页面。
- 页面过渡使用 `animations` 的 `SharedAxisTransitionType.scaled`（Shared axis Z），进入 300 ms、退出 250 ms。主页面和日志页分别使用 secondary/primary animation，一起完成纵深缩放和淡入淡出；窗口按钮在过渡层外保持固定。退出结束后才卸载日志导航器；退出途中重新打开会反向继续同一动画，不创建第二个日志导航器。遵循系统减少动画设置。
- 根反馈层位于 `MaterialApp.builder`，使用 `Overlay.wrap` 提供提示所需的 Overlay；日志页使用独立 Navigator 和 `HeroControllerScope.none`，为下拉菜单、Tooltip 提供正确祖先，避免与主导航共享 HeroController。不能把依赖 Overlay 的控件直接挂在主 Navigator 外。
- 动画预热通过 `rootOverlay` 插入临时 Navigator，同样必须使用 `HeroControllerScope.none`。否则根 Overlay 位于主 Navigator 之上时，预热会继承主 HeroController，导致冲突及后续 `_debugLocked` 断言。`startup_warmup_navigation_test.dart` 验证实际预热循环与主导航仍可正常 push/pop；加 `--dart-define=WARMUP_FULLSCREEN=true` 覆盖全屏预热分支。
- 页面只浏览本次运行的实时缓存，可筛选级别、来源、关键词，暂停自动滚动，查看完整详情、复制与导出当前日志。搜索直接筛选缓存，不查询历史文件；缓存仍受 5,000 条、8 MiB 上限约束。
- 不提供历史会话选择或“加载更多”。通过“打开日志文件夹”使用系统文件管理器查看历史 JSONL 文件，目录尚未初始化时入口禁用。
- “清空当前视图”只隐藏当前已有记录，不删除文件。详情读取已被淘汰且未保存的记录时会显示短状态；通知早于日志批次到达时只延迟重试一次，不随每条新日志重复查询。

## 数据流与限制

Rust tracing 与 FFI 诊断记录进入独立诊断线程，不使用播放事件流。生产端 `try_send` 有界队列最多 1,024 条、8 MiB，不能等待磁盘；丢弃数量以 WARN 记录。控制台同样使用后台 writer。实时缓存最多 5,000 条、8 MiB。FFI 每 100 ms 发送批次，订阅先注册再取快照，落后或重连后重新同步缓存。完整详情按需查询；底层历史分页接口保留，但日志页不再调用。

Flutter logger 保留原始多行消息和 Dart 堆栈，100 ms 批量送往 Rust。收到的 Rust 日志不会重新写回。启动前和 Rust 加载失败时，Flutter 独立保存、读取相同格式的 JSONL 日志；日志不可写时仍保留内存与控制台，避免日志服务递归报错。

应用支持目录下的 `logs/` 按会话、分段存储：每段 10 MiB，总量 100 MiB，七天保留。初始化与轮转时清理旧段，为当前段保留空间。导出是用户选择的文本副本，不影响源日志。不读取或迁移旧临时日志。

TypeScript runner 将 console 日志编码到 stderr，stdout 专用于 RPC；采集时保留级别、插件 ID 与进程 generation。NCM host 的错误通过所属插件 stderr 汇入。网易云侧车的 stderr 由所属插件继承转发。ASIO native host 的后台日志同时写入原文件与 stderr，宿主仅采集 stderr，不重复读取文件。安装目录中的旧 runner/host 不会自动变成新实现，需使用本次生成的 APP 与对应插件产物。

不记录 RPC 请求正文。采集前对 Authorization、Cookie、password、常见 token/API key 字段脱敏；后续新增日志也应避免写入业务凭据。

## 验证入口

- Rust：`cargo test -p stellatune-ffi -p stellatune-backend-api -p stellatune-audio-asio-adapter --features stellatune-audio-asio-adapter/test-host --lib --tests`；诊断测试覆盖分页/详情、历史、滚动/保留、缓存淘汰、慢消费者、溢出、存储不可用与错误关联。
- Flutter：`flutter test`；`diagnostics_test.dart` 按生产环境的 `MaterialApp.builder` 挂载方式验证悬停 Tooltip、下拉菜单、返回后滚动位置、缺失详情的有限重试，以及反馈去重、静默、启动降级、筛选、清空和关闭日志页不重建页面。浅色/深色、540/1200 像素截图输出到 `build/visual-review/diagnostics-*.png`。
- 插件：`cargo test -p stellatune-plugins --test typescript_runtime`；Node 测试运行 `tools/typescript-plugin-runtime/tests/bundle.test.mjs` 与 NCM/网易云插件测试。bundle 测试需设置 `STELLATUNE_TEST_RUNTIME_DIR` 指向新构建的 `plugin-runtime`，检查离仓安装及 stderr 的级别、多行内容和 RPC 隔离。
- 原生绑定：设置 `STELLATUNE_TEST_DIAGNOSTICS_DLL` 为构建后的 `stellatune_ffi.dll`，运行 `flutter test test/diagnostics_bridge_test.dart`。该测试实际加载 DLL，验证日志订阅、AppError 异常解码、诊断引用、脱敏和导出；普通测试环境未提供 DLL 时跳过。
- 硬件：`asio_smoke` 配合 `STELLATUNE_TEST_UNAVAILABLE_ASIO_DEVICE=asio:Realtek ASIO` 验证打开失败后恢复 `asio:USB DAC ASIO`。硬件占用会影响后续共享输出检查，不能以模拟测试代替实际声卡测试。

本次 Windows 验证使用 `target/diagnostics-windows` 独立 CMake 输出目录，避免覆盖正在运行的 APP；插件构建产物位于各插件的 `dist/`。

本机实测 Realtek 打开失败后恢复 SMSL 成功；同一 smoke 测试后续切回系统共享输出时返回资源占用，此步骤尚未通过硬件验证。ASIO 使用 0.2.3 安装包，NCM host 日志新增部分使用 0.2.2 安装包。
