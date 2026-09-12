# CUE 分轨

外置 CUE 将一个音频文件映射为多首曲目，不修改源文件，不生成永久分轨文件。
本机播放和 DLNA 共用 `AudioSegment` / `SegmentDecoder` 的采样帧区间。

## 数据与扫描

- `audio_files` 保存规范化路径、mtime/大小指纹、源采样率、总采样帧数、PCM 精度和元数据缓存。
- `tracks.file_id` 关联物理文件。普通曲目键为 `file:<规范化路径>`；CUE 曲目键为 `cue:<规范化 CUE 路径>:<TRACK 编号>`。
- 曲目 ID、播放器 TrackId 和队列 occurrence ID 各自独立。文件路径只用于访问资源，相同文件的分轨不会合并收藏或队列项。
- 封面通过文件级 `cover_key` 共享，UI 使用 `coverId` / `localCoverId`，不能用曲目 ID 推断封面路径。
- `worker/cue_import.rs` 是完整扫描、强制扫描、目录恢复及文件监听共用的入口。先解析并验证整份文档，再事务更新曲目和整轨入口。
- 解析 FILE、TRACK AUDIO、TITLE、PERFORMER、INDEX，保留 REM DATE / DISCNUMBER。曲目标题和艺术家依次回退到 CUE 专辑级标签、音频元数据。多人艺术家沿用现有拆分规则。
- INDEX 01 定义起点；同一文件下一首 INDEX 01 定义终点，末首到文件结尾。保留原始 1/75 秒 CUE 帧，以整数算术换算源采样帧。INDEX 00 间隙随前曲，不合成 PREGAP/POSTGAP，不额外展示隐藏曲。
- BOM、UTF-8 优先；传统编码使用 chardetng 与引用文件存在性辅助判断，存在无法消除的歧义时拒绝导入。引用路径必须位于授权扫描目录内，并遵守排除目录。
- 文件优先精确匹配；同名且只有一个不同扩展名的音频候选时可替代并记录日志，多候选拒绝猜测。
- 完全相同的 CUE 合并；冲突文档不任意选取，恢复整轨入口。已有文档暂时不完整时保留最后有效版本；确认删除后恢复整轨。首次失败不隐藏可扫描的音频文件。
- 未变化文档不重写曲目；强制重扫保留曲目 ID，同批多个文档复用文件探测。后台失败仅记日志。

## 播放

`LocalTrackResolver::resolve_resource` 解析曲目 ID，返回文件资源与可选区间。
区间应用在解码器之后、重采样与音效之前。适配器吸收 seek 提前落点，丢弃区间前采样，严格裁剪末块并返回 EOF；编码首尾 padding 仅处理一次。

播放事件、时长、seek 和保存位置都使用单曲相对时间。解码器直接按采样帧 seek；采样率之间的帧数换算不经过毫秒。
同文件相邻区间顺序播放时覆盖边界淡入淡出／交叉淡化策略，使用现有预加载与无缝晋升机制。切换歌曲或队列顺序不改变曲目身份。

## DLNA

`dlna_play_local_track(renderer, library_track_id)` 由后端解析单曲。普通曲目发布原文件；CUE 曲目发布独立虚拟 WAV，DIDL 使用单曲元数据、时长、大小、采样率、声道和共享封面。

- 保持采样率和声道；接受可精确保留的整数 PCM 16/24 位、浮点 PCM 32 位。未知精度、整数 32 位等不能经现有 f32 解码链保留的格式明确报错。
- 标准 RIFF/WAV 长度限制包含头和偶数字节 padding，超限报错。多声道使用 WAVEFORMATEXTENSIBLE；浮点输出包含 fact。
- 每个读取请求独立定位、解码，通过容量为 2 的通道背压输出；同时最多 4 个解码请求。没有整张专辑的内存缓存或永久临时音频。
- 支持完整 GET、HEAD、单段字节 Range，包括头部范围、后缀范围和非采样帧对齐范围。HEAD 忽略 Range；有效范围返回 206，越界返回 416 及 `Content-Range: bytes */length`。未知单位和不支持的多段请求忽略 Range。[依据 RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html#name-range-requests)。
- 断开请求会释放解码状态；停止／切换输出撤销资源；成功切歌回收旧令牌，失败发布回收新令牌；兜底最多保留 16 个发布令牌。
- ConnectionManager 明确声明不支持 WAV 时拒绝投送；能力不可查询时按原格式尝试，由设备响应错误，不静默转码。
- 设备获得单曲时间线。不同设备的无缝切歌能力不作保证。

## 重建与范围

音乐库结构版本与播放结构版本均为 2，不提供旧接口或旧曲目关系迁移。
启动检测明确的版本不匹配，展示“重建音乐库”；只有点击后才执行事务重建。
重建清空曲目、歌单关联、收藏、播放队列／位置及旧封面缓存，保留扫描／排除目录、歌单名称、设置、媒体来源和插件配置。
重建完成后重启并重新扫描。任意数据库读取、SQL 或文件访问错误不会自动触发删库。

本期不含内嵌 CUESHEET、CUE 编辑、无 CUE 自动分轨。音频必须能被当前解码链识别并提供可靠帧数与定位，缺少支持的 TTA 等仍不能分轨。

## 验证与复验

自动测试覆盖解析与编码、重复／冲突、首次导入失败、暂时损坏、强制扫描、监听修改／删除、稳定 ID、共享封面、独立收藏／队列及相对 seek；已知采样序列检查起止裁剪、随机 seek、连续片段、整数／浮点精度和 Range 拼接。HTTP 测试检查 HEAD、206/416、字节范围和令牌回收。

2026-09-12 对 `D:\Music` 的验收采用临时数据库和封面目录，源文件只读。
初次通过 46 份 CUE、55 个音频文件、513 首分轨，重扫 ID 不变。
另对《ELZA》的 FLAC 和《雪あかりの夜想曲》的 WAV 分别抽取首／中／末曲验证实际解码与非对齐 Range 拼接，均通过（44.1 kHz、16 位）。
发现的不支持音频／标签或不确定编码会保留失败日志；不会强行写入乱码。

当前局域网发现 0 台 DLNA renderer，**真实设备播放、拖动、切歌尚未验收**。

复验命令（只读音乐目录，数据库与封面写到临时目录）：

```powershell
$env:STELLATUNE_CUE_TEST_ROOT = 'D:/Music'
cargo test -p stellatune-library real_music_cue_import_read_only -- --ignored --nocapture
$env:STELLATUNE_CUE_ALBUM_ROOT = 'D:/Music/包含外置 CUE 的专辑目录'
cargo test -p stellatune-backend-api real_album_segments_read_only -- --ignored --nocapture
cargo test -p stellatune-ffi discover_real_cue_renderers -- --ignored --nocapture
```
