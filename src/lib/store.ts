// App state: imported part cards, job settings, and the last nest result.
import { createStore } from 'solid-js/store'
import type { CardInfo, NestJob, NestOutcome, Progress, Strategy } from './api'

export interface Card {
  info: CardInfo
  quantity: number
  rotations: number
  mirror: boolean
}

export interface Settings {
  sheetWidth: number
  sheetHeight: number
  /** 0 = as many as needed. */
  sheets: number
  spacing: number
  margin: number
  strategy: Strategy
  timeSeconds: number
  kerf: number
  leadIn: number
}

export interface State {
  cards: Card[]
  selected: number | null
  settings: Settings
  result: NestOutcome | null
  progress: Progress | null
  nesting: boolean
  /** Short-lived messages (import errors, export done, ...). */
  notices: { id: number; kind: 'error' | 'info'; text: string }[]
}

export const [state, setState] = createStore<State>({
  cards: [],
  selected: null,
  settings: {
    sheetWidth: 1220,
    sheetHeight: 2440,
    sheets: 0,
    spacing: 2,
    margin: 5,
    strategy: 'balanced',
    timeSeconds: 10,
    kerf: 0,
    leadIn: 0
  },
  result: null,
  progress: null,
  nesting: false,
  notices: []
})

let noticeId = 0
export function notify(kind: 'error' | 'info', text: string) {
  const id = ++noticeId
  setState('notices', n => [...n, { id, kind, text }])
  setTimeout(() => setState('notices', n => n.filter(x => x.id !== id)), 6000)
}

export function jobFromState(): NestJob {
  const s = state.settings
  return {
    items: state.cards.map(c => ({
      id: c.info.id,
      quantity: c.quantity,
      rotations: c.rotations,
      mirror: c.mirror
    })),
    sheet_width: s.sheetWidth,
    sheet_height: s.sheetHeight,
    sheets: s.sheets > 0 ? s.sheets : null,
    spacing: s.spacing,
    margin: s.margin,
    strategy: s.strategy,
    time_ms: Math.round(s.timeSeconds * 1000),
    seed: 1
  }
}

/** A stable, distinct colour per card (golden-angle hue steps). */
export function cardHue(id: number): number {
  return (id * 137.508) % 360
}

export function cardColor(id: number, alpha = 1): [number, number, number, number] {
  const h = cardHue(id) / 360
  const s = 0.55
  const l = 0.62
  const f = (n: number) => {
    const k = (n + h * 12) % 12
    const a = s * Math.min(l, 1 - l)
    return l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1))
  }
  return [f(0), f(8), f(4), alpha]
}
