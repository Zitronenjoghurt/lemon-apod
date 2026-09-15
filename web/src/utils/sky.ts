import type { SkyEventKind } from '@/api/types'

export const EVENT_ICONS: Record<SkyEventKind, string> = {
  moon: 'moon-phase',
  season: 'sun',
  shower: 'sparkles',
  eclipse: 'eclipse',
  planet: 'globe',
  conjunction: 'conjunction',
}
