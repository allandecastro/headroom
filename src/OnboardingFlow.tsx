import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { ClaudePasteForm } from './components/onboarding/ClaudePasteForm';
import { CopilotPasteForm } from './components/onboarding/CopilotPasteForm';
import { ServiceAuthCard } from './components/onboarding/ServiceAuthCard';
import { ClaudeIcon, GitHubIcon } from './components/onboarding/icons';
import { Button } from './components/ui/Button';

// Full-window auth-method picker. See docs/mockups/02-onboarding.html.
// Phase 1 wires only the paste path; the magic sign-in buttons are stubs.
export default function OnboardingFlow() {
  function finish(startWatching: boolean) {
    // Nudge an immediate poll so freshly-saved credentials are picked up;
    // the 30s loop would catch them regardless.
    if (startWatching) {
      invoke('refresh_all').catch(() => {});
    }
    getCurrentWindow().hide();
  }

  return (
    <div className="h-full flex flex-col overflow-hidden rounded-[10px] border-hairline border-default bg-window-opaque text-fg-primary">
      <div
        data-tauri-drag-region
        className="flex items-center gap-1.5 px-3 py-2.5 border-b border-hairline border-default"
      >
        <span className="w-[11px] h-[11px] rounded-full bg-black/[0.18] dark:bg-white/[0.18]" />
        <span className="w-[11px] h-[11px] rounded-full bg-black/[0.18] dark:bg-white/[0.18]" />
        <span className="w-[11px] h-[11px] rounded-full bg-black/[0.18] dark:bg-white/[0.18]" />
      </div>

      <div className="flex-1 overflow-y-auto px-6 pt-6 pb-[18px]">
        <h1 className="mb-1 text-[17px] font-medium tracking-[-0.01em]">Connect your services</h1>
        <p className="mb-[18px] text-[12px] leading-normal text-fg-tertiary">
          Both services can authenticate the easy way or with a token. Headroom stores credentials
          in your OS keychain — nothing leaves your computer.
        </p>

        <ServiceAuthCard
          icon={<ClaudeIcon />}
          name="Claude Code"
          primaryLabel="Sign in with Claude"
          advancedLabel="Paste a session key"
        >
          <ClaudePasteForm />
        </ServiceAuthCard>

        <ServiceAuthCard
          icon={<GitHubIcon />}
          name="GitHub Copilot"
          primaryLabel="Sign in with GitHub"
          advancedLabel="Paste a personal access token"
        >
          <CopilotPasteForm />
        </ServiceAuthCard>

        <div className="flex items-center justify-between mt-1 pt-3.5 border-t border-hairline border-default">
          <span className="text-[10px] text-fg-quaternary">1 of 1</span>
          <div className="flex gap-2">
            <Button onClick={() => finish(false)}>Skip</Button>
            <Button variant="primary" onClick={() => finish(true)}>
              Start watching →
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
