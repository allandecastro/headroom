import React from 'react';
import ReactDOM from 'react-dom/client';
import { getCurrentWindow } from '@tauri-apps/api/window';
import App from './App';
import OnboardingFlow from './OnboardingFlow';
import SettingsPanel from './SettingsPanel';
import './index.css';

// All windows load this same bundle; render by window label.
function resolveRoot() {
  const label = getCurrentWindow().label;
  if (label === 'onboarding') return OnboardingFlow;
  if (label === 'settings') return SettingsPanel;
  return App;
}

const Root = resolveRoot();

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
