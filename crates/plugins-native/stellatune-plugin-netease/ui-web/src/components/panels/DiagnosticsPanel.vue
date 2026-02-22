<script setup lang="ts">
import { ref } from "vue";
import UiSelect from "../UiSelect.vue";

const levelOptions = [
  { value: "standard", label: "标准（standard）" },
  { value: "higher", label: "较高（higher）" },
  { value: "exhigh", label: "极高（exhigh）" },
  { value: "lossless", label: "无损（lossless）" }
];

defineOptions({
  name: "DiagnosticsPanel"
});

const props = defineProps<{
  isBusy: boolean;
  hasGatewayContext: boolean;
  onInvokeSourceAction: (action: string, request: Record<string, unknown>) => Promise<Record<string, unknown>[]>;
}>();

const sessionStatus = ref("点击“刷新会话信息”查看当前认证会话。");
const sessionRaw = ref("");
const sessionSummary = ref("");

const lyricSongId = ref("");
const lyricStatus = ref("输入歌曲 ID 后可查看歌词。");
const lyricText = ref("");
const lyricRaw = ref("");

const urlSongId = ref("");
const urlLevel = ref("standard");
const urlStatus = ref("输入歌曲 ID 后可解析播放 URL。");
const urlResolved = ref("");
const urlRaw = ref("");

async function refreshSession(): Promise<void> {
  try {
    const rows = await props.onInvokeSourceAction("netease.auth.session", {});
    const payload = extractControlPayload(rows);
    sessionRaw.value = toPrettyJson(payload);
    sessionSummary.value = summarizeSession(payload);
    sessionStatus.value = "会话信息已刷新。";
  } catch (error) {
    sessionStatus.value = `刷新失败：${formatError(error)}`;
    sessionRaw.value = "";
    sessionSummary.value = "";
  }
}

async function fetchLyric(): Promise<void> {
  const songId = normalizeSongId(lyricSongId.value);
  if (!songId) {
    lyricStatus.value = "请先输入有效的歌曲 ID。";
    return;
  }
  try {
    const rows = await props.onInvokeSourceAction("netease.song.lyric", { song_id: Number(songId) });
    const payload = extractControlPayload(rows);
    lyricRaw.value = toPrettyJson(payload);
    lyricText.value = extractLyricText(payload) ?? "";
    lyricStatus.value = lyricText.value.trim().length > 0 ? "歌词加载完成。" : "接口返回成功，但未提取到可显示歌词。";
  } catch (error) {
    lyricStatus.value = `获取歌词失败：${formatError(error)}`;
    lyricRaw.value = "";
    lyricText.value = "";
  }
}

async function resolveSongUrl(): Promise<void> {
  const songId = normalizeSongId(urlSongId.value);
  if (!songId) {
    urlStatus.value = "请先输入有效的歌曲 ID。";
    return;
  }
  try {
    const rows = await props.onInvokeSourceAction("netease.song.url", {
      song_id: Number(songId),
      level: urlLevel.value
    });
    const payload = extractControlPayload(rows);
    urlRaw.value = toPrettyJson(payload);
    urlResolved.value = extractSongUrl(payload) ?? "";
    urlStatus.value = urlResolved.value ? "URL 解析完成。" : "接口返回成功，但未提取到 URL。";
  } catch (error) {
    urlStatus.value = `解析失败：${formatError(error)}`;
    urlRaw.value = "";
    urlResolved.value = "";
  }
}

function extractControlPayload(rows: Record<string, unknown>[]): unknown {
  if (rows.length === 0) {
    return {};
  }
  const first = rows[0];
  const payload = asRecord(first.playlist_ref);
  if (payload) {
    return payload;
  }
  return first;
}

function extractLyricText(payload: unknown): string | null {
  const payloadObj = asRecord(payload);
  const bodyObj = asRecord(payloadObj?.body);
  const dataObj = asRecord(bodyObj?.data);
  const lrcObj = asRecord(dataObj?.lrc);
  const yrcObj = asRecord(dataObj?.yrc);
  const tlyricObj = asRecord(dataObj?.tlyric);

  return (
    asText(lrcObj?.lyric) ??
    asText(yrcObj?.lyric) ??
    asText(tlyricObj?.lyric) ??
    asText(payloadObj?.lyric) ??
    null
  );
}

