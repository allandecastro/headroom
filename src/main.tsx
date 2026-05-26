import React from 'react';
import ReactDOM from 'react-dom/client';
import { getCurrentWindow } from '@tauri-apps/api/window';
import App from './App';
import OnboardingFlow from './OnboardingFlow';
import './index.css';

// Both windows load this same bundle; render by window label.
const Root = getCurrentWindow().label === 'onboarding' ? OnboardingFlow : App;

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
