import { appendReplyPart } from '../chat-reply'
import type { LocalReplyPart } from '../local-chat'
import type { AgentRun } from './protocol'

export function replayRun(run: AgentRun): LocalReplyPart[] {
  let parts: LocalReplyPart[] = []
  for (const { event } of run.events) {
    switch (event.type) {
      case 'message': parts = appendReplyPart(parts, { type: 'chunk', text: event.text }); break
      case 'thinking': parts = appendReplyPart(parts, { type: 'thinking', text: event.text }); break
      case 'tool': parts = appendReplyPart(parts, { type: 'tool', id: event.callId, name: event.name, status: event.status, input: event.input ?? undefined, output: event.output ?? undefined }); break
      case 'status': parts = appendReplyPart(parts, { type: 'status', message: event.message, level: event.level === 'warning' ? 'info' : event.level }); break
      case 'state':
        if (event.state === 'failed' || event.state === 'needs_reconciliation' || event.state === 'interrupted' || event.state === 'cancelled') {
          parts = appendReplyPart(parts, { type: 'status', level: 'error', message: event.message ?? event.state })
        }
        break
      case 'session':
      case 'context':
      case 'plan':
      case 'operation':
      case 'artifact':
      case 'usage':
      case 'validation':
      case 'input': break
    }
  }
  return parts
}
