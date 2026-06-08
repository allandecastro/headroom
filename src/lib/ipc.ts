// Typed wrappers around the Tauri credential-write commands.
// Backend: src-tauri/src/lib.rs (set_claude_session, set_copilot_*, clear_credentials).
// Tauri maps JS camelCase keys to the Rust snake_case parameter names.

import { invoke } from '@tauri-apps/api/core';
import type { UpdateInfo } from './api';

export type CredentialService = 'claude' | 'copilot';

/** Store the Claude `sessionKey` cookie value in the OS keychain. */
export function setClaudeSession(sessionKey: string): Promise<void> {
  return invoke('set_claude_session', { sessionKey });
}

/** Store the GitHub token used to read Copilot quota in the OS keychain. */
export function setCopilotToken(token: string): Promise<void> {
  return invoke('set_copilot_token', { token });
}

export interface CopilotSigninStart {
  user_code: string;
  verification_uri: string;
  expires_in: number;
}

/**
 * Begin the GitHub device-flow sign-in for Copilot. Resolves with the code to
 * display; completion arrives asynchronously via the `copilot-signed-in` event
 * (or `copilot-signin-error` with a message).
 */
export function startCopilotSignin(): Promise<CopilotSigninStart> {
  return invoke('start_copilot_signin');
}

/** Clear every credential stored under a service's keychain prefix. */
export function clearCredentials(service: CredentialService): Promise<void> {
  return invoke('clear_credentials', { service });
}

// ─── Copilot accounts (multi-account) ────────────────────────────────────────

/** A connected GitHub Copilot account. `label` defaults to the login but is renameable. */
export interface CopilotAccount {
  id: string;
  login: string;
  label: string;
}

/** List the connected Copilot accounts. */
export function listCopilotAccounts(): Promise<CopilotAccount[]> {
  return invoke('list_copilot_accounts');
}

/** Disconnect one Copilot account (deletes its token + registry entry). */
export function removeCopilotAccount(id: string): Promise<void> {
  return invoke('remove_copilot_account', { id });
}

/** Rename a Copilot account's display label (e.g. to its org name). */
export function setCopilotAccountLabel(id: string, label: string): Promise<void> {
  return invoke('set_copilot_account_label', { id, label });
}

// ─── Settings IPC ────────────────────────────────────────────────────────────

export interface Settings {
  poll_interval_secs: number; // 15 | 30 | 60 | 300
  theme: 'auto' | 'light' | 'dark';
  show_tray_percentage: boolean;
  notify_warn_pct: number; // orange alert threshold (0 = off)
  notify_crit_pct: number; // red alert threshold (0 = off)
  show_claude_design: boolean;
  check_updates: boolean; // auto-check GitHub for newer releases
  codex_live_query: boolean; // query Codex's /codex/usage endpoint vs. local logs only
  notified_update_version: string; // version last toasted about (carried, not shown)
}

/** Load persisted settings from the backend. */
export function getSettings(): Promise<Settings> {
  return invoke('get_settings');
}

/** Persist updated settings to the backend. */
export function setSettings(settings: Settings): Promise<void> {
  return invoke('set_settings', { settings });
}

/** Open (or focus) the onboarding window. */
export function openOnboarding(): Promise<void> {
  return invoke('open_onboarding');
}

/**
 * Open the embedded Claude login window. After the user signs in, the backend
 * captures the `sessionKey` cookie, stores it, and emits `claude-signed-in` (or
 * `claude-signin-error` with a message if sign-in times out).
 */
export function startClaudeSignin(): Promise<void> {
  return invoke('start_claude_signin');
}

/** Open (or focus) the settings window. */
export function openSettings(): Promise<void> {
  return invoke('open_settings');
}

/** Whether Headroom launches at login. */
export function getAutostart(): Promise<boolean> {
  return invoke('get_autostart');
}

/** Enable/disable launching Headroom at login. */
export function setAutostart(enabled: boolean): Promise<void> {
  return invoke('set_autostart', { enabled });
}

/** The cached update result (null = up to date / not yet checked). */
export function getUpdate(): Promise<UpdateInfo | null> {
  return invoke('get_update');
}

/** Force an update check now; resolves with the newer release or null. */
export function checkForUpdateNow(): Promise<UpdateInfo | null> {
  return invoke('check_for_update_now');
}

/** Download progress for an in-app update, emitted as the `update-progress` event. */
export interface UpdateProgress {
  downloaded: number;
  content_length: number | null;
}

/** What the renderer should do after {@link installUpdate}. */
export type InstallOutcome = { kind: 'open_url'; url: string };

/**
 * Download and install the latest release, then relaunch. On a successful
 * in-app install the app restarts and this promise never resolves. On platforms
 * that can't self-install (macOS, `.deb`) — or any updater failure — it resolves
 * with `{ kind: 'open_url' }` so the caller opens the release page instead.
 * Subscribe to the `update-progress` event for a progress bar while it runs.
 */
export function installUpdate(): Promise<InstallOutcome> {
  return invoke('install_update');
}

/**
 * Fetch the raw `copilot_internal/user` payload (token redacted) for diagnostics
 * — used to capture the real (undocumented, post-migration) shape for reporting.
 */
export function copilotDiagnostics(accountId?: string): Promise<string> {
  return invoke('copilot_diagnostics', { accountId });
}

/**
 * Fetch the raw Claude `/usage` payload (sessionKey redacted) for diagnostics —
 * the Claude counterpart to {@link copilotDiagnostics}.
 */
export function claudeDiagnostics(): Promise<string> {
  return invoke('claude_diagnostics');
}

/**
 * Dump Codex diagnostics: resolved home, CLI version, auth mode, the live
 * `/codex/usage` payload (tokens redacted) when reachable, and the latest local
 * `rate_limits` line. Used to report exec-mode null logs or shape changes.
 */
export function codexDiagnostics(): Promise<string> {
  return invoke('codex_diagnostics');
}
