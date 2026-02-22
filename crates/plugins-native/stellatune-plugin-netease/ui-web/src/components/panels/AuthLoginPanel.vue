<script setup lang="ts">
import { computed, ref } from "vue";
import JsonTreeNode from "../JsonTreeNode.vue";
import UiSelect from "../UiSelect.vue";
import type { SummaryField } from "../../view-models";

interface JsonFlatRow {
  path: string;
  type: string;
  preview: string;
  searchable: string;
}

defineOptions({
  name: "AuthLoginPanel"
});

const props = defineProps<{
  isBusy: boolean;
  hasGatewayContext: boolean;
  qrPolling: boolean;
  qrPollingText: string;
  authStateText: string;
  authUser: string;
  authCode: number | null;
  authCookieLength: number | null;
  qrKey: string;
  qrImageUrl: string;
  qrTextUrl: string;
  qrStatus: string;
  authRawPayload: string;
  authRawValue: unknown;
  authResultSummary: SummaryField[];
  onStartQrLogin: () => unknown;
  onRefreshStatus: () => unknown;
  onStopWait: () => unknown;
  onLogout: () => unknown;
  onRefreshSession: () => unknown;
  onQrStartOnly: () => unknown;
  onQrStatusOnly: () => unknown;
  onCopyAuthRaw: () => unknown;
}>();

const authJsonKeyword = ref("");
const authJsonTypeFilter = ref("全部");
const authJsonViewMode = ref<"table" | "tree">("table");

const authJsonRows = computed(() => flattenJson(props.authRawValue, 280));
const authJsonTypeOptions = computed(() => {
  const typeSet = new Set<string>();
  for (const row of authJsonRows.value) {
    typeSet.add(row.type);
  }
  return ["全部", ...Array.from(typeSet.values())];
});
const filteredAuthJsonRows = computed(() => {
  const keyword = authJsonKeyword.value.trim().toLowerCase();
  return authJsonRows.value.filter((row) => {
    if (authJsonTypeFilter.value !== "全部" && row.type !== authJsonTypeFilter.value) {
      return false;
    }
    if (!keyword) {
      return true;
    }
    return row.searchable.includes(keyword);
  });
});
const authJsonStatsLabel = computed(
  () => `结构化字段 ${authJsonRows.value.length} 条，筛选后 ${filteredAuthJsonRows.value.length} 条`
);

function flattenJson(value: unknown, maxRows: number): JsonFlatRow[] {
  if (value === null || value === undefined) {
    return [];
  }
  const rows: JsonFlatRow[] = [];
  const stack: Array<{ path: string; value: unknown }> = [{ path: "$", value }];
  while (stack.length > 0 && rows.length < maxRows) {
    const current = stack.pop();
    if (!current) {
      break;
    }
    const type = detectJsonType(current.value);
    const preview = describeJsonValue(current.value);
    rows.push({
      path: current.path,
      type,
      preview,
      searchable: `${current.path} ${type} ${preview}`.toLowerCase()
    });

    if (Array.isArray(current.value)) {
      for (let index = current.value.length - 1; index >= 0; index -= 1) {
        stack.push({
          path: `${current.path}[${index}]`,
          value: current.value[index]
        });
      }
      continue;
    }

    const objectValue = asRecord(current.value);
    if (!objectValue) {
      continue;
    }
    const entries = Object.entries(objectValue);
    for (let index = entries.length - 1; index >= 0; index -= 1) {
      const [key, child] = entries[index];
      stack.push({
        path: `${current.path}${formatJsonPathSuffix(key)}`,
        value: child
      });
    }
  }

  if (stack.length > 0) {
    rows.push({
      path: "$",
      type: "提示",
      preview: `字段过多，已截断为前 ${maxRows} 条`,
      searchable: "提示 截断"
    });
  }
  return rows;
}

function formatJsonPathSuffix(key: string): string {
  if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) {
    return `.${key}`;
  }
  return `[${JSON.stringify(key)}]`;
}

function detectJsonType(value: unknown): string {
  if (value === null) {
    return "空值";
  }
  if (Array.isArray(value)) {
    return "数组";
  }
  if (typeof value === "string") {
    return "字符串";
  }
  if (typeof value === "number") {
    return "数字";
  }
  if (typeof value === "boolean") {
    return "布尔";
  }
  if (typeof value === "object") {
    return "对象";
  }
  return typeof value;
}

