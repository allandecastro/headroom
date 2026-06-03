// Settings window.

import type { ReactNode } from 'react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-shell';
import { useFitWindowHeight } from './lib/useFitWindow';
import { SegmentedControl } from './components/ui/SegmentedControl';
import { Toggle } from './components/ui/Toggle';
import { Slider } from './components/ui/Slider';
import { Button } from './components/ui/Button';
import { ClaudeIcon, GitHubIcon, LinkedInIcon } from './components/onboarding/icons';
import {
  getSettings,
  setSettings,
  openOnboarding,
  clearCredentials,
  getAutostart,
  setAutostart,
  getUpdate,
  checkForUpdateNow,
  installUpdate,
  copilotDiagnostics,
  claudeDiagnostics,
  listCopilotAccounts,
  removeCopilotAccount,
  setCopilotAccountLabel,
} from './lib/ipc';
import type { Settings, UpdateProgress, CopilotAccount } from './lib/ipc';
import type { Snapshot, ServiceStatus, UpdateInfo } from './lib/api';

// ─── Poll interval options ───────────────────────────────────────────────────

type PollValue = '15' | '30' | '60' | '300';

const POLL_OPTIONS: { value: PollValue; label: string }[] = [
  { value: '15', label: '15s' },
  { value: '30', label: '30s' },
  { value: '60', label: '1m' },
  { value: '300', label: '5m' },
];

// ─── Theme options ───────────────────────────────────────────────────────────

type Theme = 'auto' | 'light' | 'dark';

