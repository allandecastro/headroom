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

// ─── Settings IPC ────────────────────────────────────────────────────────────

export interface Settings {
  poll_interval_secs: number; // 15 | 30 | 60 | 300
  theme: 'auto' | 'light' | 'dark';
  show_tray_percentage: boolean;
  notify_warn_pct: number; // orange alert threshold (0 = off)
  notify_crit_pct: number; // red alert threshold (0 = off)
  show_claude_design: boolean;
  check_updates: boolean; // auto-check GitHub for newer releases
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
 * captures the `sessionKey` cookie, stores it, and emits `claude-signed-in`.
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

/**
 * Fetch the raw `copilot_internal/user` payload (token redacted) for diagnostics
 * — used to capture the real (undocumented, post-migration) shape for reporting.
 */
export function copilotDiagnostics(): Promise<string> {
  return invoke('copilot_diagnostics');
}
