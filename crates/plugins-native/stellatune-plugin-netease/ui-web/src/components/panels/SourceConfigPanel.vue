<script setup lang="ts">
import UiSelect from "../UiSelect.vue";
import type { ConfigFormState } from "../../view-models";

defineOptions({
  name: "SourceConfigPanel"
});

defineProps<{
  form: ConfigFormState;
  isBusy: boolean;
  hasGatewayContext: boolean;
  lastApplySummary: string;
  onReload: () => unknown;
  onApplyTemp: () => unknown;
  onSave: () => unknown;
}>();

const qualityOptions = [
  { value: "standard", label: "标准（standard）" },
  { value: "higher", label: "较高（higher）" },
  { value: "exhigh", label: "极高（exhigh）" },
  { value: "lossless", label: "无损（lossless）" }
];
</script>

<template>
  <section class="panel">
    <div class="panel-head">
      <h2>音源配置</h2>
      <div class="actions">
        <button :disabled="isBusy || !hasGatewayContext" @click="onReload()">重新加载</button>
        <button :disabled="isBusy || !hasGatewayContext" @click="onApplyTemp()">临时应用</button>
        <button class="primary" :disabled="isBusy || !hasGatewayContext" @click="onSave()">保存并应用</button>
      </div>
    </div>

    <div class="grid">
      <label>
        Sidecar 服务地址
        <input v-model="form.sidecarBaseUrl" type="text" placeholder="http://127.0.0.1:46321" />
      </label>
      <label>
        Sidecar 路径（可选）
        <input v-model="form.sidecarPath" type="text" placeholder="bin/stellatune-ncm-sidecar.exe" />
      </label>
      <label>
        API 超时（毫秒）
        <input v-model.number="form.apiRequestTimeoutMs" type="number" min="500" step="100" />
      </label>
      <label>
        流读取超时（毫秒，可选）
        <input v-model="form.streamReadTimeoutMs" type="number" min="500" step="100" placeholder="留空表示 null" />
      </label>
      <label>
        默认音质等级
        <UiSelect v-model="form.defaultLevel" :options="qualityOptions" />
      </label>
    </div>

    <label class="full-width">
      Sidecar 参数（每行一项）
      <textarea v-model="form.sidecarArgsText" rows="4" placeholder="--port=46321"></textarea>
    </label>

    <p class="hint" v-if="lastApplySummary">{{ lastApplySummary }}</p>
  </section>
</template>
