import type { SummaryField } from "../view-models";
import { asNonEmptyString, asRecord, formatActionName, truncateText } from "./auth-display";

export function buildPlaybackSummary(
  action: string,
  responseMessage: string,
  data: unknown
): SummaryField[] {
  const rows: SummaryField[] = [
    { label: "动作", value: formatActionName(action) },
    { label: "网关响应", value: responseMessage }
  ];

  const payload = asRecord(data);
  const dispatch = asNonEmptyString(payload?.dispatch);
  if (dispatch) {
    rows.push({ label: "分发目标", value: dispatch });
  }

  const trackToken = asNonEmptyString(payload?.track_token);
  if (trackToken) {
    rows.push({ label: "Track Token", value: truncateText(trackToken, 80) });
  }

  if (payload?.autoplay === true) {
    rows.push({ label: "执行结果", value: "已切换并开始播放" });
  } else if (payload?.queued === true) {
    rows.push({ label: "执行结果", value: "已加入下一首" });
  } else if (payload?.paused === true) {
    rows.push({ label: "执行结果", value: "已暂停播放" });
  } else if (payload?.stopped === true) {
    rows.push({ label: "执行结果", value: "已停止播放" });
  } else {
    const mode = asNonEmptyString(payload?.mode);
    if (mode) {
      rows.push({ label: "模式", value: mode });
    }
  }

  if (rows.length > 8) {
    return rows.slice(0, 8);
  }
  return rows;
}

export function buildPlaybackEventSummary(responseMessage: string, fields: SummaryField[]): string {
  const important = fields
    .filter((item) => item.label === "执行结果" || item.label === "Track Token")
    .map((item) => `${item.label}=${item.value}`);
  if (important.length === 0) {
    return responseMessage;
  }
  return `${responseMessage}；${important.join("；")}`;
}
