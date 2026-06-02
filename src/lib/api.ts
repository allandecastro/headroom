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

export interface Pace {
  daily_rate: number; // %/day burned over the recent ~24h lookback
  safe_pace: number; // max %/day that lands at 100% exactly at reset
  over_pace: boolean;
  low_confidence: boolean; // lookback straddled an app-closed gap — show as tentative
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
  sparkline?: number[]; // downsampled recent utilization for a sparkline
  pace?: Pace; // recent-burn-rate pace, only for long windows
}

// Copilot's normalized usage, tagged on billing regime. Mirrors the Rust
// `CopilotUsage` enum in src-tauri/src/sources/copilot.rs. The endpoint is
// undocumented and changed with the 2026-06-01 AI-Credits migration, so the UI
// must render every case — including `unknown` — and degrade gracefully.
export type CopilotUsage =
  | {
      mode: 'premium_requests'; // legacy request-based billing (grandfathered annual)
      entitlement: number;
      remaining: number;
      used: number;
      percent_remaining: number | null;
      overage_permitted: boolean;
      reset_date: string; // ISO 8601
    }
  | {
      mode: 'ai_credits_capped'; // usage-based, with a per-seat cap (1 credit = $0.01)
      entitlement: number;
      remaining: number;
      used: number;
      percent_remaining: number | null;
      overage_permitted: boolean;
      reset_date: string;
    }
  | {
      mode: 'ai_credits_pooled'; // usage-based, no per-seat cap (org pool) — show no bar/count
      reset_date: string;
    }
  | { mode: 'unknown'; raw_snapshot_ids: string[] };

export interface ServiceStatus {
  id: string;
  name: string;
  plan: string;
  state: ServiceState;
  quotas: Quota[];
  error_detail?: string;
  copilot_usage?: CopilotUsage; // Copilot only; drives the Unlimited / Unknown states
}

export interface Snapshot {
  polled_at: number; // Unix timestamp (seconds)
  services: ServiceStatus[];
}

// Matches src-tauri/src/updates.rs UpdateInfo.
export interface UpdateInfo {
  version: string; // latest release version, no leading "v" (e.g. "1.3.0")
  url: string; // release page to open for the download
}
