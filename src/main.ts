import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';

const target = document.getElementById('app');
if (!target) throw new Error('#app not found');

async function boot(el: HTMLElement): Promise<void> {
  // SPIKE-002 harness: launched with TOME_SPIKE_002=1 the app runs the reader
  // bridge benchmark instead of the UI, reports to stdout, and exits. Dead
  // code in a normal launch; remove with the harness when S1-13 lands.
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    if (await invoke<boolean>('spike002_mode')) {
      const { runSpike002 } = await import('./spike/spike002');
      await runSpike002(el);
      return;
    }
  } catch {
    // Not running under Tauri (plain vite dev in a browser): fall through.
  }
  mount(App, { target: el });
}

void boot(target);
