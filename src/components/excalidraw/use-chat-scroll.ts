import { useCallback, useEffect, useRef, useState, type UIEvent } from 'react'
import type { ChatSession } from './use-chat-history'

export function useChatScroll(session: ChatSession | null, enabled = true) {
  const containerRef = useRef<HTMLDivElement>(null)
  const following = useRef(true)
  const previousSession = useRef(session?.id)
  const [readingSession, setReadingSession] = useState<string | null>(null)
  const scrollToLatest = useCallback(() => {
    const container = containerRef.current
    if (container) container.scrollTop = container.scrollHeight
    following.current = true
    setReadingSession(null)
  }, [])
  const onScroll = useCallback((event: UIEvent<HTMLDivElement>) => {
    const container = event.currentTarget
    following.current = container.scrollHeight - container.scrollTop - container.clientHeight < 64
    setReadingSession(following.current ? null : session?.id ?? null)
  }, [session?.id])
  useEffect(() => {
    if (!enabled) return
    if (previousSession.current !== session?.id) following.current = true
    previousSession.current = session?.id
    const container = containerRef.current
    if (following.current && container) container.scrollTop = container.scrollHeight
  }, [session?.messages, session?.id, enabled])
  return { containerRef, onScroll, scrollToLatest, showLatest: !!session && readingSession === session.id }
}
