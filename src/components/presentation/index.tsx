import { useSyncExternalStore } from 'react'
import { presentations } from '@/lib/presentation/controller'
import { DeckDialog } from './deck-dialog'
import { PresentationPlayer } from './player'
export function PresentationHost() {
  const state = useSyncExternalStore(presentations.subscribe, presentations.getSnapshot)
  switch (state.phase) {
    case 'closed':
      return null
    case 'setup':
      return <DeckDialog deck={state.deck} loading={state.loading} error={state.error} />
    case 'playing':
      return <PresentationPlayer state={state} />
  }
}
