// Shapes must stay in sync with src-tauri/src/sources/mod.rs

export type ServiceState = 'active' | 'auth_required' | 'unreachable';

export type QuotaWindow = 'five_hour' | 'weekly_sonnet' | 'weekly_opus' | 'monthly';

export type QuotaUnit = 'messages' | 'hours' | 'requests' | 'usd_credits';

export interface Quota {
  window: QuotaWindow;
  label: string;
  used: number;
  total: number;
  unit: QuotaUnit;
  resets_at: string; // ISO 8601
  advice?: string;
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
