// Typed wrappers around the Tauri commands in src-tauri/src/lib.rs.
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export interface Point {
  x: number
  y: number
}

export interface Polygon {
  outer: Point[]
  holes: Point[][]
}

/** An imported DXF/SVG file. Geometry is in mm. */
export interface CardInfo {
  id: number
  name: string
  shapes: Polygon[]
  width: number
  height: number
  warnings: string[]
}

export type Strategy = 'fast' | 'balanced' | 'high_quality'

export interface NestJob {
  items: { id: number; quantity: number; rotations: number; mirror: boolean }[]
  sheet_width: number
  sheet_height: number
  sheets: number | null
  spacing: number
  margin: number
  strategy: Strategy
  time_ms: number
  seed: number
}

export interface Sheet {
  id: number
  width: number
  height: number
  parts: { card: number; polygon: Polygon }[]
}

export interface NestOutcome {
  sheets: Sheet[]
  placed: number
  required_placed: number
  required_total: number
  utilization: number
  runtime_ms: number
  unplaced: { card: number; name: string; reason: string; count: number }[]
}

export interface Progress {
  elapsed_ms: number
  budget_ms: number
}

export const importPart = (name: string, content: string) =>
  invoke<CardInfo>('import_part', { name, content })

export const removePart = (id: number) => invoke<void>('remove_part', { id })

export const nest = (job: NestJob) => invoke<NestOutcome>('nest', { job })

export const exportLayout = (
  path: string,
  format: 'svg' | 'dxf',
  kerf: number,
  leadIn: number
) => invoke<void>('export_layout', { path, format, kerf, leadIn })

export const onProgress = (cb: (p: Progress) => void): Promise<UnlistenFn> =>
  listen<Progress>('nest-progress', e => cb(e.payload))
