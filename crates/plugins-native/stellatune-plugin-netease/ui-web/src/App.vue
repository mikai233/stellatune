<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import {
  createNeteaseConfigRoot,
  extractNeteaseConfig,
  getPluginConfig,
  invokePluginAction,
  putPluginConfig,
  resolveGatewayContext,
  subscribePluginEvents
} from "./api";
import GatewayHeroCard from "./components/panels/GatewayHeroCard.vue";
import SourceConfigPanel from "./components/panels/SourceConfigPanel.vue";
import AuthLoginPanel from "./components/panels/AuthLoginPanel.vue";
import EventStreamPanel from "./components/panels/EventStreamPanel.vue";
import { DEFAULT_SOURCE_CONFIG } from "./types";
import type { ConfigFormState, SummaryField, UiEventRow } from "./view-models";
import { buildApplySummary, buildSourceConfig, hydrateForm } from "./logic/config-form";
import {
  asNonEmptyString,
  asNumber,
  asRecord,
  buildActionSummary,
  buildAuthActionEventSummary,
  extractActionPayload,
  extractQrImage,
  extractQrKey,
  extractQrTextUrl,
  formatActionName,
  formatAuthState,
  formatEventName,
  prettyJson,
  summarizePayload,
  summarizeStreamEvent
} from "./logic/auth-display";

interface AuthActionRunOptions {
  silentMessage?: boolean;
  eventNameOverride?: string;
  background?: boolean;
  skipEvent?: boolean;
}

const gateway = resolveGatewayContext();
const isBusy = ref(false);
const message = ref("已就绪");
const lastApplySummary = ref("");
const qrStatus = ref("未开始");
const qrKey = ref("");
const qrImageUrl = ref("");
const qrTextUrl = ref("");
const qrPolling = ref(false);
const qrPollingAttempt = ref(0);
const authState = ref("unknown");
const authUser = ref("（未知）");
const authCode = ref<number | null>(null);
const authCookieLength = ref<number | null>(null);
const authResultSummary = ref<SummaryField[]>([]);
const authRawPayload = ref("");
const authRawValue = ref<unknown>(null);
const events = ref<UiEventRow[]>([]);
let closeEvents: (() => void) | null = null;

const form = reactive<ConfigFormState>({
  sidecarBaseUrl: DEFAULT_SOURCE_CONFIG.sidecar_base_url,
  sidecarPath: DEFAULT_SOURCE_CONFIG.sidecar_path ?? "",
  sidecarArgsText: "",
  apiRequestTimeoutMs: DEFAULT_SOURCE_CONFIG.api_request_timeout_ms,
  streamReadTimeoutMs: "",
  defaultLevel: DEFAULT_SOURCE_CONFIG.default_level
});

const hasToken = computed(() => gateway.token.length > 0);
const hasGatewayContext = computed(() => hasToken.value);
const authStateText = computed(() => formatAuthState(authState.value));
const qrPollingText = computed(() => (qrPolling.value ? `自动查询中（第 ${qrPollingAttempt.value} 次）` : ""));

onMounted(() => {
  if (hasGatewayContext.value) {
    void bootstrapPage();
  } else {
    message.value = "缺少网关会话令牌，请通过宿主网关地址打开此页面。";
    pushEvent("环境诊断", "未检测到 token，API 请求将不可用。", {
      source: "系统"
    });
  }
  closeEvents = subscribePluginEvents(
    gateway,
    (event) => {
      pushEvent(formatEventName(event.name), event.payload, {
        source: "事件流",
        summary: summarizeStreamEvent(event.name, event.payload)
      });
    },
    () => {
      pushEvent("事件流错误", "事件推送连接已断开", {
        source: "系统",
        summary: "SSE 连接断开，通常会由浏览器自动重连。"
      });
    }
  );
});

onBeforeUnmount(() => {
  stopQrPolling();
  closeEvents?.();
});

async function bootstrapPage(): Promise<void> {
  await loadConfig();
  await runAuthAction("netease.auth.login_status", {
    silentMessage: true,
    eventNameOverride: "自动刷新登录状态"
  });
}

