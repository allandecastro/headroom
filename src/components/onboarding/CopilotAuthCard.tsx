import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { open as openUrl } from '@tauri-apps/plugin-shell';
import { startCopilotSignin } from '../../lib/ipc';
import type { CopilotSigninStart } from '../../lib/ipc';
import { GitHubIcon } from './icons';
import { ServiceAuthCard } from './ServiceAuthCard';
import { CopilotPasteForm } from './CopilotPasteForm';

type Phase = 'idle' | 'pending' | 'done' | 'error';

// GitHub Copilot onboarding: primary "Sign in with GitHub" device flow, with
// the manual token paste kept under "Advanced". The device flow returns a code
// the user enters at github.com/login/device; the backend polls and emits
// `copilot-signed-in` / `copilot-signin-error` when it resolves.
export function CopilotAuthCard() {
  const [phase, setPhase] = useState<Phase>('idle');
  const [code, setCode] = useState<CopilotSigninStart | null>(null);
  const [error, setError] = useState('');
  const [copied, setCopied] = useState(false);

  async function copyCode() {
    if (!code) return;
    try {
      await navigator.clipboard.writeText(code.user_code);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch (e) {
      console.error(e);
    }
  }

  useEffect(() => {
    const unlistenDone = listen('copilot-signed-in', () => {
      setPhase('done');
    });
    const unlistenErr = listen<string>('copilot-signin-error', (e) => {
      setError(e.payload);
      setPhase('error');
    });
    return () => {
      unlistenDone.then((u) => u());
      unlistenErr.then((u) => u());
    };
  }, []);

  async function signIn() {
    setPhase('pending');
    setError('');
    try {
      const info = await startCopilotSignin();
      setCode(info);
      openUrl(info.verification_uri).catch(console.error);
    } catch (e) {
      setError(String(e));
      setPhase('error');
    }
  }

  return (
    <ServiceAuthCard
      icon={<GitHubIcon />}
      name="GitHub Copilot"
      primaryLabel="Sign in with GitHub"
      advancedLabel="Paste a token"
      onPrimary={signIn}
      extra={
        phase === 'idle' ? null : (
          <div className="mb-2 rounded-[6px] border-hairline border-default bg-secondary px-3 py-2.5 text-[12px] leading-normal">
            {phase === 'pending' && code && (
              <>
                <p className="mb-1.5 text-fg-secondary">
                  Enter this code at{' '}
                  <button
                    type="button"
                    onClick={() => openUrl(code.verification_uri).catch(console.error)}
                    className="underline hover:text-fg-primary"
                  >
                    github.com/login/device
                  </button>
                  :
                </p>
                <div className="flex items-center justify-center gap-2">
                  <span className="font-mono text-[18px] font-semibold tracking-[0.18em] text-fg-primary">
                    {code.user_code}
                  </span>
                  <button
                    type="button"
                    onClick={copyCode}
                    aria-label="Copy code"
                    className="rounded-[5px] border-hairline border-default px-2 py-1 text-[10px] text-fg-secondary hover:bg-secondary"
                  >
                    {copied ? 'Copied' : 'Copy'}
                  </button>
                </div>
                <p className="mt-1.5 text-[11px] text-fg-tertiary">Waiting for authorization…</p>
              </>
            )}
            {phase === 'done' && (
              <p className="text-state-ok-text dark:text-state-ok-text-dark">
                ✓ Connected — GitHub Copilot is now linked.
              </p>
            )}
            {phase === 'error' && (
              <p className="text-state-crit-text dark:text-state-crit-text-dark">
                Sign-in failed: {error || 'unknown error'}. Try again or paste a token below.
              </p>
            )}
          </div>
        )
      }
    >
      <CopilotPasteForm />
    </ServiceAuthCard>
  );
}
