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
      mode: 'premium_requests'; // a bounded request count: legacy premium requests, or Free chat/completions
      label: string; // "Premium requests" | "Chat" | "Completions"
      entitlement: number;
      remaining: number;
      used: number;
      percent_remaining: number | null;
      overage_permitted: boolean;
      reset_date: string; // ISO 8601
    }
  | {
      mode: 'ai_credits_capped'; // usage-based, with a per-seat cap (1 credit = $0.01)
      label: string; // "AI Credits", or the quota's own name (e.g. "Chat") for a Free request cap
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

// Codex extras, mirroring the Rust `CodexMeta` in src-tauri/src/sources/codex.rs.
// Codex usage is read locally (no sign-in): either the live `/codex/usage`
// endpoint or the local rollout logs. This block carries the "as of / source"
// info, credits balance, and token-consumption stats the card renders.
export type CodexSourceKind = 'live' | 'logs' | 'empty';

export interface CodexTokenStats {
  input: number;
  cached_input: number;
  output: number;
  reasoning: number;
  total: number;
  window_label: string; // e.g. "last 24h"
}

export interface CodexMeta {
  source: CodexSourceKind;
  captured_at?: string; // ISO 8601 — when the snapshot was recorded
  stale?: boolean; // a log snapshot whose window already reset
  token_expired?: boolean; // the live token was rejected — run `codex` to refresh
  credits_balance?: string; // e.g. "$12.34", when present
  token_stats?: CodexTokenStats;
  note?: string; // guidance shown when there are no % rows
}

export interface ServiceStatus {
  id: string;
  name: string;
  plan: string;
  state: ServiceState;
  quotas: Quota[];
  error_detail?: string;
  copilot_usage?: CopilotUsage; // Copilot only; drives the Unlimited / Unknown states
  codex_meta?: CodexMeta; // Codex only; "as of / source", credits, token stats
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