function extractSongUrl(payload: unknown): string | null {
  const payloadObj = asRecord(payload);
  const bodyObj = asRecord(payloadObj?.body);
  const dataObj = asRecord(bodyObj?.data);

  if (Array.isArray(dataObj?.items) && dataObj.items.length > 0) {
    const first = asRecord(dataObj.items[0]);
    return asText(first?.url);
  }

  return (
    asText(dataObj?.url) ??
    asText(payloadObj?.url) ??
    null
  );
}

function summarizeSession(payload: unknown): string {
  const payloadObj = asRecord(payload);
  const bodyObj = asRecord(payloadObj?.body);
  const dataObj = asRecord(bodyObj?.data);
  const profileObj = asRecord(dataObj?.profile);
  const accountObj = asRecord(dataObj?.account);
  const nickname = asText(profileObj?.nickname);
  const uid = asText(accountObj?.id) ?? asText(profileObj?.userId);
  const code = asText(bodyObj?.code);
  const cookieText = asText(payloadObj?.cookie);
  const pieces = [
    code ? `code=${code}` : "",
    uid ? `uid=${uid}` : "",
    nickname ? `昵称=${nickname}` : "",
    cookieText ? `cookie长度=${cookieText.length}` : ""
  ].filter((item) => item.length > 0);
  return pieces.length > 0 ? pieces.join("；") : "未识别到关键会话字段";
}

function normalizeSongId(raw: string): string | null {
  const trimmed = raw.trim();
  if (!/^\d+$/.test(trimmed)) {
    return null;
  }
  return trimmed;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function asText(value: unknown): string | null {
  if (typeof value === "string") {
    const trimmed = value.trim();
    return trimmed.length > 0 ? trimmed : null;
  }
  if (typeof value === "number" && Number.isFinite(value)) {
    return String(value);
  }
  return null;
}

function toPrettyJson(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function formatError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}
</script>

<template>
  <section class="panel">
    <div class="panel-head">
      <h2>诊断工具</h2>
    </div>

    <div class="diagnostics-grid">
      <article class="diagnostics-card">
        <div class="panel-head compact">
          <h3>会话诊断</h3>
          <div class="actions">
            <button :disabled="isBusy || !hasGatewayContext" @click="refreshSession()">刷新会话信息</button>
          </div>
        </div>
        <p class="hint">{{ sessionStatus }}</p>
        <p class="hint" v-if="sessionSummary">{{ sessionSummary }}</p>
        <details class="raw-panel" v-if="sessionRaw.trim()">
          <summary>查看原始响应（JSON）</summary>
          <pre class="payload">{{ sessionRaw }}</pre>
        </details>
      </article>

      <article class="diagnostics-card">
        <div class="panel-head compact">
          <h3>歌词预览</h3>
          <div class="actions">
            <button :disabled="isBusy || !hasGatewayContext" @click="fetchLyric()">获取歌词</button>
          </div>
        </div>
        <div class="grid">
          <label>
            歌曲 ID
            <input v-model="lyricSongId" type="text" placeholder="例如：33894312" />
          </label>
        </div>
        <p class="hint">{{ lyricStatus }}</p>
        <pre class="payload" v-if="lyricText.trim()">{{ lyricText }}</pre>
        <details class="raw-panel" v-if="lyricRaw.trim()">
          <summary>查看原始响应（JSON）</summary>
          <pre class="payload">{{ lyricRaw }}</pre>
        </details>
      </article>

      <article class="diagnostics-card">
        <div class="panel-head compact">
          <h3>音质与 URL 测试</h3>
          <div class="actions">
            <button :disabled="isBusy || !hasGatewayContext" @click="resolveSongUrl()">解析 URL</button>
          </div>
        </div>
        <div class="grid">
          <label>
            歌曲 ID
            <input v-model="urlSongId" type="text" placeholder="例如：33894312" />
          </label>
          <label>
            音质等级
            <UiSelect v-model="urlLevel" :options="levelOptions" />
          </label>
        </div>
        <p class="hint">{{ urlStatus }}</p>
        <p class="hint" v-if="urlResolved">解析 URL：{{ urlResolved }}</p>
        <details class="raw-panel" v-if="urlRaw.trim()">
          <summary>查看原始响应（JSON）</summary>
          <pre class="payload">{{ urlRaw }}</pre>
        </details>
      </article>
    </div>
  </section>
</template>
