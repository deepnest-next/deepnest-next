import { save } from '@tauri-apps/plugin-dialog'
import { onCleanup, onMount, Show, type Component } from 'solid-js'
import { exportLayout, nest, onProgress } from '../lib/api'
import { jobFromState, notify, setState, state } from '../lib/store'

async function runNest() {
  setState({ nesting: true, progress: null })
  try {
    setState('result', await nest(jobFromState()))
  } catch (e) {
    notify('error', String(e))
  } finally {
    setState({ nesting: false, progress: null })
  }
}

async function runExport(format: 'svg' | 'dxf') {
  const path = await save({
    defaultPath: `nest.${format}`,
    filters: [{ name: format.toUpperCase(), extensions: [format] }]
  })
  if (!path) return
  try {
    await exportLayout(path, format, state.settings.kerf, state.settings.leadIn)
    notify('info', `Saved ${path}`)
  } catch (e) {
    notify('error', String(e))
  }
}

const btn =
  'rounded-md px-3 py-1.5 text-sm font-medium disabled:cursor-not-allowed disabled:opacity-40'

const Toolbar: Component = () => {
  onMount(() => {
    const off = onProgress(p => setState('progress', p))
    onCleanup(() => void off.then(f => f()))
  })

  const canNest = () => !state.nesting && state.cards.some(c => c.quantity > 0)
  const fraction = () => {
    const p = state.progress
    return p ? Math.min(1, p.elapsed_ms / Math.max(1, p.budget_ms)) : 0
  }

  return (
    <header class="flex h-12 shrink-0 items-center gap-3 border-b border-slate-200 bg-white px-4 dark:border-slate-800 dark:bg-slate-900">
      <h1 class="text-sm font-semibold">
        Deepnest <span class="text-sky-600">Next</span>
      </h1>
      <div class="mx-2 h-5 w-px bg-slate-200 dark:bg-slate-700" />
      <button
        class={`${btn} bg-sky-600 text-white hover:bg-sky-500`}
        disabled={!canNest()}
        onClick={() => void runNest()}
      >
        {state.nesting ? 'Nesting…' : 'Nest'}
      </button>
      <Show when={state.nesting}>
        <div class="flex items-center gap-2 text-xs text-slate-500">
          <div class="h-1.5 w-40 overflow-hidden rounded-full bg-slate-200 dark:bg-slate-700">
            <div
              class="h-full bg-sky-500 transition-[width] duration-100"
              style={{ width: `${fraction() * 100}%` }}
            />
          </div>
          <span class="tabular-nums">
            {((state.progress?.elapsed_ms ?? 0) / 1000).toFixed(1)} /{' '}
            {state.settings.timeSeconds} s
          </span>
        </div>
      </Show>
      <div class="ml-auto flex gap-2">
        <button
          class={`${btn} border border-slate-300 hover:bg-slate-50 dark:border-slate-700 dark:hover:bg-slate-800`}
          disabled={!state.result || state.nesting}
          onClick={() => void runExport('svg')}
        >
          Export SVG
        </button>
        <button
          class={`${btn} border border-slate-300 hover:bg-slate-50 dark:border-slate-700 dark:hover:bg-slate-800`}
          disabled={!state.result || state.nesting}
          onClick={() => void runExport('dxf')}
        >
          Export DXF
        </button>
      </div>
    </header>
  )
}

export default Toolbar
