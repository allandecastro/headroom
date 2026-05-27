// Shapes must stay in sync with src-tauri/src/sources/mod.rs

export type ServiceState = 'active' | 'needs_setup' | 'auth_required' | 'unreachable';

export type QuotaWindow =
  | 'five_hour'
  | 'weekly_all'
  | 'weekly_sonnet'
  | 'weekly_opus'
  | 'claude_design'
  | 'monthly';

export type QuotaUnit = 'messages' | 'hours' | 'requests' | 'usd_credits' | 'percent';

export interface Projection {
  projected_pct: number; // extrapolated utilization at reset
  will_exceed: boolean;
  eta?: string; // ISO 8601 — when it's projected to hit 100%, if before reset
}

export interface Quota {
  window: QuotaWindow;
  label: string;
  used: number;
  total: number;
  unit: QuotaUnit;
  resets_at: string; // ISO 8601
  advice?: string;
  projection?: Projection;
}

export interface ServiceStatus {
  id: string;
  name: string;
  plan: string;
  state: ServiceState;
  quotas: Quota[];
  error_detail?: string;
}

export interface Snapshot {
  polled_at: number; // Unix timestamp (seconds)
  services: ServiceStatus[];
}