function describeJsonValue(value: unknown): string {
  if (value === null) {
    return "null";
  }
  if (typeof value === "string") {
    return truncateText(value.replace(/\s+/g, " "), 140);
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (Array.isArray(value)) {
    return `数组（${value.length} 项）`;
  }
  const record = asRecord(value);
  if (record) {
    const keys = Object.keys(record);
    return `对象（${keys.length} 字段）`;
  }
  return truncateText(String(value), 140);
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function truncateText(text: string, maxLength: number): string {
  if (text.length <= maxLength) {
    return text;
  }
  return `${text.slice(0, maxLength)}...（已截断）`;
}
</script>

<template>
  <section class="panel">
    <div class="panel-head">
      <h2>登录</h2>
      <div class="actions">
        <button class="primary" :disabled="isBusy || !hasGatewayContext" @click="onStartQrLogin()">
          扫码登录（自动）
        </button>
        <button :disabled="isBusy || !hasGatewayContext" @click="onRefreshStatus()">刷新状态</button>
        <button v-if="qrPolling" :disabled="isBusy || !hasGatewayContext" @click="onStopWait()">
          停止等待
        </button>
        <button :disabled="isBusy || !hasGatewayContext" @click="onLogout()">退出登录</button>
      </div>
    </div>
    <ol class="auth-steps">
      <li>点击“扫码登录（自动）”。</li>
      <li>用网易云音乐 App 扫码并在手机确认。</li>
      <li>页面会自动刷新状态，成功后显示“已登录”。</li>
    </ol>
    <p class="hint" v-if="qrPolling">{{ qrPollingText }}</p>
    <div class="auth-grid">
      <div class="auth-item">
        <span>认证状态</span>
        <strong>{{ authStateText }}</strong>
      </div>
      <div class="auth-item">
        <span>用户</span>
        <strong>{{ authUser }}</strong>
      </div>
      <div class="auth-item">
        <span>状态码</span>
        <strong>{{ authCode ?? "（无）" }}</strong>
      </div>
      <div class="auth-item">
        <span>登录凭证长度</span>
        <strong>{{ authCookieLength ?? "（未知）" }}</strong>
      </div>
    </div>
    <p class="hero-meta" v-if="qrKey">登录二维码 key：<strong>{{ qrKey }}</strong></p>
    <div class="qr-preview" v-if="qrImageUrl || qrTextUrl">
      <img v-if="qrImageUrl" :src="qrImageUrl" alt="网易云二维码" class="qr-image" />
      <p class="hint" v-if="qrTextUrl">扫码链接：{{ qrTextUrl }}</p>
    </div>
    <p class="hero-meta"><strong>{{ qrStatus }}</strong></p>
    <details class="raw-panel auth-advanced">
      <summary>高级操作（调试用）</summary>
      <div class="actions">
        <button :disabled="isBusy || !hasGatewayContext" @click="onRefreshSession()">刷新登录会话</button>
        <button :disabled="isBusy || !hasGatewayContext" @click="onQrStartOnly()">仅获取二维码</button>
        <button :disabled="isBusy || !hasGatewayContext" @click="onQrStatusOnly()">仅查扫码状态</button>
      </div>
    </details>
    <div class="result-card">
      <div class="panel-head compact">
        <h3>最近一次认证结果</h3>
        <div class="actions">
          <button :disabled="!authRawPayload.trim()" @click="onCopyAuthRaw()">复制原始响应</button>
        </div>
      </div>
      <div class="result-grid" v-if="authResultSummary.length > 0">
        <div class="result-item" v-for="item in authResultSummary" :key="item.label">
          <span>{{ item.label }}</span>
          <strong>{{ item.value }}</strong>
        </div>
      </div>
      <p class="hint" v-else>尚未执行认证动作。</p>
      <div class="json-inspector" v-if="authRawPayload.trim()">
        <div class="json-view-switch">
          <button :class="{ active: authJsonViewMode === 'table' }" @click="authJsonViewMode = 'table'">
            路径表
          </button>
          <button :class="{ active: authJsonViewMode === 'tree' }" @click="authJsonViewMode = 'tree'">
            树形视图
          </button>
        </div>

        <template v-if="authJsonViewMode === 'table'">
          <div class="json-toolbar">
            <input
              v-model="authJsonKeyword"
              type="text"
              placeholder="筛选 JSON 路径或值（例如 profile、cookie、803）"
            />
            <UiSelect v-model="authJsonTypeFilter" :options="authJsonTypeOptions" />
          </div>
          <p class="hint">{{ authJsonStatsLabel }}</p>
          <div class="json-table-wrap">
            <table class="json-table">
              <thead>
                <tr>
                  <th>路径</th>
                  <th>类型</th>
                  <th>值摘要</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="row in filteredAuthJsonRows" :key="row.path">
                  <td class="json-path">{{ row.path }}</td>
                  <td class="json-type">{{ row.type }}</td>
                  <td class="json-preview">{{ row.preview }}</td>
                </tr>
                <tr v-if="filteredAuthJsonRows.length === 0">
                  <td class="json-empty" colspan="3">当前筛选条件下无匹配字段。</td>
                </tr>
              </tbody>
            </table>
          </div>
        </template>
        <template v-else>
          <p class="hint">点击每一行左侧三角可展开/收起，悬停会高亮可点击行。</p>
          <div class="json-tree-wrap">
            <ul class="json-tree-root">
              <JsonTreeNode node-key="$" path="$" :value="authRawValue" :depth="0" />
            </ul>
          </div>
        </template>
      </div>
      <details class="raw-panel" v-if="authRawPayload.trim()">
        <summary>查看原始响应（JSON）</summary>
        <pre class="payload">{{ authRawPayload }}</pre>
      </details>
    </div>
  </section>
</template>
