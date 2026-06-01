import { useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { ClaudePasteForm } from './components/onboarding/ClaudePasteForm';
import { CopilotPasteForm } from './components/onboarding/CopilotPasteForm';
import { ServiceAuthCard } from './components/onboarding/ServiceAuthCard';
import { ClaudeIcon, GitHubIcon } from './components/onboarding/icons';
import { Button } from './components/ui/Button';
import { startClaudeSignin } from './lib/ipc';
import { useFitWindowHeight } from './lib/useFitWindow';

// Full-window auth-method picker.
// Claude has a one-click sign-in (embedded webview) with a paste-session-key
// fallback under "Advanced". Copilot is a one-step token paste — the
// copilot_internal/user endpoint reads quota from any GitHub token.
export default function OnboardingFlow() {
  const bodyRef = useRef<HTMLDivElement>(null);
  useFitWindowHeight(bodyRef, 480);

  function finish() {
    // Nudge an immediate poll so freshly-saved credentials are picked up;
    // the 30s loop would catch them regardless.
    invoke('refresh_all').catch(() => {});
    getCurrentWindow().hide();
  }

  return (
    <div className="min-h-screen bg-window-opaque text-fg-primary">
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
          onPrimary={() => startClaudeSignin().catch(console.error)}
        >
          <ClaudePasteForm />
        </ServiceAuthCard>

        <ServiceAuthCard icon={<GitHubIcon />} name="GitHub Copilot">
          <CopilotPasteForm />
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
