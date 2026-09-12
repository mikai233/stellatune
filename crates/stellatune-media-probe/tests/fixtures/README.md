# Synthetic property fixtures

All tones are generated test data, 997 Hz, 48 kHz stereo, 0.25 seconds. No FFmpeg runtime dependency.

```powershell
ffmpeg -f lavfi -i 'sine=frequency=997:sample_rate=48000:duration=0.25' -ac 2 -c:a CODEC OPTIONS OUTPUT
```

| Output | Codec | Options |
| --- | --- | --- |
| cbr.mp3 | libmp3lame | -b:a 128k |
| no-info.mp3 | libmp3lame | -b:a 128k -write_xing 0 |
| layer2.mp2 | mp2 | -b:a 192k |
| vbr.mp3 | libmp3lame | -q:a 4 |
| adts.aac | aac | -b:a 128k |
| aac.m4a | aac | -b:a 128k |
| alac.m4a | alac | -ac 2 |
| tone.flac | flac | -sample_fmt s32 |
| vorbis.ogg | libvorbis | -q:a 4 |
| opus.ogg | libopus | -b:a 64k |
| float.caf | pcm_f32le | -ac 2 |
| integer.aiff | pcm_s24be | -ac 2 |

The short MPEG Info-header fixture's average may differ from the encoder target.
Tests require the conservative estimated flag, not a fabricated exact CBR mode.
WAVEFORMATEXTENSIBLE storage-width cases are generated directly in Rust tests.
