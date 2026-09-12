# 音频技术属性与码率

`stellatune-media-probe` 独立读取技术属性，依赖锁定为 Lofty 0.25.1。
它不提取歌曲标签或封面，不提供播放时长，不改变 Symphonia 的精确采样帧数、
CUE 边界、播放或 DLNA 解码流程。原文件始终只读，FFmpeg 仅用于生成测试样本和核对结果。

## 读取与准确性

入口 `probe(Read + Seek, format_hint, cancel)` 返回 `AudioProperties`、状态及实际读取字节数。
每次追加探测累计读取最多 8 MiB（重复读取也计入），读和定位前后检查取消和 2 秒截止时间；
每个进程最多两个探测同时运行，排队时间计入截止时间。截止时间是协作式的，不能中断
已经阻塞的操作系统 I/O；调用返回后会丢弃超时结果。超限不转为全文件扫描。
Lofty 的标签、封面和隐式标签转换读取均关闭。已有标签/封面提取的预算不属于此追加探测。

- 只读取 Lofty `audio_bitrate()`，从 kbps 换算为 bps；不使用 `overall_bitrate()`。
- MP1/2/3、裸 AAC/ADTS 标为估算平均值，不推断 CBR/VBR。Lofty 公共属性没有公开可靠的
  MPEG 码率模式及估算分支。短 MP3 的 Info/Xing 帧开销也可能影响返回值。
- M4A 的 AAC/ALAC、FLAC、Vorbis 等使用库返回的音频平均值；非正值、溢出或缺失保留未知。
  MPEG 层级、MP4 编码和 Ogg 编码来自实际文件解析，不能从容器扩展名猜测。
- PCM 的固定码率为 `sample_rate × storage_bytes_per_frame × 8`。
  小型容器描述读取器解析 WAV `fmt ` / WAVEFORMATEXTENSIBLE、AIFF/AIFC `COMM`、CAF `desc`。
  支持整数和浮点 PCM，24 个有效位存储在 32 位槽中仍按 32 位计算；CAF 用 packet 大小/帧数。
  非 PCM 不使用这个公式。Lofty 本身不支持 CAF；目前 CAF 的追加探测限于明确的 LPCM。
- 旧的 MP3 逐帧码率扫描器已删除。标签提取仍使用原有 Symphonia 流程。

## 扫描与缓存

统一能力表位于 `stellatune-media-probe::formats`。完整扫描、目录扫描、监听、CUE 文件名
回退匹配及内置解码器共用这张表。自动扫描 MP1/MP2/MP3/MPA、AAC、FLAC、WAV/WAVE、
AIFF/AIF/AIFC、M4A/M4B/M4R/ALAC、Ogg/OGA、CAF。MOV/MP4/3GP/3G2/M4P 保留直接打开能力，
不新增自动扫描。APE/WavPack/Opus 能否入库取决于实际安装的解码器；目前内置解码器不支持它们。
候选文件还需通过实际音频编码检查，例如 Opus 内容的 `.ogg` 不会作为 Vorbis 入库。

增量迁移 `0012_audio_properties.sql` 在 `audio_files` 上增加独立的 JSON 缓存、探测版本、
状态及缓存对应的 mtime/大小。普通扫描会补旧版本属性，保持歌曲标签、曲目 ID、收藏、
歌单、队列和 CUE 帧边界；无需重建。一个音频文件的所有 CUE 曲目共用一次探测结果。
没有改变文件指纹时，unsupported/invalid/budgetExceeded 不重复探测；强制扫描可以重试。
ioError/cancelled 在下次显式扫描重试，普通监听事件不反复重试。提交前重新检查指纹，
数据库 UPDATE 也限定原指纹，丢弃扫描中已经变化的文件结果。读取缓存时同样检查指纹。

## 插件与 NCM

