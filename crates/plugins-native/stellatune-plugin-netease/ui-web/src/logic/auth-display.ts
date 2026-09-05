import type { SummaryField } from "../view-models";

export function extractActionPayload(data: unknown): unknown | null {
  const outer = asRecord(data);
  const rows = Array.isArray(outer?.response) ? outer.response : [];
  const first = rows.length > 0 ? asRecord(rows[0]) : null;
  return first?.playlist_ref ?? null;
}

export function buildActionSummary(
  action: string,
  responseMessage: string,
  payload: unknown,
  qrKey: string,
  authCode: number | null
): SummaryField[] {
  const rows: SummaryField[] = [
    { label: "动作", value: formatActionName(action) },
    { label: "插件响应", value: responseMessage }
  ];
  const payloadObj = asRecord(payload);
  const bodyObj = asRecord(payloadObj?.body);
  const bodyDataObj = asRecord(bodyObj?.data);
  const code = asNumber(bodyObj?.code);
  if (code !== null) {
    rows.push({ label: "返回码", value: String(code) });
  }
  const apiMessage = asNonEmptyString(bodyObj?.message);
  if (apiMessage) {
    rows.push({ label: "接口消息", value: apiMessage });
  }
  const cookieText = asNonEmptyString(payloadObj?.cookie);
  if (cookieText) {
    rows.push({ label: "Cookie（脱敏）", value: maskCookie(cookieText) });
  }
  const profile = asRecord(bodyDataObj?.profile);
  const account = asRecord(bodyDataObj?.account);
  const nickname = asNonEmptyString(profile?.nickname);
  const uid =
    asNumber(account?.id) ??
    asNumber(profile?.userId) ??
    asNumber(bodyDataObj?.accountId) ??
    null;
  if (nickname || uid !== null) {
    rows.push({
      label: "账号",
      value: nickname && uid !== null ? `${nickname}（ID=${uid}）` : nickname ?? `ID=${uid}`
    });
  }
  if (action === "netease.auth.qr.start" && qrKey.trim().length > 0) {
    rows.push({ label: "二维码 Key", value: qrKey.trim() });
  }
  if (action === "netease.auth.qr.status" && authCode !== null) {
    rows.push({ label: "二维码状态", value: formatAuthCode(authCode) });
  }
  if (rows.length > 8) {
    return rows.slice(0, 8);
  }
  return rows;
}

export function buildAuthActionEventSummary(responseMessage: string, fields: SummaryField[]): string {
  const important = fields
    .filter((item) => item.label === "返回码" || item.label === "账号" || item.label === "二维码状态")
    .map((item) => `${item.label}=${item.value}`);
  if (important.length === 0) {
    return responseMessage;
  }
  return `${responseMessage}；${important.join("；")}`;
}

export function extractQrKey(payload: Record<string, unknown> | null): string | null {
  const direct = asNonEmptyString(payload?.key);
  if (direct) {
    return direct;
  }
  const keyPayload = asRecord(payload?.key_payload);
  const keyBody = asRecord(keyPayload?.body);
  const keyData = asRecord(keyBody?.data);
  return asNonEmptyString(keyData?.unikey) ?? asNonEmptyString(keyData?.key) ?? null;
}

export function extractQrImage(payload: Record<string, unknown> | null): string | null {
  const createPayload = asRecord(payload?.create_payload);
  const body = asRecord(createPayload?.body);
  const data = asRecord(body?.data);
  return asNonEmptyString(data?.qrimg) ?? null;
}

export function extractQrTextUrl(payload: Record<string, unknown> | null): string | null {
  const createPayload = asRecord(payload?.create_payload);
  const body = asRecord(createPayload?.body);
  const data = asRecord(body?.data);
  return asNonEmptyString(data?.qrurl) ?? null;
}

export function summarizeStreamEvent(name: string, payload: unknown): string {
  if (name === "lagged") {
    return "事件流积压，宿主会自动丢弃部分历史事件。";
  }
  return summarizePayload(payload);
}

