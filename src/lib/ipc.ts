// Typed wrappers around the Tauri credential-write commands.
// Backend: src-tauri/src/lib.rs (set_claude_session, set_copilot_*, clear_credentials).
// Tauri maps JS camelCase keys to the Rust snake_case parameter names.

import { invoke } from '@tauri-apps/api/core';

export type CopilotPlan = 'free' | 'pro' | 'pro_plus';

export type CredentialService = 'claude' | 'copilot';

/** Store the Claude `sessionKey` cookie value in the OS keychain. */
export function setClaudeSession(sessionKey: string): Promise<void> {
  return invoke('set_claude_session', { sessionKey });
}

/** Store the Copilot Bearer token (OAuth or PAT) in the OS keychain. */
export function setCopilotToken(token: string): Promise<void> {
  return invoke('set_copilot_token', { token });
}

/** Store the GitHub username used in the Copilot billing API path. */
export function setCopilotUsername(username: string): Promise<void> {
  return invoke('set_copilot_username', { username });
}

/** Store the Copilot plan tier. The backend rejects anything but the known tiers. */
export function setCopilotPlan(plan: CopilotPlan): Promise<void> {
  return invoke('set_copilot_plan', { plan });
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
  notify_80: boolean;
  notify_95: boolean;
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
