export interface ConfigFormState {
  sidecarBaseUrl: string;
  sidecarPath: string;
  sidecarArgsText: string;
  apiRequestTimeoutMs: number;
  streamReadTimeoutMs: string;
  defaultLevel: string;
}

export interface SummaryField {
  label: string;
  value: string;
}

export interface UiEventRow {
  id: number;
  time: string;
  name: string;
  source: string;
  summary: string;
  raw: string;
  searchable: string;
}
