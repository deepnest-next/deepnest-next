import { For, Show, type Component, type JSX } from 'solid-js'
import type { Strategy } from '../lib/api'
import { setState, state, type Settings } from '../lib/store'

const NumberField: Component<{
  label: string
  key: keyof Omit<Settings, 'strategy'>
  min?: number
  step?: string
  unit?: string
  title?: string
}> = props => (
  <label class="flex flex-col gap-1 text-xs text-slate-500" title={props.title}>
    <span>
      {props.label}
      <Show when={props.unit}>
        <span class="text-slate-400"> ({props.unit})</span>
      </Show>
    </span>
    <input
      type="number"
      min={props.min ?? 0}
      step={props.step ?? 'any'}
      class="rounded-md border border-slate-300 bg-transparent px-2 py-1 text-sm text-slate-900 dark:border-slate-700 dark:text-slate-100"
      value={state.settings[props.key]}
      disabled={state.nesting}
      onChange={e => {
        const v = +e.currentTarget.value
        if (Number.isFinite(v) && v >= (props.min ?? 0))
          setState('settings', props.key, v)
        else e.currentTarget.value = String(state.settings[props.key])
      }}
    />
  </label>
)

const Section: Component<{ title: string; children: JSX.Element }> = props => (
  <section class="border-b border-slate-200 px-4 py-3 dark:border-slate-800">
    <h2 class="mb-2 text-xs font-semibold tracking-wide text-slate-500 uppercase">
      {props.title}
    </h2>
    <div class="grid grid-cols-2 gap-2">{props.children}</div>
  </section>
)

const STRATEGIES: { value: Strategy; label: string }[] = [
  { value: 'fast', label: 'Fast' },
  { value: 'balanced', label: 'Balanced' },
  { value: 'high_quality', label: 'High quality' }
]

const pct = (x: number) => `${(x * 100).toFixed(1)}%`

const SettingsPanel: Component = () => (
  <aside class="flex h-full w-64 shrink-0 flex-col overflow-y-auto border-l border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
    <Section title="Sheet">
      <NumberField label="Width" key="sheetWidth" min={1} unit="mm" />
      <NumberField label="Height" key="sheetHeight" min={1} unit="mm" />
      <NumberField label="Sheets" key="sheets" step="1" title="0 = as many as needed" />
      <NumberField label="Margin" key="margin" unit="mm" />
    </Section>
    <Section title="Nesting">
      <NumberField label="Spacing" key="spacing" unit="mm" />
      <NumberField label="Time" key="timeSeconds" min={1} unit="s" />
      <label class="col-span-2 flex flex-col gap-1 text-xs text-slate-500">
        Strategy
        <select
          class="rounded-md border border-slate-300 bg-transparent px-2 py-1 text-sm text-slate-900 dark:border-slate-700 dark:text-slate-100"
          value={state.settings.strategy}
          disabled={state.nesting}
          onChange={e =>
            setState('settings', 'strategy', e.currentTarget.value as Strategy)
          }
        >
          <For each={STRATEGIES}>{s => <option value={s.value}>{s.label}</option>}</For>
        </select>
      </label>
    </Section>
    <Section title="Export">
      <NumberField
        label="Kerf"
        key="kerf"
        unit="mm"
        title="Cut width; outlines are offset by half"
      />
      <NumberField label="Lead-in" key="leadIn" unit="mm" title="Pierce lead-in length" />
    </Section>

    <Show when={state.result}>
      {r => (
        <section class="px-4 py-3 text-sm">
          <h2 class="mb-2 text-xs font-semibold tracking-wide text-slate-500 uppercase">
            Result
          </h2>
          <dl class="grid grid-cols-2 gap-y-1 text-slate-600 dark:text-slate-300">
            <dt>Placed</dt>
            <dd class="text-right tabular-nums">
              {r().required_placed} / {r().required_total}
            </dd>
            <dt>Sheets</dt>
            <dd class="text-right tabular-nums">{r().sheets.length}</dd>
            <dt>Utilization</dt>
            <dd class="text-right tabular-nums">{pct(r().utilization)}</dd>
            <dt>Time</dt>
            <dd class="text-right tabular-nums">
              {(r().runtime_ms / 1000).toFixed(1)} s
            </dd>
          </dl>
          <Show when={r().unplaced.length}>
            <ul class="mt-3 space-y-1 rounded-md bg-amber-50 p-2 text-xs text-amber-800 dark:bg-amber-950/40 dark:text-amber-300">
              <For each={r().unplaced}>
                {u => (
                  <li>
                    {u.count} × {u.name}: {u.reason}
                  </li>
                )}
              </For>
            </ul>
          </Show>
        </section>
      )}
    </Show>
  </aside>
)

export default SettingsPanel