`LocalFileMetadata.audio` 及 `inspect-file` 响应的 `audio` 是可选技术属性对象。
`audioProbeStatus` 可携带读取状态，未实现的插件保持未知。库的属性升级走
`MetadataProvider::probe_audio`，不会重写歌曲标签。

NCM 插件版本 0.2.3 使用共享探测模块。`AudioSlice` 将现有解密 reader 包装为从音频起点
计数、在音频结尾结束的资源，支持 Start/Current/End 定位并拒绝越界。
内部 MP3/FLAC 属性直接读取，不经过 HTTP、不缓存整首解密音频、不创建解密临时文件。
容器仍显示 NCM，编码来自真实解密内容。另修正了 ncmdump 0.8 在封面容量大于图片长度时
使用错误解密基址的问题：只在 reader 中呈现四字节的容量视图，实际图片长度仍单独保存，
不改磁盘文件。带预留填充的 NCM 也能得到与普通 payload 相同的属性和流内容。

`propertiesOnly: true` 只返回属性及状态，不发布封面或音频 URL；
`skipAudio: true` 供库先读取标签使用，随后只进行一次独立属性探测，避免重复读取。
旧版已安装 NCM 插件需要更新至 0.2.3 才能提供这些属性。

## 接口与界面

Rust、FRB 和队列序列化使用可选 `BitrateInfo`，不保留 `bit_rate` / `variable_bit_rate` 别名：

```json
{"bps":192000,"kind":"average","estimated":true,"mode":null}
```

`kind` 为 fixed/average/nominal，`mode` 可选 cbr/vbr/abr。
旧播放状态缺少该可选字段时仍可读取，不因技术属性缺失清空队列。
界面用 `约 192 kbps` 表示估算，明确模式才附加 CBR/VBR/ABR；详情区分平均、标称、固定值。
有损编码未知码率显示“未知”，无损/PCM 没有可靠码率时隐藏该项；CUE 标为“源文件平均码率”。
NCM 显示如 `NCM · FLAC · 24-bit · 96.0 kHz`。
专辑音源身份和混合规格判定均不包含码率。悬停、排序、详情和展示只读缓存。

## 验证

固定回归样本在 `crates/stellatune-media-probe/tests/fixtures`，生成命令见该目录 README。
测试覆盖 MP3 CBR/VBR、10 MiB 标签跳过、AAC/M4A/ALAC/FLAC/Vorbis/Opus、整数/浮点 PCM、
24-in-32、读取计数、取消/截止时间、并发上限、音频 slice 边界、三种扫描路径、属性升级保留
曲目和收藏、文件变化丢弃结果。NCM 测试比较包装后的属性和同一解密 payload 的属性，
插件进程测试覆盖 RPC、HTTP Range、关闭资源与无解密文件落盘。

2026-09-12 对 `D:\Music` / `D:\CloudMusic` 的代表性文件完成只读属性验收：

| 样本 | 结果 | 追加探测读取 |
| --- | --- | --- |
| 《爱尔兰画眉》MP3 | 约 128 kbps，44.1 kHz，2 声道 | 156 B |
| Revenge FLAC | 平均 860 kbps，16-bit / 44.1 kHz | 116 B |
| 太陽系ディスコ M4A | AAC，平均 194 kbps，44.1 kHz | 379 B |
| Magical Mirai 2017 AIFF | PCM 16-bit / 44.1 kHz，1411.2 kbps | 38 B |
| Project DIVA X Disc 2 WAV | PCM 16-bit / 44.1 kHz，1411.2 kbps | 36 B |
| Epic Happy Inspiring Orchestral NCM | 内部 FLAC，24-bit / 44.1 kHz，平均 1618 kbps | 有界解密 reader |

WAV/AIFF 精确码率与 ffprobe 一致，MP3 编码码率为 128000 bps；压缩格式的展示按
Lofty 音频属性公共接口的精度，不将文件整体码率当音频码率。真实文件与用户数据库未被修改。
