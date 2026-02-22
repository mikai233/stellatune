export interface NeteaseSourceConfig {
  sidecar_base_url: string;
  sidecar_path: string | null;
  sidecar_args: string[];
  api_request_timeout_ms: number;
  stream_read_timeout_ms: number | null;
  default_level: string;
}

export interface ConfigApplyOutcome {
  kind: string;
  type_id: string;
  status: string;
  detail?: string;
}

export interface ConfigApplyReport {
  plugin_id: string;
  applied: number;
  skipped: number;
  failed: number;
  outcomes: ConfigApplyOutcome[];
}

export interface PluginConfigResponse {
  plugin_id: string;
  config: Record<string, Record<string, unknown>>;
  apply_report?: ConfigApplyReport;
}

export interface ActionInvokeResponse {
  plugin_id: string;
  action: string;
  accepted: boolean;
  message: string;
  data: unknown;
}

export interface PluginUiEvent {
  plugin_id: string;
  name: string;
  payload: unknown;
  ts_ms: number;
}

export const DEFAULT_SOURCE_CONFIG: NeteaseSourceConfig = {
  sidecar_base_url: "http://127.0.0.1:46321",
  sidecar_path: null,
  sidecar_args: [],
  api_request_timeout_ms: 8000,
  stream_read_timeout_ms: null,
  default_level: "standard"
};
