import { expect, it } from 'vitest'
import { renderToStaticMarkup } from 'react-dom/server'
import { createElement } from 'react'
import { MessageBubble } from './chat-message'
import { failChatReply } from '@/lib/chat-reply'

it.each([
  { reason: 'Connection lost', content: '' },
  { reason: 'Stopped', content: '' },
  { reason: 'Connection lost', content: 'Partial answer' },
  { reason: 'Stopped', content: 'Partial answer' },
])('shows the API terminal state $reason with content "$content"', ({ reason, content }) => {
  // Given
  const failed = failChatReply({ content, activities: [], source: 'api' as const }, reason)
  // When
  const html = renderToStaticMarkup(createElement(MessageBubble, { message: { id: 'reply', role: 'assistant', timestamp: 1, ...failed } }))
  // Then
  expect(html).toContain(reason)
  expect(html).not.toContain('italic')
  expect(html).not.toContain('animate-')
  if (content) expect(html).toContain(content)
})
