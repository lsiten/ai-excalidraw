import { useCallback, useEffect, useRef, useState, type RefObject } from 'react'
import { isConfigValid, getAIConfig, saveAIConfig } from '@/lib/ai'
import { executeDrawRun } from '@/lib/agent/run-execution'
import { loadRuns } from '@/lib/agent/store'
import type { ExcalidrawWrapperRef, ElementSummary } from './wrapper'
import type { useChatHistory } from './use-chat-history'
import { useLocale } from '@/hooks/use-locale'
import { useToast } from '@/components/ui/toast'
import { onStorageError } from '@/lib/storage'
import { useAIConfig } from '@/hooks/use-ai-config'
import type { AIProvider } from '@/lib/ai-config'
import type { AIProjectContext } from '@/lib/local-chat'

type ChatHistoryApi = ReturnType<typeof useChatHistory>

interface UseAiDrawOptions {
  excalidrawRef: RefObject<ExcalidrawWrapperRef | null>
  chatHistory: ChatHistoryApi
  selectedElements: ElementSummary[]
  projectContext?: AIProjectContext
}

export function useAiDraw({
  excalidrawRef,
  chatHistory,
  selectedElements,
  projectContext,
}: UseAiDrawOptions) {
  const { t } = useLocale()
  const { showToast } = useToast()
  const [isLoading, setIsLoading] = useState(false)
  const config = useAIConfig()
  const abortControllerRef = useRef<AbortController | null>(null)
  const requestSessionRef = useRef<string | null>(null)

  const {
    currentSession,
    currentSessionId,
    isLoaded,
    createSession,
    addMessage,
    updateMessage,
  } = chatHistory

  useEffect(() => {
    if (requestSessionRef.current && currentSessionId !== requestSessionRef.current) {
      abortControllerRef.current?.abort()
    }
  }, [currentSessionId])

  useEffect(() => {
    return onStorageError(() => {
      showToast(t.storageFull, 4000)
    })
  }, [showToast, t.storageFull])

  // 同步画布与当前会话
  useEffect(() => {
    if (!isLoaded || !currentSessionId) return

    const syncCanvas = () => {
      if (!excalidrawRef.current?.isReady()) return false
      const canvasSessionId = excalidrawRef.current.getCurrentSessionId()
      if (canvasSessionId !== currentSessionId) {
        excalidrawRef.current.switchToSession(
          currentSessionId,
          currentSession?.useIndependentCanvas ?? false
        )
      }
      return true
    }

    if (syncCanvas()) return

    const interval = setInterval(() => {
      if (syncCanvas()) clearInterval(interval)
    }, 100)

    return () => clearInterval(interval)
  }, [
    isLoaded,
    currentSessionId,
    currentSession?.useIndependentCanvas,
    excalidrawRef,
  ])

  useEffect(() => () => abortControllerRef.current?.abort(), [])

  useEffect(() => {
    void loadRuns(projectContext?.projectId).catch(error => {
      showToast(error instanceof Error ? error.message : '运行记录读取失败', 3000)
    })
  }, [projectContext?.projectId, showToast])

  const setProvider = useCallback((provider: AIProvider) => {
    if (!saveAIConfig({ ...getAIConfig(), provider })) showToast(t.saveConfigFailed, 3000)
  }, [showToast, t.saveConfigFailed])

  const abort = useCallback(() => {
    abortControllerRef.current?.abort()
  }, [])

  const send = useCallback(
    async (rawInput: string, parentRunId?: string) => {
      if (!rawInput.trim() || isLoading || abortControllerRef.current) return false

      const requestConfig = getAIConfig()
      const requestSelection = excalidrawRef.current?.getSelectedElementsSummary() ?? selectedElements
      if (!isConfigValid(requestConfig)) {
        showToast(t.configRequiredAlert, 3000)
        return false
      }

      const userMessage = rawInput.trim()
      setIsLoading(true)

      let sessionId = currentSessionId
      let historyBefore = currentSession?.messages ?? []
      if (!sessionId) {
        sessionId = createSession()
        historyBefore = []
        excalidrawRef.current?.switchToSession(sessionId, true)
      }
      requestSessionRef.current = sessionId

      addMessage(sessionId, 'user', userMessage)
      const assistantMessageId = addMessage(sessionId, 'assistant', '')

      const abortController = new AbortController()
      abortControllerRef.current = abortController
      try {
        const canvas = excalidrawRef.current
        if (!canvas) throw new Error('画布尚未就绪')
        await executeDrawRun({ conversationId: sessionId, parentRunId, canvas, signal: abortController.signal,
          request: { userMessage, config: requestConfig, projectContext, onChunk: () => undefined,
            selectedElements: requestSelection, history: historyBefore.map(m => ({ role: m.role, content: m.content })),
            canvasBounds: canvas.getCanvasBounds(),
          },
          onReply: reply => updateMessage(sessionId, assistantMessageId, reply.content, { ...reply, source: requestConfig.provider }),
          onGenerated: ids => { canvas.scrollToElements([...ids]); showToast(t.elementsGenerated, 2000) },
        })
      } catch (error) {
        const message = error instanceof Error ? error.message : '运行失败'
        updateMessage(sessionId, assistantMessageId, message, { source: requestConfig.provider,
          activities: [{ type: 'status', level: 'error', message }],
        })
        showToast(t.generateFailed, 2000)
      } finally {
        abortControllerRef.current = null
        requestSessionRef.current = null
        setIsLoading(false)
      }

      return true
    },
    [
      isLoading,
      currentSessionId,
      currentSession?.messages,
      createSession,
      addMessage,
      updateMessage,
      selectedElements,
      excalidrawRef,
      showToast,
      t,
      projectContext,
    ]
  )

  return {
    isLoading,
    provider: config.provider,
    setProvider,
    send,
    abort,
  }
}
