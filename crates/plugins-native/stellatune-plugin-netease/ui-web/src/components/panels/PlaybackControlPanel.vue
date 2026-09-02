<script setup lang="ts">
import { ref } from "vue";
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

const inputError = ref("");
const trackIdInput = ref("");

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
  const trackId = trackIdInput.value.trim();
  if (!/^[1-9][0-9]*$/.test(trackId)) {
    return "请输入有效的稳定 TrackId。";
  }
  return { track_id: trackId };
}
</script>

<template>
  <section class="panel">
    <div class="panel-head">
      <h2>播放控制</h2>
      <div class="actions">
        <button class="primary" :disabled="isBusy || !hasGatewayContext" @click="runTrackAction('playback.play_track')">
          立即播放
        </button>
        <button :disabled="isBusy || !hasGatewayContext" @click="runTrackAction('playback.enqueue_track')">
          加入下一首
        </button>
        <button :disabled="isBusy || !hasGatewayContext" @click="runSimpleAction('playback.pause')">
          暂停
        </button>
        <button :disabled="isBusy || !hasGatewayContext" @click="runSimpleAction('playback.stop')">
          停止
        </button>
      </div>
    </div>

    <label class="full-width">
      稳定 TrackId
      <input v-model="trackIdInput" type="text" inputmode="numeric" placeholder="由播放器曲目目录分配的 TrackId" />
    </label>

    <p class="hint">
      搜索结果会先用 provider identity 注册曲目；这里仅接受已经注册的稳定 TrackId。
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
