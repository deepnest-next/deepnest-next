import {
  createEffect,
  createSignal,
  onCleanup,
  onMount,
  Show,
  type Component
} from 'solid-js'
import { NestRenderer, type Scene, type Theme } from '../lib/renderer'
import { cardColor, state } from '../lib/store'

const LIGHT: Theme = {
  background: [0.95, 0.96, 0.97, 1],
  sheet: [1, 1, 1, 1],
  sheetEdge: [0.55, 0.58, 0.62, 1],
  outline: [0.12, 0.14, 0.18, 0.9]
}

const DARK: Theme = {
  background: [0.07, 0.08, 0.1, 1],
  sheet: [0.13, 0.15, 0.18, 1],
  sheetEdge: [0.4, 0.44, 0.5, 1],
  outline: [0.92, 0.94, 0.97, 0.85]
}

/** The nest result (sheets side by side), or a preview row of the imported parts. */
function buildScene(): Scene {
  const result = state.result
  if (result) {
    const shapes: Scene['shapes'] = []
    const sheets: Scene['sheets'] = []
    let x = 0
    for (const sheet of result.sheets) {
      sheets.push({ x, y: 0, w: sheet.width, h: sheet.height })
      for (const p of sheet.parts)
        shapes.push({ polygon: p.polygon, dx: x, dy: 0, fill: cardColor(p.card, 0.85) })
      x += sheet.width + Math.max(sheet.width * 0.06, 20)
    }
    return { sheets, shapes }
  }

  const shapes: Scene['shapes'] = []
  let x = 0
  for (const card of state.cards) {
    let minX = Infinity
    let minY = Infinity
    for (const s of card.info.shapes)
      for (const p of s.outer) {
        minX = Math.min(minX, p.x)
        minY = Math.min(minY, p.y)
      }
    const selected = state.selected === card.info.id
    for (const polygon of card.info.shapes)
      shapes.push({
        polygon,
        dx: x - minX,
        dy: -minY,
        fill: cardColor(card.info.id, selected ? 0.95 : 0.6)
      })
    x += card.info.width + Math.max(card.info.width * 0.15, 10)
  }
  return { sheets: [], shapes }
}

const Viewport: Component = () => {
  let canvas!: HTMLCanvasElement
  let renderer: NestRenderer | undefined
  const [error, setError] = createSignal('')

  const dark = window.matchMedia('(prefers-color-scheme: dark)')
  const theme = () => (dark.matches ? DARK : LIGHT)

  onMount(() => {
    try {
      renderer = new NestRenderer(canvas, theme())
    } catch (e) {
      setError(String(e))
      return
    }
    const r = renderer
    const ro = new ResizeObserver(() => r.resize())
    ro.observe(canvas)
    r.resize()

    const onTheme = () => r.setTheme(theme(), buildScene())
    dark.addEventListener('change', onTheme)

    let drag: { x: number; y: number } | null = null
    const down = (e: PointerEvent) => {
      drag = { x: e.clientX, y: e.clientY }
      canvas.setPointerCapture(e.pointerId)
    }
    const move = (e: PointerEvent) => {
      if (!drag) return
      r.pan(e.clientX - drag.x, e.clientY - drag.y)
      drag = { x: e.clientX, y: e.clientY }
    }
    const up = () => (drag = null)
    const wheel = (e: WheelEvent) => {
      e.preventDefault()
      const rect = canvas.getBoundingClientRect()
      r.zoomAt(e.clientX - rect.left, e.clientY - rect.top, Math.exp(-e.deltaY * 0.0015))
    }
    const dbl = () => r.fit()
    canvas.addEventListener('pointerdown', down)
    canvas.addEventListener('pointermove', move)
    canvas.addEventListener('pointerup', up)
    canvas.addEventListener('pointercancel', up)
    canvas.addEventListener('wheel', wheel, { passive: false })
    canvas.addEventListener('dblclick', dbl)

    onCleanup(() => {
      ro.disconnect()
      dark.removeEventListener('change', onTheme)
      r.dispose()
    })
  })

  // Re-frame when the content changes (new result, parts added/removed); keep the camera
  // when only the selection highlight changes.
  let lastKey = ''
  createEffect(() => {
    const scene = buildScene()
    const key = `${state.result ? 'r' : 'p'}:${state.cards.map(c => c.info.id).join(',')}`
    renderer?.setScene(scene, key !== lastKey)
    lastKey = key
  })

  return (
    <div class="relative h-full w-full">
      <canvas
        ref={el => (canvas = el)}
        class="h-full w-full cursor-grab touch-none active:cursor-grabbing"
      />
      <Show when={error()}>
        <div class="absolute inset-0 grid place-items-center text-sm text-red-600">
          {error()}
        </div>
      </Show>
      <Show when={!error() && state.cards.length === 0}>
        <div class="pointer-events-none absolute inset-0 grid place-items-center text-sm text-slate-400">
          Add DXF or SVG parts to start
        </div>
      </Show>
      <div class="pointer-events-none absolute bottom-2 right-3 text-[11px] text-slate-400">
        drag to pan · wheel to zoom · double-click to fit
      </div>
    </div>
  )
}

export default Viewport
