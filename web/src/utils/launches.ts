import type { Launch, LaunchStream } from '@/api/types'
import { localDay } from '@/utils/date'

const FIRM = new Set(['SEC', 'MIN', 'HR', 'Second', 'Minute', 'Hour'])
const DAY_MS = 86_400_000

const HOSTS: Record<string, string> = {
  'youtube.com': 'YouTube',
  'youtu.be': 'YouTube',
  'plus.nasa.gov': 'NASA+',
  'nasa.gov': 'NASA',
  'x.com': 'X',
  'twitter.com': 'X',
  'twitch.tv': 'Twitch',
  'spacex.com': 'SpaceX',
}

export const DAY_LONG = new Intl.DateTimeFormat(undefined, {
  weekday: 'long',
  day: 'numeric',
  month: 'long',
  year: 'numeric',
})

export const DAY_SHORT = new Intl.DateTimeFormat(undefined, {
  weekday: 'short',
  day: 'numeric',
  month: 'short',
})

export const TIME = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' })

export function isFirm(launch: Launch): boolean {
  return Boolean(launch.precision && FIRM.has(launch.precision))
}

export function dayOf(iso: string): string {
  const at = new Date(iso)
  return Number.isNaN(at.getTime()) ? iso.slice(0, 10) : localDay(at)
}

export function daysAway(day: string, from: number): string {
  const [year, month, date] = day.split('-').map(Number)
  if (!year || !month || !date) return ''

  const target = new Date(year, month - 1, date).getTime()
  const here = new Date(from)
  const today = new Date(here.getFullYear(), here.getMonth(), here.getDate()).getTime()

  const days = Math.round((target - today) / DAY_MS)
  if (days === 0) return 'today'
  if (days === 1) return 'tomorrow'
  if (days === -1) return 'yesterday'
  return days > 0 ? `in ${days} days` : `${-days} days ago`
}

export function timeOf(iso: string): string {
  const at = new Date(iso)
  return Number.isNaN(at.getTime()) ? '' : TIME.format(at)
}

export function clockOf(launch: Launch): string {
  return isFirm(launch) ? timeOf(launch.net) : 'TBD'
}

export function windowOf(launch: Launch): string | null {
  if (isFirm(launch) || !launch.window_start || !launch.window_end) return null

  const from = new Date(launch.window_start)
  const to = new Date(launch.window_end)
  if (Number.isNaN(from.getTime()) || Number.isNaN(to.getTime())) return null
  if (to.getTime() - from.getTime() < 60_000) return null

  return `${TIME.format(from)} to ${TIME.format(to)}`
}

export function countdown(iso: string, from: number): string {
  const at = new Date(iso).getTime()
  if (Number.isNaN(at)) return ''

  const ms = at - from
  const away = Math.abs(ms)
  const ago = ms < 0

  if (away < 60_000) return 'right now'
  if (away < 3_600_000) return step(Math.round(away / 60_000), 'min', ago)
  if (away < DAY_MS) return step(Math.round(away / 3_600_000), 'h', ago)

  const days = Math.round(away / DAY_MS)
  if (days === 1) return ago ? 'yesterday' : 'tomorrow'
  if (days < 45) return step(days, 'days', ago)

  const months = Math.round(days / 30.44)
  return step(months, months === 1 ? 'month' : 'months', ago)
}

function step(count: number, unit: string, ago: boolean): string {
  return ago ? `${count} ${unit} ago` : `in ${count} ${unit}`
}

export function statusTone(status: string | null): 'bad' | 'warn' | 'good' | null {
  if (!status) return null

  const said = status.toLowerCase()
  if (said.includes('failure') || said.includes('partial')) return 'bad'
  if (said.includes('hold') || said.includes('scrub')) return 'warn'
  if (said.includes('success')) return 'good'
  if (said.includes('go for launch')) return 'good'
  return null
}

export function launchMark(launch: Launch, from: number): string {
  if (statusTone(launch.status) === 'bad') return 'times-circle'
  if (hasOutcome(launch.status)) return 'check'

  return isImminent(launch, from) ? 'imminent' : 'launch'
}

export function hasOutcome(status: string | null): boolean {
  if (!status) return false

  const said = status.toLowerCase()
  return said.includes('success') || said.includes('failure') || said.includes('partial')
}

export function tminus(iso: string, from: number, within = DAY_MS): string | null {
  const at = new Date(iso).getTime()
  if (Number.isNaN(at)) return null

  const ms = at - from
  if (ms < 0 || ms > within) return null

  const total = Math.floor(ms / 1000)
  const hours = Math.floor(total / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const seconds = total % 60

  const pad = (value: number) => String(value).padStart(2, '0')
  return `T-${pad(hours)}:${pad(minutes)}:${pad(seconds)}`
}

export const IMMINENT_MS = 2 * 3_600_000

export function isImminent(launch: Launch, from: number): boolean {
  if (launch.webcast_live) return true

  const at = new Date(launch.net).getTime()
  if (Number.isNaN(at)) return false

  return at - from <= IMMINENT_MS && at - from > -3_600_000
}

export function summaryOf(launch: Launch): string {
  return [launch.provider, launch.vehicle, launch.orbit]
    .filter((part) => part && part !== 'Unknown' && part !== 'TBD')
    .join(' · ')
}

export function streamLabel(stream: LaunchStream): string {
  try {
    const host = new URL(stream.url).hostname.replace(/^www\./, '')
    return HOSTS[host] ?? host
  } catch {
    return 'Webcast'
  }
}
