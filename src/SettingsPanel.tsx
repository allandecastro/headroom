// Settings window.

import type { ReactNode } from 'react';
import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
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
  setCopilotPlan,
} from './lib/ipc';
import type { CopilotPlan, Settings } from './lib/ipc';
import type { Snapshot, ServiceStatus } from './lib/api';

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

// ─── Copilot plan picker (only when Copilot is connected) ───────────────────

const COPILOT_PLAN_OPTIONS: { value: CopilotPlan; label: string }[] = [
  { value: 'free', label: 'Free' },
  { value: 'pro', label: 'Pro' },
  { value: 'pro_plus', label: 'Pro+' },
];

function CopilotPlanRow({ plan: initialPlan }: { plan: CopilotPlan }) {
  const [plan, setPlan] = useState<CopilotPlan>(initialPlan);
  // Re-sync if a fresh snapshot brings a different stored plan (e.g. changed
  // from the onboarding window).
  useEffect(() => setPlan(initialPlan), [initialPlan]);

  function change(next: CopilotPlan) {
    setPlan(next);
    setCopilotPlan(next).catch(console.error);
  }

  return (
    <div className="flex items-center gap-2.5 py-1.5 pl-[26px]">
      <span className="flex-1 text-[11px] text-fg-tertiary">Copilot plan</span>
      <SegmentedControl
        ariaLabel="Copilot plan"
        options={COPILOT_PLAN_OPTIONS}
        value={plan}
        onChange={change}
      />
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
  const bodyRef = useRef<HTMLDivElement>(null);
  useFitWindowHeight(bodyRef, 480);

  // Load settings + autostart state from backend on mount
  useEffect(() => {
    getSettings().then(setLocalSettings).catch(console.error);
    getAutostart().then(setAutostartState).catch(console.error);
  }, []);

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

    return () => {
      unlisten.then((fn) => fn()).catch(console.error);
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
  const copilotSvc = snapshot?.services.find((s) => s.id === 'copilot');

  return (
    <div className="min-h-screen bg-window-opaque text-fg-primary">
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
        <Group label="Services">
          <div>
            <ServiceRow
              icon={<ClaudeIcon />}
              svc={claudeSvc}
              staticName="Claude"
              credentialKey="claude"
              onSignOut={refreshSnapshot}
            />
            <ServiceRow
              icon={<GitHubIcon />}
              svc={copilotSvc}
              staticName="GitHub Copilot"
              credentialKey="copilot"
              onSignOut={refreshSnapshot}
            />
            {copilotSvc?.state === 'active' && (
              <CopilotPlanRow plan={(copilotSvc.plan as CopilotPlan) || 'pro'} />
            )}
          </div>
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
              <span className="text-[12px] text-fg-secondary">Headroom · v1.0.0</span>
              <span className="mt-0.5 text-[10.5px] text-fg-quaternary">
                Know your headroom — quota meter for Claude & GitHub Copilot
              </span>
            </div>
            <button
              onClick={() => openUrl(`${GITHUB_URL}/releases`)}
              className="text-[10.5px] text-fg-quaternary hover:text-fg-secondary"
            >
              Check for updates
            </button>
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