async function startQrLoginFlow(): Promise<void> {
  if (!ensureGatewayContext("扫码登录")) {
    return;
  }
  stopQrPolling();
  qrStatus.value = "正在获取登录二维码...";
  await runAuthAction("netease.auth.qr.start", {
    silentMessage: true,
    eventNameOverride: "获取登录二维码"
  });

  if (!qrImageUrl.value && !qrTextUrl.value) {
    qrStatus.value = "未拿到可用二维码，请重试";
    message.value = "获取二维码失败，请重试";
    return;
  }

  qrPolling.value = true;
  qrPollingAttempt.value = 0;
  qrStatus.value = "请使用网易云音乐 App 扫码，正在自动查询状态...";
  message.value = "请扫码并在手机上确认登录";

  for (let index = 0; index < 90 && qrPolling.value; index += 1) {
    qrPollingAttempt.value = index + 1;
    await sleep(index === 0 ? 1200 : 1800);
    if (!qrPolling.value) {
      break;
    }
    await runAuthAction("netease.auth.qr.status", {
      silentMessage: true,
      background: true,
      skipEvent: true
    });

    if (authState.value === "logged_in") {
      qrPolling.value = false;
      qrStatus.value = "扫码登录成功";
      message.value = "已完成登录";
      pushEvent("扫码登录成功", { attempts: qrPollingAttempt.value }, {
        source: "认证动作",
        summary: `已自动查询 ${qrPollingAttempt.value} 次后登录成功`
      });
      return;
    }
    if (authState.value === "qr_expired") {
      qrPolling.value = false;
      qrStatus.value = "二维码已过期，请重新扫码";
      message.value = "二维码已过期，请重新点击“扫码登录”";
      pushEvent("扫码登录失败", { reason: "qr_expired" }, {
        source: "认证动作",
        summary: "二维码已过期"
      });
      return;
    }
  }

  if (qrPolling.value) {
    qrPolling.value = false;
    qrStatus.value = "自动查询已结束，可点击“刷新状态”继续确认";
    message.value = "自动查询已结束，请根据需要继续刷新状态";
  }
}

function stopQrPolling(): void {
  if (!qrPolling.value) {
    qrPollingAttempt.value = 0;
    return;
  }
  qrPolling.value = false;
  qrPollingAttempt.value = 0;
  qrStatus.value = "已停止自动查询";
}

async function loadConfig(): Promise<void> {
  if (!ensureGatewayContext("加载配置")) {
    return;
  }
  isBusy.value = true;
  try {
    const response = await getPluginConfig(gateway);
    const sourceConfig = extractNeteaseConfig(response.config);
    hydrateForm(form, sourceConfig);
    message.value = "已从宿主加载配置";
    pushEvent("配置加载", response.config, {
      source: "配置",
      summary: "插件配置加载完成"
    });
  } catch (error) {
    message.value = `加载失败：${formatError(error)}`;
    pushEvent("配置加载失败", { error: formatError(error) }, { source: "配置" });
  } finally {
    isBusy.value = false;
  }
}

async function saveConfig(): Promise<void> {
  await submitConfig(true);
}

async function applyConfigWithoutPersist(): Promise<void> {
  await submitConfig(false);
}

async function submitConfig(persist: boolean): Promise<void> {
  if (!ensureGatewayContext("保存配置")) {
    return;
  }
  isBusy.value = true;
  try {
    const sourceConfig = buildSourceConfig(form);
    if (persist) {
      const updated = await putPluginConfig(gateway, createNeteaseConfigRoot(sourceConfig));
      lastApplySummary.value = buildApplySummary(updated.apply_report);
      message.value = "配置已保存并应用";
      pushEvent("配置保存", updated.apply_report ?? updated.config, {
        source: "配置",
        summary: lastApplySummary.value || "已持久化配置并应用到运行时"
      });
    } else {
      const updated = await invokePluginAction(gateway, "config.apply", {
        persist: false,
        config: createNeteaseConfigRoot(sourceConfig)
      });
      message.value = updated.message;
      pushEvent("配置临时应用", updated.data, {
        source: "配置",
        summary: updated.message
      });
    }
  } catch (error) {
    message.value = `保存失败：${formatError(error)}`;
    pushEvent("配置保存失败", { error: formatError(error) }, { source: "配置" });
  } finally {
    isBusy.value = false;
  }
}

