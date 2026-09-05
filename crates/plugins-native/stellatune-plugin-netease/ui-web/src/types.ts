export interface NeteaseSourceConfig {
  sidecar_base_url: string;
  sidecar_path: string | null;
  sidecar_args: string[];
  api_request_timeout_ms: number;
  stream_read_timeout_ms: number | null;
  default_level: string;
}

export interface PluginConfigResponse {
  config: Record<string, unknown>;
}

export interface ActionInvokeResponse {
  plugin_id: string;
  action: string;
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
