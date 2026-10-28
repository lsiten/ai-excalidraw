import { z } from 'zod'

export const aiProviderSchema = z.enum(['api', 'codex', 'claude', 'cursor', 'pi'])
export type AIProvider = z.infer<typeof aiProviderSchema>

const configSchema = z.object({
  provider: aiProviderSchema.default('api'),
  apiKey: z.string().default(''),
  baseURL: z.string().default(''),
  model: z.string().default('gpt-4o'),
})

export type AIConfig = z.infer<typeof configSchema>
export const AI_CONFIG_KEY = 'ai-excalidraw-config'
export const AI_CONFIG_EVENT = 'ai-excalidraw-config-changed'

export function getAIConfig(): AIConfig {
  const env = {
    apiKey: import.meta.env.VITE_AI_API_KEY || '',
    baseURL: import.meta.env.VITE_AI_BASE_URL || '',
    model: import.meta.env.VITE_AI_MODEL || 'gpt-4o',
  }
  let stored: unknown = {}
  try {
    stored = JSON.parse(localStorage.getItem(AI_CONFIG_KEY) || '{}')
  } catch (error) {
    if (!(error instanceof Error)) throw error
  }
  const parsed = configSchema.partial().safeParse(stored)
  const config = configSchema.parse({ ...env, ...(parsed.success ? parsed.data : {}) })
  return {
    provider: config.provider,
    apiKey: env.apiKey && env.baseURL ? env.apiKey : config.apiKey || env.apiKey,
    baseURL: env.apiKey && env.baseURL ? env.baseURL : config.baseURL || env.baseURL,
    model: env.apiKey && env.baseURL ? env.model : config.model || env.model,
  }
}

export function saveAIConfig(config: AIConfig): boolean {
  try {
    localStorage.setItem(AI_CONFIG_KEY, JSON.stringify(config))
    window.dispatchEvent(new Event(AI_CONFIG_EVENT))
    return true
  } catch (error) {
    if (!(error instanceof Error)) throw error
    console.warn('Failed to save AI config', error)
    return false
  }
}

export function isConfigValid(config: AIConfig): boolean {
  return config.provider !== 'api' || !!(config.apiKey.trim() && config.baseURL.trim() && config.model.trim())
}