async function runAuthAction(action: string, options?: AuthActionRunOptions): Promise<void> {
  if (!ensureGatewayContext("执行认证操作")) {
    return;
  }
  const shouldSetBusy = !options?.background;
  if (shouldSetBusy) {
    isBusy.value = true;
  }
  try {
    const requestPayload: Record<string, unknown> = {};
    if (action === "netease.auth.qr.status" && qrKey.value.trim().length > 0) {
      requestPayload.request = { key: qrKey.value.trim() };
    }
    const response = await invokePluginAction(gateway, action, requestPayload);
    const actionPayload = extractActionPayload(response.data);
    applyAuthSnapshot(action, actionPayload);
    const summaryFields = buildActionSummary(
      action,
      response.message,
      actionPayload,
      qrKey.value,
      authCode.value
    );
    authResultSummary.value = summaryFields;
    authRawValue.value = response.data;
    authRawPayload.value = prettyJson(response.data);
    qrStatus.value = `${formatActionName(action)}：${response.message}`;
    if (!options?.silentMessage) {
      message.value = `操作已完成：${formatActionName(action)}`;
    }
    if (!options?.skipEvent) {
      pushEvent(options?.eventNameOverride ?? formatActionName(action), response.data, {
        source: "认证动作",
        summary: buildAuthActionEventSummary(response.message, summaryFields)
      });
    }
  } catch (error) {
    qrStatus.value = `${formatActionName(action)}：失败`;
    authResultSummary.value = [
      { label: "动作", value: formatActionName(action) },
      { label: "错误信息", value: formatError(error) }
    ];
    authRawValue.value = { error: formatError(error) };
    authRawPayload.value = formatError(error);
    if (!options?.silentMessage) {
      message.value = `操作失败：${formatActionName(action)}`;
    }
    if (!options?.skipEvent) {
      const fallbackName = options?.eventNameOverride ?? formatActionName(action);
      pushEvent(`${fallbackName}（失败）`, { error: formatError(error) }, {
        source: "认证动作",
        summary: formatError(error)
      });
    }
  } finally {
    if (shouldSetBusy) {
      isBusy.value = false;
    }
  }
}

async function refreshLoginStatus(): Promise<void> {
  await runAuthAction("netease.auth.login_status");
}

async function refreshLoginSession(): Promise<void> {
  await runAuthAction("netease.auth.login_refresh");
}

async function qrStartOnly(): Promise<void> {
  await runAuthAction("netease.auth.qr.start");
}

async function qrStatusOnly(): Promise<void> {
  await runAuthAction("netease.auth.qr.status");
}

async function logout(): Promise<void> {
  stopQrPolling();
  await runAuthAction("netease.auth.logout");
}

function applyAuthSnapshot(action: string, payload: unknown): void {
  const payloadObj = asRecord(payload);
  const bodyObj = asRecord(payloadObj?.body);
  const bodyDataObj = asRecord(bodyObj?.data);
  const code = asNumber(bodyObj?.code);
  if (code !== null) {
    authCode.value = code;
  }

  const cookieText = typeof payloadObj?.cookie === "string" ? payloadObj.cookie.trim() : "";
  if (cookieText.length > 0) {
    authCookieLength.value = cookieText.length;
  } else if (action === "netease.auth.logout") {
    authCookieLength.value = 0;
  }

  const profile = asRecord(bodyDataObj?.profile);
  const account = asRecord(bodyDataObj?.account);
  const nickname = asNonEmptyString(profile?.nickname);
  const uid =
    asNumber(account?.id) ??
    asNumber(profile?.userId) ??
    asNumber(bodyDataObj?.accountId) ??
    null;
  if (nickname) {
    authUser.value = uid ? `${nickname}（用户ID=${uid}）` : nickname;
  } else if (uid) {
    authUser.value = `用户ID=${uid}`;
  }

  if (action === "netease.auth.qr.start") {
    const nextKey = extractQrKey(payloadObj);
    if (nextKey) {
      qrKey.value = nextKey;
    }
    qrImageUrl.value = extractQrImage(payloadObj) ?? "";
    qrTextUrl.value = extractQrTextUrl(payloadObj) ?? "";
    authState.value = "qr_wait_scan";
  } else if (action === "netease.auth.qr.status") {
    const qrCode = code ?? -1;
    if (qrCode === 803) {
      authState.value = "logged_in";
    } else if (qrCode === 802) {
      authState.value = "qr_wait_confirm";
    } else if (qrCode === 801) {
      authState.value = "qr_wait_scan";
    } else if (qrCode === 800) {
      authState.value = "qr_expired";
    }
  } else if (action === "netease.auth.logout") {
    authState.value = "logged_out";
    authUser.value = "（未知）";
    qrImageUrl.value = "";
    qrTextUrl.value = "";
  } else if (action === "netease.auth.login_status" || action === "netease.auth.login_refresh") {
    authState.value = uid ? "logged_in" : "logged_out";
  }
}

