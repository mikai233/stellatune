// Developer-only fixture generator: node generate.mjs (requires ffmpeg).
// The checked-in NCM contains a synthetic 440 Hz tone, no commercial audio.
import { createCipheriv } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';

const u32 = value => { const bytes = Buffer.alloc(4); bytes.writeUInt32LE(value); return bytes; };
const encrypt = (data, key) => {
  const cipher = createCipheriv('aes-128-ecb', Buffer.from(key), null);
  return Buffer.concat([cipher.update(data), cipher.final()]);
};
const format = process.argv[2] ?? 'flac';
if (!['flac', 'mp3'].includes(format)) throw new Error('use flac or mp3');
const key = Buffer.from('stellatune-test-key');
const header = encrypt(Buffer.concat([Buffer.from('neteasecloudmusic'), key]), 'hzHRAmso5kInbaxW').map(b => b ^ 0x64);
const metadata = { musicName: 'Synthetic tone', musicId: 1, album: '', artist: [], bitrate: 128000, duration: 2000, format };
const info = Buffer.from("163 key(Don't modify):" + encrypt(Buffer.from('music:' + JSON.stringify(metadata)), "#14ljk_!\\]&0U<'(").toString('base64')).map(b => b ^ 0x63);
const audio = execFileSync('ffmpeg', ['-v', 'error', '-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=8000:duration=2', '-c:a', format === 'flac' ? 'flac' : 'libmp3lame', '-f', format, 'pipe:1']);
// Streaming ffmpeg leaves STREAMINFO's total sample count unknown; set it to 16000.
if (format === 'flac') {
const streamInfo = audio.readBigUInt64BE(18);
audio.writeBigUInt64BE((streamInfo & ~((1n << 36n) - 1n)) | 16000n, 18);
}
const box = Uint8Array.from({ length: 256 }, (_, i) => i);
let j = 0;
for (let i = 0; i < 256; i++) { j = (box[i] + j + key[i % key.length]) & 255; [box[i], box[j]] = [box[j], box[i]]; }
const payload = audio.map((byte, i) => { const j = (i + 1) & 255; return byte ^ box[(box[(box[j] + j) & 255] + box[j]) & 255]; });
writeFileSync(new URL(format === 'flac' ? 'tone.ncm' : 'tone-mp3.ncm', import.meta.url), Buffer.concat([
  Buffer.from('CTENFDAM'), Buffer.alloc(2), u32(header.length), header,
  u32(info.length), info, Buffer.alloc(5), u32(0), u32(0), payload,
]));
