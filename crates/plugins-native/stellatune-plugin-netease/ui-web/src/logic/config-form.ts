import { DEFAULT_SOURCE_CONFIG, type NeteaseSourceConfig } from "../types";
import type { ConfigFormState } from "../view-models";

export function hydrateForm(form: ConfigFormState, rawConfig: unknown): void {
  const merged = normalizeSourceConfig(rawConfig);
  form.sidecarBaseUrl = merged.sidecar_base_url;
  form.sidecarPath = merged.sidecar_path ?? "";
  form.sidecarArgsText = merged.sidecar_args.join("\n");
  form.apiRequestTimeoutMs = merged.api_request_timeout_ms;
  form.streamReadTimeoutMs =
    merged.stream_read_timeout_ms === null ? "" : String(merged.stream_read_timeout_ms);
  form.defaultLevel = merged.default_level;
}

export function buildSourceConfig(form: ConfigFormState): NeteaseSourceConfig {
  const streamReadTimeout = form.streamReadTimeoutMs.trim();
  const sidecarPath = form.sidecarPath.trim();
  return {
    sidecar_base_url: form.sidecarBaseUrl.trim() || DEFAULT_SOURCE_CONFIG.sidecar_base_url,
    sidecar_path: sidecarPath.length > 0 ? sidecarPath : null,
    sidecar_args: form.sidecarArgsText
      .split(/\r?\n/)
      .map((item) => item.trim())
      .filter((item) => item.length > 0),
    api_request_timeout_ms: Math.max(500, Number(form.apiRequestTimeoutMs) || 8000),
    stream_read_timeout_ms:
      streamReadTimeout.length > 0 ? Math.max(500, Number(streamReadTimeout) || 500) : null,
    default_level: form.defaultLevel.trim() || "standard"
  };
}

function normalizeSourceConfig(rawConfig: unknown): NeteaseSourceConfig {
  if (typeof rawConfig !== "object" || rawConfig === null) {
    return { ...DEFAULT_SOURCE_CONFIG };
  }
  const record = rawConfig as Record<string, unknown>;
  return {
    sidecar_base_url: readString(record.sidecar_base_url, DEFAULT_SOURCE_CONFIG.sidecar_base_url),
    sidecar_path: readNullableString(record.sidecar_path),
    sidecar_args: readStringArray(record.sidecar_args),
    api_request_timeout_ms: readNumber(record.api_request_timeout_ms, 8000),
    stream_read_timeout_ms: readNullableNumber(record.stream_read_timeout_ms),
    default_level: readString(record.default_level, "standard")
  };
}

function readString(raw: unknown, fallback: string): string {
  if (typeof raw !== "string") {
    return fallback;
  }
  const trimmed = raw.trim();
  return trimmed.length > 0 ? trimmed : fallback;
}

function readNullableString(raw: unknown): string | null {
  if (typeof raw !== "string") {
    return null;
  }
  const trimmed = raw.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function readStringArray(raw: unknown): string[] {
  if (!Array.isArray(raw)) {
    return [];
  }
  return raw
    .filter((item): item is string => typeof item === "string")
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

function readNumber(raw: unknown, fallback: number): number {
  if (typeof raw !== "number" || !Number.isFinite(raw)) {
    return fallback;
  }
  return Math.max(0, Math.trunc(raw));
}

function readNullableNumber(raw: unknown): number | null {
  if (typeof raw !== "number" || !Number.isFinite(raw)) {
    return null;
  }
  return Math.max(0, Math.trunc(raw));
}