export function summarizePayload(payload: unknown): string {
  if (payload === null || payload === undefined) {
    return "无数据";
  }
  if (typeof payload === "string") {
    return truncateText(payload, 140);
  }
  if (typeof payload === "number" || typeof payload === "boolean") {
    return String(payload);
  }
  if (Array.isArray(payload)) {
    return `数组数据（${payload.length} 项）`;
  }
  const record = asRecord(payload);
  if (record) {
    const messageText = asNonEmptyString(record.message);
    if (messageText) {
      return truncateText(messageText, 140);
    }
    const errorText = asNonEmptyString(record.error);
    if (errorText) {
      return truncateText(errorText, 140);
    }
    const keys = Object.keys(record);
    if (keys.length === 0) {
      return "对象数据（空）";
    }
    return `对象数据（字段：${keys.slice(0, 6).join("、")}${keys.length > 6 ? "..." : ""}）`;
  }
  return truncateText(String(payload), 140);
}

export function prettyJson(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

export function truncateText(text: string, maxLength: number): string {
  if (text.length <= maxLength) {
    return text;
  }
  return `${text.slice(0, maxLength)}...（已截断）`;
}

export function maskCookie(rawCookie: string): string {
  const compact = rawCookie.replace(/\s+/g, " ");
  const visiblePrefix = compact.slice(0, 20);
  const visibleSuffix = compact.slice(-12);
  return `${visiblePrefix}...${visibleSuffix}（总长度 ${compact.length}）`;
}

export function formatActionName(action: string): string {
  switch (action) {
    case "netease.auth.login_status":
      return "刷新登录状态";
    case "netease.auth.login_refresh":
      return "刷新登录会话";
    case "netease.auth.qr.start":
      return "获取登录二维码";
    case "netease.auth.qr.status":
      return "查询扫码状态";
    case "netease.auth.logout":
      return "退出登录";
    case "search":
      return "搜索歌曲";
    case "list_playlists":
      return "加载歌单";
    case "playlist_tracks":
      return "加载歌单歌曲";
    case "netease.auth.session":
      return "查询认证会话";
    case "netease.song.lyric":
      return "获取歌曲歌词";
    case "netease.song.url":
      return "解析歌曲 URL";
    case "playback.play_track":
    case "playback.play_provider_track":
      return "立即播放指定曲目";
    case "playback.enqueue_track":
    case "playback.enqueue_provider_track":
      return "加入下一首";
    case "playback.pause":
      return "暂停播放";
    case "playback.next":
      return "切换到指定曲目";
    case "playback.stop":
      return "停止播放";
    default:
      return action;
  }
}

export function formatAuthState(state: string): string {
  switch (state) {
    case "unknown":
      return "未知";
    case "qr_wait_scan":
      return "等待扫码";
    case "qr_wait_confirm":
      return "等待确认";
    case "qr_expired":
      return "二维码已过期";
    case "logged_in":
      return "已登录";
    case "logged_out":
      return "未登录";
    default:
      return state;
  }
}

export function formatEventName(name: string): string {
  switch (name) {
    case "lagged":
      return "事件流积压";
    case "config.loaded":
      return "配置加载";
    case "config.saved":
      return "配置保存";
    case "config.load.failed":
      return "配置加载失败";
    case "config.save.failed":
      return "配置保存失败";
    case "config.applied.temp":
      return "配置临时应用";
    case "stream.error":
      return "事件流错误";
    default:
      return name;
  }
}

export function asRecord(value: unknown): Record<string, unknown> | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

export function asNumber(value: unknown): number | null {
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

export function asNonEmptyString(value: unknown): string | null {
  if (typeof value !== "string") {
    return null;
  }
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function formatAuthCode(code: number): string {
  switch (code) {
    case 803:
      return "803（登录成功）";
    case 802:
      return "802（已扫码，待确认）";
    case 801:
      return "801（等待扫码）";
    case 800:
      return "800（二维码过期）";
    default:
      return String(code);
  }
}
