<script setup lang="ts">
import { ref } from "vue";
import UiSelect from "../UiSelect.vue";

interface DiscoveryItem {
  kind: string;
  itemId: string;
  title: string;
  subtitle: string;
  artist: string;
  album: string;
  trackCountText: string;
  sourceId: string;
  trackId: string;
  pathHint: string;
  extHint: string;
  trackPayload: Record<string, unknown> | null;
  playlistRef: Record<string, unknown> | null;
}

defineOptions({
  name: "DiscoveryPanel"
});

const props = defineProps<{
  isBusy: boolean;
  hasGatewayContext: boolean;
  pluginId: string;
  sourceTypeId: string;
  sourceConfig: Record<string, unknown>;
  onInvokeSourceAction: (action: string, request: Record<string, unknown>) => Promise<Record<string, unknown>[]>;
  onRunPlaybackAction: (action: string, payload?: Record<string, unknown>) => Promise<void>;
}>();

const levelOptions = [
  { value: "standard", label: "标准（standard）" },
  { value: "higher", label: "较高（higher）" },
  { value: "exhigh", label: "极高（exhigh）" },
  { value: "lossless", label: "无损（lossless）" }
];

const searchKeywords = ref("");
const searchLimit = ref(30);
const searchOffset = ref(0);
const searchLevel = ref("standard");
const searchStatus = ref("输入关键词后点击搜索。");
const searchItems = ref<DiscoveryItem[]>([]);

const playlistLimit = ref(30);
const playlistOffset = ref(0);
const playlistItems = ref<DiscoveryItem[]>([]);
const playlistStatus = ref("点击“加载歌单”查看账号歌单。");
const selectedPlaylistId = ref("");
const selectedPlaylistTitle = ref("");
const playlistTracks = ref<DiscoveryItem[]>([]);
const playlistTrackStatus = ref("未选择歌单。");

async function runSearch(): Promise<void> {
  const keywords = searchKeywords.value.trim();
  if (keywords.length === 0) {
    searchStatus.value = "请输入关键词。";
    searchItems.value = [];
    return;
  }
  try {
    const rows = await props.onInvokeSourceAction("search", {
      keywords,
      limit: normalizeLimit(searchLimit.value, 30),
      offset: normalizeOffset(searchOffset.value),
      level: searchLevel.value
    });
    searchItems.value = normalizeItems(rows).filter((item) => item.kind === "track");
    searchStatus.value = `搜索完成，共 ${searchItems.value.length} 条歌曲结果。`;
  } catch (error) {
    searchItems.value = [];
    searchStatus.value = `搜索失败：${formatError(error)}`;
  }
}

async function loadPlaylists(): Promise<void> {
  try {
    const rows = await props.onInvokeSourceAction("list_playlists", {
      limit: normalizeLimit(playlistLimit.value, 30),
      offset: normalizeOffset(playlistOffset.value)
    });
    playlistItems.value = normalizeItems(rows).filter((item) => item.kind === "playlist");
    playlistStatus.value = `歌单加载完成，共 ${playlistItems.value.length} 条。`;
    if (playlistItems.value.length === 0) {
      selectedPlaylistId.value = "";
      selectedPlaylistTitle.value = "";
      playlistTracks.value = [];
      playlistTrackStatus.value = "当前无可用歌单。";
    }
  } catch (error) {
    playlistItems.value = [];
    playlistStatus.value = `加载失败：${formatError(error)}`;
  }
}

async function loadPlaylistTracks(item: DiscoveryItem): Promise<void> {
  const request = buildPlaylistTrackRequest(item);
  if (!request) {
    playlistTrackStatus.value = "该歌单缺少可识别的 playlist_id。";
    return;
  }
  selectedPlaylistId.value = item.itemId;
  selectedPlaylistTitle.value = item.title;
  try {
    const rows = await props.onInvokeSourceAction("playlist_tracks", request);
    playlistTracks.value = normalizeItems(rows).filter((entry) => entry.kind === "track");
    playlistTrackStatus.value = `“${item.title}” 已加载 ${playlistTracks.value.length} 首歌曲。`;
  } catch (error) {
    playlistTracks.value = [];
    playlistTrackStatus.value = `加载歌单歌曲失败：${formatError(error)}`;
  }
}

