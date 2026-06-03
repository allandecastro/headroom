import { useCallback, useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { open as openUrl } from '@tauri-apps/plugin-shell';
import { startCopilotSignin, listCopilotAccounts } from '../../lib/ipc';
import type { CopilotSigninStart, CopilotAccount } from '../../lib/ipc';
import { GitHubIcon } from './icons';
import { ServiceAuthCard } from './ServiceAuthCard';
import { CopilotPasteForm } from './CopilotPasteForm';

type Phase = 'idle' | 'pending' | 'error';

// Pin the onboarding window above the browser while the user reads the code —
// it's skipTaskbar, so otherwise it slips behind the browser and can't be found.
function pinWindow(on: boolean) {
  getCurrentWindow().setAlwaysOnTop(on).catch(console.error);
}

// GitHub Copilot onboarding: primary "Sign in with GitHub" device flow, with
// the manual token paste kept under "Advanced". The device flow returns a code
// the user enters at github.com/login/device; the backend polls and emits
// `copilot-signed-in` / `copilot-signin-error` when it resolves.
export function CopilotAuthCard() {
  const [phase, setPhase] = useState<Phase>('idle');
  const [code, setCode] = useState<CopilotSigninStart | null>(null);
  const [error, setError] = useState('');
  const [copied, setCopied] = useState(false);
  const [accounts, setAccounts] = useState<CopilotAccount[]>([]);

  const loadAccounts = useCallback(() => {
    listCopilotAccounts().then(setAccounts).catch(console.error);
  }, []);

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
    loadAccounts();
    const unlistenDone = listen('copilot-signed-in', () => {
      // Reset to idle so the button is ready for the next account; the connected
      // list (refreshed below) is the persistent "linked" indicator.
      setPhase('idle');
      setCode(null);
      loadAccounts();
      pinWindow(false);
    });
    const unlistenErr = listen<string>('copilot-signin-error', (e) => {
      setError(e.payload);
      setPhase('error');
      pinWindow(false);
    });
    // The paste path doesn't emit copilot-signed-in; refresh on any snapshot.
    const unlistenTokens = listen('tokens-updated', loadAccounts);
    return () => {
      unlistenDone.then((u) => u());
      unlistenErr.then((u) => u());
      unlistenTokens.then((u) => u());
      pinWindow(false);
    };
  }, [loadAccounts]);

  async function signIn() {
    setPhase('pending');
    setError('');
    pinWindow(true);
    try {
      const info = await startCopilotSignin();
      setCode(info);
      openUrl(info.verification_uri).catch(console.error);
    } catch (e) {
      setError(String(e));
      setPhase('error');
      pinWindow(false);
    }
  }

  return (
    <ServiceAuthCard
      icon={<GitHubIcon />}
      name="GitHub Copilot"
      primaryLabel={accounts.length > 0 ? 'Add another GitHub account' : 'Sign in with GitHub'}
      advancedLabel="Paste a token"
      onPrimary={signIn}
      extra={
        <>
          {accounts.length > 0 && (
            <div className="mb-2 rounded-[6px] border-hairline border-default bg-secondary px-3 py-2.5 text-[12px] leading-normal">
              <p className="mb-1 text-fg-tertiary">
                Connected {accounts.length === 1 ? 'account' : 'accounts'}:
              </p>
              <ul className="space-y-0.5">
                {accounts.map((a) => (
                  <li
                    key={a.id}
                    className="flex items-center gap-1.5 text-state-ok-text dark:text-state-ok-text-dark"
                  >
                    <span aria-hidden>✓</span>
                    <span className="text-fg-primary">@{a.login}</span>
                    {a.label !== a.login && <span className="text-fg-tertiary">({a.label})</span>}
                  </li>
                ))}
              </ul>
              <p className="mt-1.5 text-[11px] text-fg-tertiary">
                To add a <em>different</em> account, sign out of github.com first (or paste its
                token below).
              </p>
            </div>
          )}
          {phase !== 'idle' && (
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
              {phase === 'error' && (
                <p className="text-state-crit-text dark:text-state-crit-text-dark">
                  Sign-in failed: {error || 'unknown error'}. Try again or paste a token below.
                </p>
              )}
            </div>
          )}
        </>
      }
    >
      <CopilotPasteForm />
    </ServiceAuthCard>
  );
}