const THEME_OPTIONS: { value: Theme; label: string }[] = [
  { value: 'auto', label: 'Auto' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
];

// ─── Default settings (used before backend responds) ─────────────────────────

const DEFAULT_SETTINGS: Settings = {
  poll_interval_secs: 30,
  theme: 'auto',
  show_tray_percentage: true,
  notify_warn_pct: 80,
  notify_crit_pct: 95,
  show_claude_design: false,
  check_updates: true,
  notified_update_version: '',
};

// Open an external URL in the default browser.
function openUrl(url: string): void {
  open(url).catch(console.error);
}

const GITHUB_URL = 'https://github.com/allandecastro/headroom';
const LINKEDIN_URL = 'https://www.linkedin.com/in/allandecastro/';

// ─── Connected pill ──────────────────────────────────────────────────────────

function ConnectedPill() {
  return (
    <span
      className="inline-flex items-center gap-1 text-[10px]"
      style={{ color: 'var(--ok-text, #5a7d3a)' }}
    >
      <span
        aria-hidden
        style={{
          width: 5,
          height: 5,
          borderRadius: '50%',
          background: 'var(--ok-fill, #6a8e4a)',
          display: 'inline-block',
          flexShrink: 0,
        }}
      />
      Connected
    </span>
  );
}

// Credentials stored but the source is failing — still "connected", just
// couldn't fetch.
function ProblemPill({ label }: { label: string }) {
  return (
    <span className="inline-flex items-center gap-1 text-[10px] text-state-warn-text-dark">
      <span
        aria-hidden
        className="inline-block h-[5px] w-[5px] flex-shrink-0 rounded-full bg-state-warn-fill-dark"
      />
      {label}
    </span>
  );
}

// ─── Service row ─────────────────────────────────────────────────────────────

interface ServiceRowProps {
  icon: ReactNode;
  svc: ServiceStatus | undefined;
  staticName: string;
  credentialKey: 'claude' | 'copilot';
  onSignOut: () => void;
}

function ServiceRow({ icon, svc, staticName, credentialKey, onSignOut }: ServiceRowProps) {
  const state = svc?.state;
  const displayName = svc ? (svc.plan ? `${svc.name} · ${svc.plan}` : svc.name) : staticName;

  // Three distinct states: connected & healthy, connected but failing, no creds.
  const statusEl =
    state === 'active' ? (
      <ConnectedPill />
    ) : state === 'unreachable' ? (
      <ProblemPill label="Connected · couldn't fetch usage" />
    ) : state === 'auth_required' ? (
      <ProblemPill label="Connected · sign in again" />
    ) : (
      <span>Not connected</span>
    );

  return (
    <div className="flex items-center gap-2.5 py-2">
      <span className="text-fg-secondary">{icon}</span>
      <div className="flex flex-col flex-1 min-w-0">
        <span className="text-[12px] text-fg-primary">{displayName}</span>
        <span
          className="text-[10px] text-fg-tertiary flex items-center gap-1"
          title={svc?.error_detail ?? undefined}
        >
          {statusEl}
        </span>
      </div>
      <div className="flex gap-1.5">
        <Button
          className="text-xxs px-[9px] py-[3px]"
          onClick={() => openOnboarding().catch(console.error)}
        >
          Re-auth
        </Button>
        <Button
          className="text-xxs px-[9px] py-[3px]"
          onClick={() => {
            clearCredentials(credentialKey).then(onSignOut).catch(console.error);
          }}
        >
          Sign out
        </Button>
      </div>
    </div>
  );
}

// ─── Copilot accounts (multi-account) ────────────────────────────────────────

function CopilotAccountRow({
  account,
  svc,
  onChange,
}: {
  account: CopilotAccount;
  svc: ServiceStatus | undefined;
  onChange: () => void;
}) {
  const [label, setLabel] = useState(account.label);
  const [diagState, setDiagState] = useState<'idle' | 'copying' | 'done' | 'error'>('idle');
  const state = svc?.state;

  // Copy THIS account's raw payload (token redacted) — account-scoped so the
  // right account's diagnostics land on the clipboard with several connected.
  async function copyDiag() {
    setDiagState('copying');
    try {
      await navigator.clipboard.writeText(await copilotDiagnostics(account.id));
      setDiagState('done');
      setTimeout(() => setDiagState('idle'), 1500);
    } catch (e) {
      console.error(e);
      setDiagState('error');
    }
  }
  const diagLabel =
    diagState === 'copying'
      ? 'Copying…'
      : diagState === 'done'
        ? 'Copied ✓'
        : diagState === 'error'
          ? 'Failed'
          : 'Diagnostics';

  const statusEl =
    state === 'active' ? (
      <ConnectedPill />
    ) : state === 'unreachable' ? (
      <ProblemPill label="couldn't fetch usage" />
    ) : state === 'auth_required' ? (
      <ProblemPill label="sign in again" />
    ) : (
      <span>Connecting…</span>
    );

  // Commit a rename on blur / Enter; empty falls back to the GitHub login.
  function commitLabel() {
    const next = label.trim() || account.login;
    if (next !== label) setLabel(next);
    if (next !== account.label) {
      setCopilotAccountLabel(account.id, next).then(onChange).catch(console.error);
    }
  }

  return (
    <div className="flex items-center gap-2.5 py-2">
      <span className="text-fg-secondary">
        <GitHubIcon />
      </span>
      <div className="flex min-w-0 flex-1 flex-col">
        <input
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          onBlur={commitLabel}
          onKeyDown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
          aria-label="Account label"
          className="w-full bg-transparent text-[12px] text-fg-primary outline-none focus:underline"
        />
        <span className="flex items-center gap-1 text-[10px] text-fg-tertiary">
          @{account.login}
          {svc?.plan ? ` · ${svc.plan}` : ''} · {statusEl}
        </span>
      </div>
      <button
        onClick={copyDiag}
        disabled={diagState === 'copying'}
        className="text-[10.5px] text-fg-quaternary hover:text-fg-secondary disabled:opacity-60"
      >
        {diagLabel}
      </button>
      <Button
        className="text-xxs px-[9px] py-[3px]"
        onClick={() => removeCopilotAccount(account.id).then(onChange).catch(console.error)}
      >
        Remove
      </Button>
    </div>
  );
}

function CopilotAccounts({
  snapshot,
  onChange,
}: {
  snapshot: Snapshot | null;
  onChange: () => void;
}) {
  const [accounts, setAccounts] = useState<CopilotAccount[]>([]);

  const load = useCallback(() => {
    listCopilotAccounts().then(setAccounts).catch(console.error);
  }, []);

  useEffect(() => {
    load();
    // Refresh the list when an account is added (onboarding) or any change
    // re-emits the snapshot.
    const unAdded = listen('copilot-signed-in', load);
    const unTokens = listen('tokens-updated', load);
    return () => {
      unAdded.then((fn) => fn()).catch(console.error);
      unTokens.then((fn) => fn()).catch(console.error);
    };
  }, [load]);

  return (
    <div>
      {accounts.length === 0 && (
        <div className="py-2 text-[11px] text-fg-quaternary">No GitHub accounts connected.</div>
      )}
      {accounts.map((a) => (
        <CopilotAccountRow
          key={a.id}
          account={a}
          svc={snapshot?.services.find((s) => s.id === `copilot:${a.id}`)}
          onChange={() => {
            load();
            onChange();
          }}
        />
      ))}
      <div className="pt-1.5">
        <Button
          className="text-xxs px-[9px] py-[3px]"
          onClick={() => openOnboarding().catch(console.error)}
        >
          + Add GitHub account
        </Button>
      </div>
    </div>
  );
}

// ─── Section wrapper ──────────────────────────────────────────────────────────

function Group({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="py-3">
      <div className="text-[10px] font-medium uppercase tracking-[0.08em] text-fg-tertiary mb-3">
        {label}
      </div>
      {children}
    </div>
  );
}

// ─── Setting row (label + control) ───────────────────────────────────────────

function Row({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center justify-between py-1.5 min-h-[28px] [&+&]:mt-0.5">
      {children}
    </div>
  );
}

// ─── Main component ───────────────────────────────────────────────────────────

export default function SettingsPanel() {
  const [settings, setLocalSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [autostart, setAutostartState] = useState(false);
  const [appVersion, setAppVersion] = useState('');
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [updateStatus, setUpdateStatus] = useState<
    'idle' | 'checking' | 'uptodate' | 'installing' | 'brew' | 'error'
  >('idle');
  const [installPct, setInstallPct] = useState<number | null>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  useFitWindowHeight(bodyRef, 480);

  // Load settings + autostart state from backend on mount
  useEffect(() => {
    getSettings().then(setLocalSettings).catch(console.error);
    getAutostart().then(setAutostartState).catch(console.error);
    getVersion().then(setAppVersion).catch(console.error);
    getUpdate().then(setUpdateInfo).catch(console.error);
  }, []);

  const [diag, setDiag] = useState<{
    which: 'claude' | 'copilot';
    state: 'copying' | 'done' | 'error';
  } | null>(null);

  // Copy the raw (token-redacted) usage payload for a service to the clipboard —
  // for reporting these undocumented endpoints' real shapes. Claude reads
  // /api/.../usage; Copilot reads copilot_internal/user.
  function copyDiagnostics(which: 'claude' | 'copilot') {
    setDiag({ which, state: 'copying' });
    const fetcher = which === 'claude' ? claudeDiagnostics : copilotDiagnostics;
    fetcher()
      .then((raw) => navigator.clipboard.writeText(raw))
      .then(() => setDiag({ which, state: 'done' }))
      .catch((err) => {
        console.error(err);
        setDiag({ which, state: 'error' });
      });
  }

  function diagLabel(which: 'claude' | 'copilot', name: string): string {
    if (diag?.which !== which) return name;
    return diag.state === 'copying' ? 'Copying…' : diag.state === 'done' ? 'Copied ✓' : 'Failed';
  }

  // Manual "Check for updates" — resolves to the newer release or null.
  function runUpdateCheck() {
    setUpdateStatus('checking');
    checkForUpdateNow()
      .then((u) => {
        setUpdateInfo(u);
        setUpdateStatus(u ? 'idle' : 'uptodate');
      })
      .catch((err) => {
        console.error(err);
        setUpdateStatus('idle');
      });
  }

  // "Update to vX": download + install + relaunch. On success the app restarts;
  // on an unsupported platform (macOS / .deb) or failure, open the release page.
  async function runInstallUpdate() {
    if (!updateInfo) return;
    setUpdateStatus('installing');
    setInstallPct(null);
    try {
      const outcome = await installUpdate();
      if (outcome?.kind === 'open_url') {
        openUrl(outcome.url);
        setUpdateStatus('idle');
      } else if (outcome?.kind === 'brew') {
        // brew upgrade runs in Terminal and replaces the app from there.
        setUpdateStatus('brew');
      }
    } catch (err) {
      console.error(err);
      setUpdateStatus('error');
    }
  }

  // Autostart is OS-level (not in settings.json), so toggle it directly.
  function toggleAutostart(enabled: boolean) {
    setAutostartState(enabled);
    setAutostart(enabled).catch((err) => {
      console.error(err);
      setAutostartState(!enabled);
    });
  }

  // Subscribe to snapshot updates and fetch initial snapshot
  useEffect(() => {
    invoke<Snapshot>('refresh_all').then(setSnapshot).catch(console.error);

    const unlisten = listen<Snapshot>('tokens-updated', (event) => {
      setSnapshot(event.payload);
    });
    const unlistenProgress = listen<UpdateProgress>('update-progress', (event) => {
      const { downloaded, content_length } = event.payload;
      setInstallPct(
        content_length ? Math.min(100, Math.round((downloaded / content_length) * 100)) : null,
      );
    });

    return () => {
      unlisten.then((fn) => fn()).catch(console.error);
      unlistenProgress.then((fn) => fn()).catch(console.error);
    };
  }, []);

  // Persist settings on any change
  function update(patch: Partial<Settings>) {
    const next = { ...settings, ...patch };
    setLocalSettings(next);
    setSettings(next).catch(console.error);
  }

  // Notification thresholds: keep critical strictly above heads-up (when both
  // are enabled). Moving one nudges the other rather than allowing crit <= warn.
  function updateThreshold(patch: Partial<Settings>) {
    const next = { ...settings, ...patch };
    const warn = next.notify_warn_pct;
    const crit = next.notify_crit_pct;
    if (warn > 0 && crit > 0 && crit <= warn) {
      if ('notify_warn_pct' in patch) {
        next.notify_crit_pct = Math.min(100, warn + 5);
        if (next.notify_crit_pct <= warn) next.notify_warn_pct = next.notify_crit_pct - 5;
      } else {
        next.notify_warn_pct = Math.max(0, crit - 5);
      }
    }
    setLocalSettings(next);
    setSettings(next).catch(console.error);
  }

  // Re-fetch snapshot after sign-out
  function refreshSnapshot() {
    invoke<Snapshot>('refresh_all').then(setSnapshot).catch(console.error);
  }

  const pollValue = String(settings.poll_interval_secs) as PollValue;

  const claudeSvc = snapshot?.services.find((s) => s.id === 'claude');

  return (
    <div className="h-screen overflow-y-auto bg-window-opaque text-fg-primary">
      <div ref={bodyRef} className="px-[22px] pt-[18px] pb-4">
        {/* POLLING */}
        <Group label="Polling">
          <Row>
            <div className="flex flex-col flex-1">
              <span className="text-[12px] text-fg-secondary">Refresh interval</span>
              <span className="text-[10.5px] text-fg-quaternary mt-0.5">
                How often Headroom polls each service
              </span>
            </div>
            <SegmentedControl
              options={POLL_OPTIONS}
              value={pollValue}
              onChange={(v) => update({ poll_interval_secs: Number(v) })}
              ariaLabel="Refresh interval"
            />
          </Row>
        </Group>

        {/* APPEARANCE */}
        <Group label="Appearance">
          <Row>
            <span className="text-[12px] text-fg-secondary flex-1">Theme</span>
            <SegmentedControl
              options={THEME_OPTIONS}
              value={settings.theme}
              onChange={(v) => update({ theme: v })}
              ariaLabel="Theme"
            />
          </Row>
          <Row>
            <span className="text-[12px] text-fg-secondary flex-1">Show percentage in tray</span>
            <Toggle
              checked={settings.show_tray_percentage}
              onChange={(v) => update({ show_tray_percentage: v })}
              ariaLabel="Show percentage in tray"
            />
          </Row>
          <Row>
            <span className="flex-1 text-[12px] text-fg-secondary">Launch at startup</span>
            <Toggle checked={autostart} onChange={toggleAutostart} ariaLabel="Launch at startup" />
          </Row>
          <Row>
            <div className="flex flex-1 flex-col">
              <span className="text-[12px] text-fg-secondary">Show Claude Design usage</span>
              <span className="mt-0.5 text-[10.5px] text-fg-quaternary">
                Extra Claude usage meter in the popover
              </span>
            </div>
            <Toggle
              checked={settings.show_claude_design}
              onChange={(v) => update({ show_claude_design: v })}
              ariaLabel="Show Claude Design usage"
            />
          </Row>
        </Group>

        {/* SERVICES */}
        <Group label="Claude">
          <ServiceRow
            icon={<ClaudeIcon />}
            svc={claudeSvc}
            staticName="Claude"
            credentialKey="claude"
            onSignOut={refreshSnapshot}
          />
        </Group>

        <Group label="GitHub Copilot accounts">
          <CopilotAccounts snapshot={snapshot} onChange={refreshSnapshot} />
        </Group>

        {/* NOTIFICATIONS */}
        <Group label="Notifications">
          <Row>
            <div className="flex flex-1 flex-col">
              <span className="text-[12px] text-fg-secondary">Heads-up alert 🟠</span>
              <span className="mt-0.5 text-[10.5px] text-fg-quaternary">
                Orange notification at this usage
              </span>
            </div>
            <Slider
              value={settings.notify_warn_pct}
              onChange={(v) => updateThreshold({ notify_warn_pct: v })}
              accent="#d99c52"
              ariaLabel="Heads-up threshold"
            />
          </Row>
          <Row>
            <div className="flex flex-1 flex-col">
              <span className="text-[12px] text-fg-secondary">Critical alert 🔴</span>
              <span className="mt-0.5 text-[10.5px] text-fg-quaternary">
                Red notification at this usage
              </span>
            </div>
            <Slider
              value={settings.notify_crit_pct}
              onChange={(v) => updateThreshold({ notify_crit_pct: v })}
              accent="#d4625d"
              ariaLabel="Critical threshold"
            />
          </Row>
        </Group>

        {/* ABOUT */}
        <Group label="About">
          <Row>
            <div className="flex flex-1 flex-col">
              <span className="text-[12px] text-fg-secondary">Headroom · v{appVersion}</span>
              <span className="mt-0.5 text-[10.5px] text-fg-quaternary">
                Know your headroom — quota meter for Claude & GitHub Copilot
              </span>
            </div>
            {updateInfo ? (
              updateStatus === 'brew' ? (
                <span className="text-[10.5px] text-fg-quaternary">Upgrading in Terminal…</span>
              ) : updateStatus === 'installing' ? (
                <span className="text-[10.5px] text-fg-quaternary">
                  {installPct !== null ? `Downloading… ${installPct}%` : 'Installing…'}
                </span>
              ) : updateStatus === 'error' ? (
                <button
                  onClick={() => openUrl(updateInfo.url)}
                  className="text-[10.5px] font-medium text-state-ok-text hover:underline dark:text-state-ok-text-dark"
                >
                  Update failed — open page →
                </button>
              ) : (
                <button
                  onClick={runInstallUpdate}
                  className="text-[10.5px] font-medium text-state-ok-text hover:underline dark:text-state-ok-text-dark"
                >
                  Update to v{updateInfo.version} →
                </button>
              )
            ) : (
              <button
                onClick={runUpdateCheck}
                disabled={updateStatus === 'checking'}
                className="text-[10.5px] text-fg-quaternary hover:text-fg-secondary disabled:opacity-60"
              >
                {updateStatus === 'checking'
                  ? 'Checking…'
                  : updateStatus === 'uptodate'
                    ? 'Up to date ✓'
                    : 'Check for updates'}
              </button>
            )}
          </Row>
          <Row>
            <div className="flex flex-1 flex-col">
              <span className="text-[12px] text-fg-secondary">Check for updates automatically</span>
              <span className="mt-0.5 text-[10.5px] text-fg-quaternary">
                Notify me when a newer version is released
              </span>
            </div>
            <Toggle
              checked={settings.check_updates}
              onChange={(v) => update({ check_updates: v })}
              ariaLabel="Check for updates automatically"
            />
          </Row>
          <Row>
            <div className="flex flex-1 flex-col">
              <span className="text-[12px] text-fg-secondary">Claude diagnostics</span>
              <span className="mt-0.5 text-[10.5px] text-fg-quaternary">
                Copy Claude’s raw usage payload (token redacted) to report a problem
              </span>
            </div>
            <div className="flex items-center gap-3">
              <button
                onClick={() => copyDiagnostics('claude')}
                disabled={diag?.state === 'copying'}
                className="text-[10.5px] text-fg-quaternary hover:text-fg-secondary disabled:opacity-60"
              >
                {diagLabel('claude', 'Claude')}
              </button>
            </div>
          </Row>
          <Row>
            <span className="flex-1 text-[12px] text-fg-secondary">Made by Allan De Castro</span>
            <div className="flex items-center gap-3 text-fg-tertiary">
              <button
                aria-label="GitHub repository"
                onClick={() => openUrl(GITHUB_URL)}
                className="hover:text-fg-primary"
              >
                <GitHubIcon />
              </button>
              <button
                aria-label="LinkedIn — Allan De Castro"
                onClick={() => openUrl(LINKEDIN_URL)}
                className="hover:text-fg-primary"
              >
                <LinkedInIcon />
              </button>
            </div>
          </Row>
        </Group>
      </div>
    </div>
  );
}