async function playNow(item: DiscoveryItem): Promise<void> {
  const trackRef = buildTrackRef(item);
  if (!trackRef) {
    return;
  }
  await props.onRunPlaybackAction("playback.play_track_ref", { track_ref: trackRef });
}

async function enqueue(item: DiscoveryItem): Promise<void> {
  const trackRef = buildTrackRef(item);
  if (!trackRef) {
    return;
  }
  await props.onRunPlaybackAction("playback.enqueue_track_ref", { track_ref: trackRef });
}

function buildPlaylistTrackRequest(item: DiscoveryItem): Record<string, unknown> | null {
  if (item.playlistRef) {
    return {
      playlist_ref: item.playlistRef,
      limit: 200,
      offset: 0,
      level: searchLevel.value
    };
  }
  const playlistId = parseUnsignedInt(item.itemId);
  if (playlistId === null) {
    return null;
  }
  return {
    playlist_ref: {
      playlist_id: playlistId
    },
    limit: 200,
    offset: 0,
    level: searchLevel.value
  };
}

function buildTrackRef(item: DiscoveryItem): Record<string, unknown> | null {
  const trackId = item.trackId.trim();
  if (trackId.length === 0) {
    return null;
  }
  const sourceId = item.sourceId.trim() || props.sourceTypeId.trim() || "netease";
  const trackPayload = buildTrackPayload(item);
  if (!trackPayload) {
    return null;
  }
  const extHint = item.extHint.trim() || "mp3";
  const pathHint =
    item.pathHint.trim().length > 0
      ? item.pathHint.trim()
      : `${sourceId}:${trackId}.${extHint}`;
  const locatorPayload: Record<string, unknown> = {
    plugin_id: props.pluginId,
    type_id: sourceId,
    config: props.sourceConfig,
    track: trackPayload,
    ext_hint: extHint,
    path_hint: pathHint
  };
  const locator = JSON.stringify(locatorPayload);
  return {
    source_id: sourceId,
    track_id: trackId,
    locator
  };
}

function buildTrackPayload(item: DiscoveryItem): Record<string, unknown> | null {
  if (item.trackPayload) {
    return item.trackPayload;
  }
  const songId = parseUnsignedInt(item.trackId);
  if (songId === null) {
    return null;
  }
  return {
    song_id: songId,
    level: searchLevel.value,
    stream_url: null,
    ext_hint: item.extHint.trim() || "mp3"
  };
}

function normalizeItems(rows: Record<string, unknown>[]): DiscoveryItem[] {
  return rows.map((row) => {
    const kind = asText(row.kind) ?? "unknown";
    const itemId = asText(row.item_id) ?? asText(row.track_id) ?? "";
    const title = asText(row.title) ?? itemId ?? "未命名条目";
    const subtitle = asText(row.subtitle) ?? "";
    const artist = asText(row.artist) ?? "";
    const album = asText(row.album) ?? "";
    const sourceId = asText(row.source_id) ?? "netease";
    const trackId = asText(row.track_id) ?? readNestedTrackId(row) ?? "";
    const pathHint = asText(row.path_hint) ?? "";
    const extHint = asText(row.ext_hint) ?? readNestedExtHint(row) ?? "";
    const trackPayload = asRecord(row.track);
    const trackCount = asNumber(row.track_count);
    const playlistRef = asRecord(row.playlist_ref);
    return {
      kind,
      itemId,
      title,
      subtitle,
      artist,
      album,
      trackCountText: trackCount === null ? "" : `${trackCount} 首`,
      sourceId,
      trackId,
      pathHint,
      extHint,
      trackPayload,
      playlistRef
    };
  });
}

function readNestedTrackId(row: Record<string, unknown>): string | null {
  const trackObj = asRecord(row.track);
  if (!trackObj) {
    return null;
  }
  const direct = asText(trackObj.song_id);
  if (direct) {
    return direct;
  }
  return asText(trackObj.track_id);
}

function readNestedExtHint(row: Record<string, unknown>): string | null {
  const trackObj = asRecord(row.track);
  if (!trackObj) {
    return null;
  }
  return asText(trackObj.ext_hint);
}

function normalizeLimit(value: number, fallback: number): number {
  const safe = Number(value);
  if (!Number.isFinite(safe)) {
    return fallback;
  }
  return Math.max(1, Math.min(200, Math.floor(safe)));
}

function normalizeOffset(value: number): number {
  const safe = Number(value);
  if (!Number.isFinite(safe)) {
    return 0;
  }
  return Math.max(0, Math.floor(safe));
}

