<!--
  Pro-tier sync-status pill. Renders only when the daemon answers
  the `nodespace.pro.v1.CloudSyncService` probe; community mode
  hides it entirely.

  The visual contract: pill color follows the orchestrator's state
  (`SyncStatusEvent.state`):
    grey       — DISCONNECTED / UNSPECIFIED
    amber      — CONNECTING / AUTHENTICATING / SYNCING
    blue       — AUTH_REQUIRED (action needed)
    green      — CONNECTED
    red        — ERROR

  Click is reserved for the sign-in dialog (chunk 4) — left inert
  here so this commit stays scoped.
-->

<script lang="ts">
  import { proSync, type SyncState } from '$lib/stores/pro-sync.svelte';

  const labels: Record<SyncState, string> = {
    'unspecified': 'Sync',
    'disconnected': 'Sign in',
    'connecting': 'Connecting…',
    'authenticating': 'Signing in…',
    'auth-required': 'Sign in required',
    'syncing': 'Syncing…',
    'connected': 'Synced',
    'error': 'Sync error',
  };

  const tones: Record<SyncState, string> = {
    'unspecified': 'grey',
    'disconnected': 'grey',
    'connecting': 'amber',
    'authenticating': 'amber',
    'auth-required': 'blue',
    'syncing': 'amber',
    'connected': 'green',
    'error': 'red',
  };
</script>

{#if proSync.isPro}
  <button
    class="pro-sync-pill"
    data-tone={tones[proSync.state]}
    title={proSync.detail || labels[proSync.state]}
    type="button"
    aria-label="NodeSpace Pro sync status: {labels[proSync.state]}"
  >
    <span class="dot" aria-hidden="true"></span>
    <span class="label">{labels[proSync.state]}</span>
  </button>
{/if}

<style>
  .pro-sync-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border-radius: 999px;
    border: 1px solid var(--border-color, #d1d5db);
    background: var(--surface-1, #f9fafb);
    color: var(--text-primary, #1f2937);
    font-size: 12px;
    font-weight: 500;
    line-height: 1;
    cursor: pointer;
  }

  .pro-sync-pill:hover {
    background: var(--surface-2, #f3f4f6);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #9ca3af;
  }

  .pro-sync-pill[data-tone='amber'] .dot {
    background: #f59e0b;
    box-shadow: 0 0 0 2px rgba(245, 158, 11, 0.18);
  }
  .pro-sync-pill[data-tone='blue'] .dot {
    background: #2563eb;
    box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.18);
  }
  .pro-sync-pill[data-tone='green'] .dot {
    background: #16a34a;
    box-shadow: 0 0 0 2px rgba(22, 163, 74, 0.18);
  }
  .pro-sync-pill[data-tone='red'] .dot {
    background: #dc2626;
    box-shadow: 0 0 0 2px rgba(220, 38, 38, 0.18);
  }

  @media (prefers-color-scheme: dark) {
    .pro-sync-pill {
      background: var(--surface-1, #1f2937);
      border-color: var(--border-color, #374151);
      color: var(--text-primary, #e5e7eb);
    }
    .pro-sync-pill:hover {
      background: var(--surface-2, #374151);
    }
  }
</style>
