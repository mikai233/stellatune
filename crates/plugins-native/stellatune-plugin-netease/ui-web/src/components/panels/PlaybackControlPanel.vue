<script setup lang="ts">
import { ref } from "vue";
import UiSelect from "../UiSelect.vue";
import type { SummaryField } from "../../view-models";

defineOptions({
  name: "PlaybackControlPanel"
});

const props = defineProps<{
  isBusy: boolean;
  hasGatewayContext: boolean;
  playbackStatus: string;
  playbackResultSummary: SummaryField[];
  playbackRawPayload: string;
  onRunAction: (action: string, payload?: Record<string, unknown>) => unknown;
  onCopyPlaybackRaw: () => unknown;
}>();

type PlaybackInputMode = "track_ref" | "track_token";

const inputMode = ref<PlaybackInputMode>("track_ref");
const inputError = ref("");
const trackTokenInput = ref("");
const sourceIdInput = ref("netease");
const trackIdInput = ref("");
const locatorInput = ref("");

const inputModeOptions = [
  { value: "track_ref", label: "结构化输入（推荐）" },
  { value: "track_token", label: "Track Token 字符串" }
];

async function runTrackAction(action: string): Promise<void> {
  const payload = buildTrackPayload();
  if (typeof payload === "string") {
    inputError.value = payload;
    return;
  }
  inputError.value = "";
  await Promise.resolve(props.onRunAction(action, payload));
}

async function runSimpleAction(action: string): Promise<void> {
  inputError.value = "";
  await Promise.resolve(props.onRunAction(action, {}));
}

function buildTrackPayload(): Record<string, unknown> | string {
  if (inputMode.value === "track_token") {
    const token = trackTokenInput.value.trim();
    if (token.length === 0) {
      return "请先输入 Track Token。";
    }
    return {
      track_token: token
    };
  }

  const sourceId = sourceIdInput.value.trim();
  const trackId = trackIdInput.value.trim();
  const locator = locatorInput.value.trim();
  if (sourceId.length === 0 || trackId.length === 0 || locator.length === 0) {
    return "请完整填写 source_id、track_id、locator。";
  }
  return {
    track_ref: {
      source_id: sourceId,
      track_id: trackId,
      locator
    }
  };
}
</script>

<template>
  <section class="panel">
    <div class="panel-head">
      <h2>播放控制</h2>
      <div class="actions">
        <button class="primary" :disabled="isBusy || !hasGatewayContext" @click="runTrackAction('playback.play_track_ref')">
          立即播放
        </button>
        <button :disabled="isBusy || !hasGatewayContext" @click="runTrackAction('playback.enqueue_track_ref')">
          加入下一首
        </button>
        <button :disabled="isBusy || !hasGatewayContext" @click="runTrackAction('playback.next')">
          切换到该曲目
        </button>
        <button :disabled="isBusy || !hasGatewayContext" @click="runSimpleAction('playback.pause')">
          暂停
        </button>
        <button :disabled="isBusy || !hasGatewayContext" @click="runSimpleAction('playback.stop')">
          停止
        </button>
      </div>
    </div>

    <div class="grid">
      <label>
        曲目信息输入方式
        <UiSelect v-model="inputMode" :options="inputModeOptions" />
      </label>
    </div>

    <label class="full-width" v-if="inputMode === 'track_token'">
      Track Token
      <textarea
        v-model="trackTokenInput"
        rows="3"
        placeholder='例如：{"source_id":"netease","track_id":"12345","locator":"netease:track:12345"}'
      ></textarea>
    </label>
    <template v-else>
      <div class="grid">
        <label>
          source_id
          <input v-model="sourceIdInput" type="text" placeholder="netease" />
        </label>
        <label>
          track_id
          <input v-model="trackIdInput" type="text" placeholder="歌曲 ID" />
        </label>
      </div>
      <label class="full-width">
        locator
        <input v-model="locatorInput" type="text" placeholder="netease:track:12345 或 sidecar 返回的 locator" />
      </label>
    </template>

    <p class="hint">
      <code>playback.next</code> 当前是“切换到你输入的曲目”，不是自动跳到队列头。
    </p>
    <p class="hint warning" v-if="inputError">{{ inputError }}</p>
    <p class="hero-meta"><strong>{{ playbackStatus }}</strong></p>

    <div class="result-card">
      <div class="panel-head compact">
        <h3>最近一次播放动作结果</h3>
        <div class="actions">
          <button :disabled="!playbackRawPayload.trim()" @click="onCopyPlaybackRaw()">
            复制原始响应
          </button>
        </div>
      </div>
      <div class="result-grid" v-if="playbackResultSummary.length > 0">
        <div class="result-item" v-for="item in playbackResultSummary" :key="item.label">
          <span>{{ item.label }}</span>
          <strong>{{ item.value }}</strong>
        </div>
      </div>
      <p class="hint" v-else>尚未执行播放动作。</p>
      <details class="raw-panel" v-if="playbackRawPayload.trim()">
        <summary>查看原始响应（JSON）</summary>
        <pre class="payload">{{ playbackRawPayload }}</pre>
      </details>
    </div>
  </section>
</template>
