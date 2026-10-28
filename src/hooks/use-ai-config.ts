import { useEffect, useState } from 'react'
import { AI_CONFIG_EVENT, AI_CONFIG_KEY, getAIConfig } from '@/lib/ai-config'

export function useAIConfig() {
  const [config, setConfig] = useState(getAIConfig)
  useEffect(() => {
    const refresh = () => setConfig(getAIConfig())
    const onStorage = (event: StorageEvent) => {
      if (event.key === AI_CONFIG_KEY || event.key === null) refresh()
    }
    window.addEventListener(AI_CONFIG_EVENT, refresh)
    window.addEventListener('storage', onStorage)
    return () => {
      window.removeEventListener(AI_CONFIG_EVENT, refresh)
      window.removeEventListener('storage', onStorage)
    }
  }, [])
  return config
}
