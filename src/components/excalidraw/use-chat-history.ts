import { useState, useEffect, useCallback, useRef } from 'react'
import { createDebouncedWriter, readStorage } from '@/lib/storage'
import type { LocalChatActivity, LocalReplyPart } from '@/lib/local-chat'
import type { AIProvider } from '@/lib/ai-config'

export interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: number
  thinking?: string
  activities?: LocalChatActivity[]
  parts?: LocalReplyPart[]
  source?: AIProvider
  runId?: string
}

export interface ChatSession {
  id: string
  title: string
  messages: ChatMessage[]
  createdAt: number
  updatedAt: number
  /** 是否使用独立画布（新会话为 true，老会话为 undefined/false） */
  useIndependentCanvas?: boolean
}

const STORAGE_KEY = 'excalidraw-ai-chat-history'
const MAX_SESSIONS = 50

function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 11)}`
}

/**
 * 对话历史 Hook
 */
export function useChatHistory(fileHistory?: { readonly initial: ChatSession[]; readonly onChange: (sessions: ChatSession[]) => void }) {
  const [sessions, setSessions] = useState<ChatSession[]>(() =>
    fileHistory?.initial ?? readStorage<ChatSession[]>(STORAGE_KEY, [])
  )
  const [currentSessionId, setCurrentSessionId] = useState<string | null>(() => {
    const loaded = fileHistory?.initial ?? readStorage<ChatSession[]>(STORAGE_KEY, [])
    return loaded[0]?.id ?? null
  })
  const writerRef = useRef(createDebouncedWriter(STORAGE_KEY, 500))
  const sessionsRef = useRef<ChatSession[]>(sessions)
  const isLoaded = true

  useEffect(() => {
    const writer = writerRef.current
    const flush = () => writer.flush()
    window.addEventListener('beforeunload', flush)
    return () => {
      flush()
      window.removeEventListener('beforeunload', flush)
    }
  }, [])

  useEffect(() => {
    sessionsRef.current = sessions
    const trimmed = sessions.slice(0, MAX_SESSIONS)
    if (fileHistory) fileHistory.onChange(trimmed)
    else writerRef.current.write(trimmed)
  }, [sessions, fileHistory])

  const currentSession = sessions.find((s) => s.id === currentSessionId) || null

  const createSession = useCallback((title?: string): string => {
    const newSession: ChatSession = {
      id: generateId(),
      title:
        title ||
        `新对话 ${new Date().toLocaleString('zh-CN', {
          month: 'short',
          day: 'numeric',
          hour: '2-digit',
          minute: '2-digit',
        })}`,
      messages: [],
      createdAt: Date.now(),
      updatedAt: Date.now(),
      useIndependentCanvas: true,
    }
    setSessions((prev) => [newSession, ...prev])
    setCurrentSessionId(newSession.id)
    return newSession.id
  }, [])

  const addMessage = useCallback(
    (sessionId: string, role: 'user' | 'assistant', content: string): string => {
      const messageId = generateId()
      setSessions((prev) =>
        prev.map((session) => {
          if (session.id !== sessionId) return session

          const newMessage: ChatMessage = {
            id: messageId,
            role,
            content,
            timestamp: Date.now(),
          }

          let title = session.title
          if (role === 'user' && session.messages.length === 0) {
            title = content.slice(0, 30) + (content.length > 30 ? '...' : '')
          }

          return {
            ...session,
            title,
            messages: [...session.messages, newMessage],
            updatedAt: Date.now(),
          }
        })
      )
      return messageId
    },
    []
  )

  const updateMessage = useCallback(
    (sessionId: string, messageId: string, content: string, details?: Pick<ChatMessage, 'thinking' | 'activities' | 'source' | 'parts' | 'runId'>) => {
      setSessions((prev) =>
        prev.map((session) => {
          if (session.id !== sessionId) return session
          return {
            ...session,
            messages: session.messages.map((msg) =>
              msg.id === messageId ? { ...msg, ...details, content } : msg
            ),
            updatedAt: Date.now(),
          }
        })
      )
    },
    []
  )

  const deleteSession = useCallback(
    (sessionId: string) => {
      setSessions((prev) => {
        const filtered = prev.filter((s) => s.id !== sessionId)
        if (sessionId === currentSessionId && filtered.length > 0) {
          setCurrentSessionId(filtered[0].id)
        } else if (filtered.length === 0) {
          setCurrentSessionId(null)
        }
        return filtered
      })
    },
    [currentSessionId]
  )

  const clearAllSessions = useCallback(() => {
    setSessions([])
    setCurrentSessionId(null)
  }, [])

  const switchSession = useCallback((sessionId: string) => {
    setCurrentSessionId(sessionId)
  }, [])

  const replaceSessions = useCallback((next: ChatSession[]) => {
    setSessions(next)
    setCurrentSessionId(next[0]?.id ?? null)
  }, [])

  const flush = useCallback(() => {
    if (fileHistory) fileHistory.onChange(sessionsRef.current.slice(0, MAX_SESSIONS))
    else writerRef.current.write(sessionsRef.current.slice(0, MAX_SESSIONS))
    writerRef.current.flush()
  }, [fileHistory])

  return {
    sessions,
    currentSession,
    currentSessionId,
    isLoaded,
    createSession,
    addMessage,
    updateMessage,
    deleteSession,
    clearAllSessions,
    switchSession,
    replaceSessions,
    flush,
  }
}
