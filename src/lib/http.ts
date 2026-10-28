import { isTauri } from '@tauri-apps/api/core'

/** Keep streaming and cancellation while avoiding WebView CORS restrictions. */
export async function appFetch(url: string, init?: RequestInit): Promise<Response> {
  if (isTauri()) {
    const { fetch } = await import('@tauri-apps/plugin-http')
    return fetch(url, { ...init, connectTimeout: 30_000 })
  }
  return globalThis.fetch(url, init)
}
