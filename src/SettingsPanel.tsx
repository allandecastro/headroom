// Settings window. Matches docs/mockups/03-settings.html exactly.
// Window label: 'settings'. Tauri window: 460 × 560, no decorations, transparent.

import type { ReactNode } from 'react';
import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useFitWindowHeight } from './lib/useFitWindow';
import { SegmentedControl } from './components/ui/SegmentedControl';
import { Toggle } from './components/ui/Toggle';
import { Button } from './components/ui/Button';
import { ClaudeIcon, GitHubIcon } from './components/onboarding/icons';
import { getSettings, setSettings, openOnboarding, clearCredentials } from './lib/ipc';
import type { Settings } from './lib/ipc';
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
  notify_80: true,
  notify_95: true,
};

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

// ─── Service row ─────────────────────────────────────────────────────────────

interface ServiceRowProps {
  icon: ReactNode;
  svc: ServiceStatus | undefined;
  staticName: string;
  credentialKey: 'claude' | 'copilot';
  onSignOut: () => void;
}

function ServiceRow({ icon, svc, staticName, credentialKey, onSignOut }: ServiceRowProps) {
  const connected = svc?.state === 'active';
  const displayName = svc ? `${svc.name} · ${svc.plan}` : staticName;

  const detailText = connected ? (svc?.error_detail ?? '') : 'Not connected';

  return (
    <div className="flex items-center gap-2.5 py-2">
      <span className="text-fg-secondary">{icon}</span>
      <div className="flex flex-col flex-1 min-w-0">
        <span className="text-[12px] text-fg-primary">{displayName}</span>
        <span className="text-[10px] text-fg-tertiary flex items-center gap-1">
          {connected ? (
            <>
              <ConnectedPill />
              {detailText && <span>· {detailText}</span>}
            </>
          ) : (
            <span>Not connected</span>
          )}
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

// ─── Section wrapper ──────────────────────────────────────────────────────────

function Group({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="py-3 [&+&]:border-t [&+&]:border-hairline [&+&]:border-default">
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
  const bodyRef = useRef<HTMLDivElement>(null);
  useFitWindowHeight(bodyRef, 480);

  // Load settings from backend on mount
  useEffect(() => {
    getSettings().then(setLocalSettings).catch(console.error);
  }, []);

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
        </Group>

        {/* SERVICES */}
        <Group label="Services">
          <div className="[&>div+div]:border-t [&>div+div]:border-hairline [&>div+div]:border-default">
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
          </div>
        </Group>

        {/* NOTIFICATIONS */}
        <Group label="Notifications">
          <Row>
            <div className="flex flex-col flex-1">
              <span className="text-[12px] text-fg-secondary">Notify at 80%</span>
              <span className="text-[10.5px] text-fg-quaternary mt-0.5">
                Desktop alert when any quota reaches 80%
              </span>
            </div>
            <Toggle
              checked={settings.notify_80}
              onChange={(v) => update({ notify_80: v })}
              ariaLabel="Notify at 80%"
            />
          </Row>
          <Row>
            <div className="flex flex-col flex-1">
              <span className="text-[12px] text-fg-secondary">Notify at 95%</span>
              <span className="text-[10.5px] text-fg-quaternary mt-0.5">
                Desktop alert when any quota reaches 95%
              </span>
            </div>
            <Toggle
              checked={settings.notify_95}
              onChange={(v) => update({ notify_95: v })}
              ariaLabel="Notify at 95%"
            />
          </Row>
        </Group>

        {/* ABOUT */}
        <Group label="About">
          <Row>
            <span className="text-[12px] text-fg-secondary flex-1">Headroom v0.1.0</span>
            <span className="text-[10.5px] text-fg-quaternary">Check for updates</span>
          </Row>
        </Group>
      </div>
    </div>
  );
}
