# Flutter Windows 无障碍树诊断

`accessibility_bridge.cc` 的 AXTree 错误由原生引擎打印，不经过
`FlutterError.onError`。错误中的数字是本次运行的语义节点 ID；重新启动后
不能再用旧 ID 定位控件。

## 捕获实际现场

1. 完整重启 Debug APP（`flutter run -d windows`），让 `main()` 安装诊断入口。
2. 出现 AXTree 错误后，在 APP 窗口内按 **F8**。
3. 控制台会打印 `[semantics] Snapshot saved: ...`，文件位于系统临时目录。
4. 在文件里搜索错误对应的 `SemanticsNode#<id>` 或 `NODE <id>`，结合相邻
   节点和 `OWNER` 的 Widget 链定位控件。保留同一次运行的第一条原生错误。

快照记录 Dart 当前语义树和 RenderObject 对应的 Widget 链，无法直接读取
Windows 已经失步的 AXTree。如果报错节点已经移除，需要同时捕获触发操作前
的快照进行对比。诊断不会强制开启语义树，也不在每帧采集数据；Release 不安装
快捷键。快照可能包含当前歌曲名称和文件路径。

## 自动化排查

在 `apps/stellatune` 下执行：

```powershell
flutter test test/semantics_hover_test.dart
```

覆盖歌曲行、目录行、普通/可拖拽歌曲列表和窗口按钮之间的 Tooltip 悬停。
测试检查序列化更新中的子节点引用；这不是完整的 Windows AXTree 实现，
仍需要真实 Windows 引擎验证。

额外保留上游相邻 Tooltip 的复现用例，单独开启：

```powershell
flutter test --dart-define=RUN_FLUTTER_TOOLTIP_REPRO=true test/semantics_hover_test.dart --plain-name "upstream adjacent tooltip reproduction"
```

2026-09-05 在 Flutter 3.47.2 中该用例会失败并报告孤立的 Tooltip 节点。
这是 [Flutter #182444](https://github.com/flutter/flutter/issues/182444) 的对照场景，
不代表 APP 中任意 AXTree 错误都来自同一根因。

本次排查中，项目的五个悬停用例通过；真实 Windows APP 的首页、音乐库及播放
控制按钮的自动悬停没有复现 `Nodes left pending by the update`。尚未确定用户
原始日志的具体触发控件，因此未改动 Tooltip 或关闭无障碍支持。