function pushEvent(
  name: string,
  payload: unknown,
  options?: {
    source?: string;
    summary?: string;
  }
): void {
  const summary = options?.summary?.trim() || summarizePayload(payload);
  const raw = prettyJson(payload);
  const row: UiEventRow = {
    id: Date.now() + Math.floor(Math.random() * 1000),
    time: new Date().toLocaleTimeString(),
    name,
    source: options?.source ?? "系统",
    summary,
    raw,
    searchable: `${name} ${summary} ${raw}`.toLowerCase()
  };
  events.value = [row, ...events.value].slice(0, 120);
}

async function copyAuthRaw(): Promise<void> {
  if (authRawPayload.value.trim().length === 0) {
    return;
  }
  if (!navigator.clipboard?.writeText) {
    message.value = "当前浏览器不支持剪贴板写入";
    return;
  }
  try {
    await navigator.clipboard.writeText(authRawPayload.value);
    message.value = "已复制最近一次认证原始响应";
  } catch (error) {
    message.value = `复制失败：${formatError(error)}`;
  }
}

function clearEvents(): void {
  events.value = [];
}

function ensureGatewayContext(operation: string): boolean {
  if (hasGatewayContext.value) {
    return true;
  }
  message.value = `无法${operation}：缺少网关会话令牌，请通过宿主网关页面访问。`;
  return false;
}

function formatError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}
</script>

<template>
  <div class="page-shell">
    <main class="board">
      <GatewayHeroCard
        :plugin-id="gateway.pluginId"
        :origin="gateway.origin"
        :has-token="hasToken"
        :has-gateway-context="hasGatewayContext"
        :message="message"
      />

      <SourceConfigPanel
        :form="form"
        :is-busy="isBusy"
        :has-gateway-context="hasGatewayContext"
        :last-apply-summary="lastApplySummary"
        :on-reload="loadConfig"
        :on-apply-temp="applyConfigWithoutPersist"
        :on-save="saveConfig"
      />

      <AuthLoginPanel
        :is-busy="isBusy"
        :has-gateway-context="hasGatewayContext"
        :qr-polling="qrPolling"
        :qr-polling-text="qrPollingText"
        :auth-state-text="authStateText"
        :auth-user="authUser"
        :auth-code="authCode"
        :auth-cookie-length="authCookieLength"
        :qr-key="qrKey"
        :qr-image-url="qrImageUrl"
        :qr-text-url="qrTextUrl"
        :qr-status="qrStatus"
        :auth-result-summary="authResultSummary"
        :auth-raw-payload="authRawPayload"
        :auth-raw-value="authRawValue"
        :on-start-qr-login="startQrLoginFlow"
        :on-refresh-status="refreshLoginStatus"
        :on-stop-wait="stopQrPolling"
        :on-logout="logout"
        :on-refresh-session="refreshLoginSession"
        :on-qr-start-only="qrStartOnly"
        :on-qr-status-only="qrStatusOnly"
        :on-copy-auth-raw="copyAuthRaw"
      />

      <EventStreamPanel :events="events" :on-clear-events="clearEvents" />
    </main>
  </div>
</template>
