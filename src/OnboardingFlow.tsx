import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { open as openUrl } from '@tauri-apps/plugin-shell';
import { ClaudePasteForm } from './components/onboarding/ClaudePasteForm';
import { CopilotPasteForm } from './components/onboarding/CopilotPasteForm';
import { CopilotSigninPanel } from './components/onboarding/CopilotSigninPanel';
import type { SigninStatus } from './components/onboarding/CopilotSigninPanel';
import { ServiceAuthCard } from './components/onboarding/ServiceAuthCard';
import { ClaudeIcon, GitHubIcon } from './components/onboarding/icons';
import { Button } from './components/ui/Button';
import { setCopilotPlan, startClaudeSignin, startCopilotSignin } from './lib/ipc';
import type { CopilotPlan, DeviceCode } from './lib/ipc';
import { useFitWindowHeight } from './lib/useFitWindow';

// Full-window auth-method picker. See docs/mockups/02-onboarding.html.
// Both services have live sign-in (Claude via embedded webview, Copilot via the
// GitHub OAuth device flow); paste paths remain as the fallback.
export default function OnboardingFlow() {
  const bodyRef = useRef<HTMLDivElement>(null);
  useFitWindowHeight(bodyRef, 480);

  // Copilot device-flow state: null until the user clicks "Sign in with GitHub".
  const [copilotDevice, setCopilotDevice] = useState<DeviceCode | null>(null);
  const [copilotStatus, setCopilotStatus] = useState<SigninStatus>('pending');
  const [copilotError, setCopilotError] = useState<string | undefined>();
  const [copilotPlan, setCopilotPlanState] = useState<CopilotPlan>('pro');

  useEffect(() => {
    const unlistenOk = listen('copilot-signed-in', () => {
      setCopilotStatus('success');
      // Default the plan to "pro" so the source has something to read; the user
      // can change it in the panel before clicking "Start watching".
      setCopilotPlan('pro').catch(console.error);
    });
    const unlistenFail = listen<string>('copilot-signin-failed', (event) => {
      setCopilotStatus('error');
      setCopilotError(event.payload);
    });
    return () => {
      unlistenOk.then((fn) => fn());
      unlistenFail.then((fn) => fn());
    };
  }, []);

  async function startGithubSignin() {
    setCopilotStatus('pending');
    setCopilotError(undefined);
    try {
      const device = await startCopilotSignin();
      setCopilotDevice(device);
      // Auto-open the verification page so the user only has to type the code.
      openUrl(device.verification_uri).catch(console.error);
    } catch (e) {
      setCopilotStatus('error');
      setCopilotError(String(e));
    }
  }

  function changePlan(plan: CopilotPlan) {
    setCopilotPlanState(plan);
    setCopilotPlan(plan).catch(console.error);
  }

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
          Both services can authenticate the easy way or with a token. Headroom stores credentials
          in your OS keychain — nothing leaves your computer.
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

        <ServiceAuthCard
          icon={<GitHubIcon />}
          name="GitHub Copilot"
          primaryLabel="Sign in with GitHub"
          advancedLabel="Paste a personal access token"
          onPrimary={startGithubSignin}
          extra={
            copilotDevice && (
              <CopilotSigninPanel
                device={copilotDevice}
                status={copilotStatus}
                error={copilotError}
                plan={copilotPlan}
                onPlanChange={changePlan}
                onOpen={() => openUrl(copilotDevice.verification_uri).catch(console.error)}
              />
            )
          }
        >
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
