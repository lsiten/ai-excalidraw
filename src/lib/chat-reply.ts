import type { LocalChatActivity, LocalReplyPart } from './local-chat'

export function splitReplyParts(parts: readonly LocalReplyPart[]) {
  let index = parts.length - 1
  while (index >= 0 && parts[index].type === 'status') index -= 1
  const last = parts[index]
  return last?.type === 'chunk'
    ? { answer: last.text, progress: parts.filter((_, i) => i !== index) }
    : { answer: '', progress: parts }
}

export function appendReplyPart(parts: readonly LocalReplyPart[], event: LocalReplyPart): LocalReplyPart[] {
  const last = parts.at(-1)
  switch (event.type) {
    case 'chunk':
    case 'thinking':
      if (!event.text) return [...parts]
      if (last?.type === event.type) return [...parts.slice(0, -1), { ...event, text: last.text + event.text }]
      break
    case 'tool': {
      const previous = parts.findIndex((part) => part.type === 'tool' && part.id === event.id)
      if (previous >= 0) return parts.map((part, index) => index === previous ? { ...part, ...event } : part)
      break
    }
    case 'status':
      if (last?.type === 'status' && last.message === event.message && last.level === event.level) return [...parts]
      break
  }
  return [...parts, event]
}

export function failChatReply<T extends { readonly content: string; readonly activities: readonly LocalChatActivity[]; readonly parts?: readonly LocalReplyPart[] }>(reply: T, message: string) {
  const activities: LocalChatActivity[] = reply.activities.map((activity) =>
    activity.type === 'tool' && activity.status === 'running'
      ? { ...activity, status: 'failed', output: [activity.output, message].filter(Boolean).join('\n\n') }
      : activity,
  )
  const failure = { type: 'status' as const, message, level: 'error' as const }
  const parts = reply.parts?.map((part): LocalReplyPart => part.type === 'tool' && part.status === 'running'
    ? { ...part, status: 'failed', output: [part.output, message].filter(Boolean).join('\n\n') }
    : part)
  return { ...reply, activities: [...activities, failure], ...(parts && { parts: appendReplyPart(parts, failure) }) }
}