function parseUnsignedInt(text: string): number | null {
  const parsed = Number(text);
  if (!Number.isFinite(parsed) || parsed < 0) {
    return null;
  }
  return Math.floor(parsed);
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

function asNumber(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === "string") {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) {
      return parsed;
    }
  }
  return null;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
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
      <h2>内容发现</h2>
      <div class="actions">
        <button class="primary" :disabled="isBusy || !hasGatewayContext" @click="runSearch()">搜索歌曲</button>
        <button :disabled="isBusy || !hasGatewayContext" @click="loadPlaylists()">加载歌单</button>
      </div>
    </div>

    <div class="discovery-grid">
      <article class="discovery-card">
        <div class="panel-head compact">
          <h3>歌曲搜索</h3>
        </div>
        <div class="grid">
          <label class="full-width">
            关键词
            <input v-model="searchKeywords" type="text" placeholder="例如：陈奕迅 / 青花瓷 / 夜空中最亮的星" />
          </label>
          <label>
            每页条数
            <input v-model.number="searchLimit" type="number" min="1" max="200" step="1" />
          </label>
          <label>
            偏移量
            <input v-model.number="searchOffset" type="number" min="0" step="1" />
          </label>
          <label>
            音质等级
            <UiSelect v-model="searchLevel" :options="levelOptions" />
          </label>
        </div>
        <p class="hint">{{ searchStatus }}</p>
        <ul class="discovery-list" v-if="searchItems.length > 0">
          <li v-for="item in searchItems" :key="`search-${item.itemId}-${item.title}`">
            <div class="discovery-row-main">
              <strong>{{ item.title }}</strong>
              <span class="discovery-meta" v-if="item.artist || item.album">
                {{ item.artist || "未知歌手" }}<template v-if="item.album"> · {{ item.album }}</template>
              </span>
            </div>
            <div class="actions">
              <button :disabled="isBusy || !hasGatewayContext || !item.trackId" @click="playNow(item)">播放</button>
              <button :disabled="isBusy || !hasGatewayContext || !item.trackId" @click="enqueue(item)">加入下一首</button>
            </div>
          </li>
        </ul>
      </article>

      <article class="discovery-card">
        <div class="panel-head compact">
          <h3>歌单浏览</h3>
        </div>
        <div class="grid">
          <label>
            每页条数
            <input v-model.number="playlistLimit" type="number" min="1" max="200" step="1" />
          </label>
          <label>
            偏移量
            <input v-model.number="playlistOffset" type="number" min="0" step="1" />
          </label>
        </div>
        <p class="hint">{{ playlistStatus }}</p>
        <ul class="discovery-list compact" v-if="playlistItems.length > 0">
          <li
            v-for="item in playlistItems"
            :key="`playlist-${item.itemId}`"
            :class="{ selected: selectedPlaylistId === item.itemId }"
          >
            <div class="discovery-row-main">
              <strong>{{ item.title }}</strong>
              <span class="discovery-meta">{{ item.trackCountText || "曲目数未知" }}</span>
            </div>
            <div class="actions">
              <button :disabled="isBusy || !hasGatewayContext" @click="loadPlaylistTracks(item)">查看歌曲</button>
            </div>
          </li>
        </ul>

        <div class="result-card">
          <div class="panel-head compact">
            <h3>歌单歌曲 {{ selectedPlaylistTitle ? `· ${selectedPlaylistTitle}` : "" }}</h3>
          </div>
          <p class="hint">{{ playlistTrackStatus }}</p>
          <ul class="discovery-list" v-if="playlistTracks.length > 0">
            <li v-for="item in playlistTracks" :key="`playlist-track-${item.itemId}-${item.title}`">
              <div class="discovery-row-main">
                <strong>{{ item.title }}</strong>
                <span class="discovery-meta" v-if="item.artist || item.album">
                  {{ item.artist || "未知歌手" }}<template v-if="item.album"> · {{ item.album }}</template>
                </span>
              </div>
              <div class="actions">
                <button :disabled="isBusy || !hasGatewayContext || !item.trackId" @click="playNow(item)">播放</button>
                <button :disabled="isBusy || !hasGatewayContext || !item.trackId" @click="enqueue(item)">加入下一首</button>
              </div>
            </li>
          </ul>
        </div>
      </article>
    </div>
  </section>
</template>
