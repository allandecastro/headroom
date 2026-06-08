import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { ClaudePasteForm } from './components/onboarding/ClaudePasteForm';
import { CopilotAuthCard } from './components/onboarding/CopilotAuthCard';
import { ServiceAuthCard } from './components/onboarding/ServiceAuthCard';
import { ClaudeIcon, CodexIcon } from './components/onboarding/icons';
import { Button } from './components/ui/Button';
import { startClaudeSignin } from './lib/ipc';
import { useFitWindowHeight } from './lib/useFitWindow';

type ClaudePhase = 'idle' | 'pending' | 'error';

// Full-window auth-method picker.
// Claude has a one-click sign-in (embedded webview) with a paste-session-key
// fallback under "Advanced". Copilot is a one-step token paste — the
// copilot_internal/user endpoint reads quota from any GitHub token.
export default function OnboardingFlow() {
  const bodyRef = useRef<HTMLDivElement>(null);
  useFitWindowHeight(bodyRef, 480);

  const [claudePhase, setClaudePhase] = useState<ClaudePhase>('idle');
  const [claudeError, setClaudeError] = useState('');

  useEffect(() => {
    const unlistenDone = listen('claude-signed-in', () => {
      setClaudePhase('idle');
      setClaudeError('');
    });
    const unlistenErr = listen<string>('claude-signin-error', (e) => {
      setClaudeError(e.payload);
      setClaudePhase('error');
    });
    return () => {
      unlistenDone.then((u) => u());
      unlistenErr.then((u) => u());
    };
  }, []);

  function signInClaude() {
    setClaudePhase('pending');
    setClaudeError('');
    startClaudeSignin().catch((e) => {
      setClaudeError(e instanceof Error ? e.message : String(e));
      setClaudePhase('error');
    });
  }

  function finish() {
    // Nudge an immediate poll so freshly-saved credentials are picked up;
    // the 30s loop would catch them regardless.
    invoke('refresh_all').catch(() => {});
    getCurrentWindow().hide();
  }

  return (
    <div className="h-screen overflow-y-auto bg-window-opaque text-fg-primary">
      <div ref={bodyRef} className="px-6 pt-6 pb-5">
        <h1 className="mb-1 text-[17px] font-medium tracking-[-0.01em]">Connect your services</h1>
        <p className="mb-[18px] text-[12px] leading-normal text-fg-tertiary">
          Headroom stores credentials in your OS keychain — nothing leaves your computer.
        </p>

        <ServiceAuthCard
          icon={<ClaudeIcon />}
          name="Claude"
          primaryLabel="Sign in with Claude"
          advancedLabel="Paste a session key"
          onPrimary={signInClaude}
          extra={
            claudePhase !== 'idle' && (
              <div className="mb-2 rounded-[6px] border-hairline border-default bg-secondary px-3 py-2.5 text-[12px] leading-normal">
                {claudePhase === 'pending' && (
                  <p className="text-fg-secondary">
                    Complete sign-in in the Claude window — this card updates once connected.
                  </p>
                )}
                {claudePhase === 'error' && (
                  <p className="text-state-crit-text dark:text-state-crit-text-dark">
                    Sign-in failed: {claudeError || 'unknown error'}. Try again or paste a session
                    key below.
                  </p>
                )}
              </div>
            )
          }
        >
          <ClaudePasteForm />
        </ServiceAuthCard>

        <CopilotAuthCard />

        <ServiceAuthCard icon={<CodexIcon />} name="Codex">
          <div className="text-[12px] leading-normal text-fg-tertiary">
            <p className="mb-2">
              No sign-in needed. Headroom reads Codex usage from{' '}
              <code className="rounded bg-black/[0.06] px-1 dark:bg-white/[0.08]">~/.codex</code> —
              and, when you’re signed in to ChatGPT in Codex, queries Codex’s own usage endpoint
              with the token already stored there. Nothing else leaves your computer.
            </p>
            <p className="text-[11px] text-fg-quaternary">
              Run an interactive <code className="font-mono">codex</code> session to record your
              5-hour and weekly limits. Set{' '}
              <code className="rounded bg-black/[0.06] px-1 dark:bg-white/[0.08]">CODEX_HOME</code>{' '}
              if your install lives elsewhere.
            </p>
          </div>
        </ServiceAuthCard>

        <div className="flex justify-end pt-4">
          <Button variant="primary" onClick={finish}>
            Start watching →
          </Button>
        </div>
      </div>
    </div>
  );
}
