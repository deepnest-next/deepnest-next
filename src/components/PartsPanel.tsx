import { createSignal, For, Show, type Component } from 'solid-js'
import { importPart, removePart } from '../lib/api'
import { cardHue, notify, setState, state } from '../lib/store'

/** Import files one by one; a bad file is reported and skipped. */
async function addFiles(files: File[]) {
  for (const file of files) {
    try {
      const info = await importPart(file.name, await file.text())
      setState('cards', c => [...c, { info, quantity: 1, rotations: 4, mirror: false }])
      setState({ selected: info.id, result: null })
      for (const w of info.warnings) notify('info', `${file.name}: ${w}`)
    } catch (e) {
      notify('error', `${file.name}: ${String(e)}`)
    }
  }
}

async function remove(id: number) {
  await removePart(id)
  setState('cards', c => c.filter(x => x.info.id !== id))
  setState({ result: null })
  if (state.selected === id) setState('selected', null)
}

const fmt = (mm: number) => (mm >= 100 ? mm.toFixed(0) : mm.toFixed(1))

const PartsPanel: Component = () => {
  let input!: HTMLInputElement
  const [over, setOver] = createSignal(false)
  const total = () => state.cards.reduce((n, c) => n + c.quantity, 0)

  return (
    <aside
      class="flex h-full w-72 shrink-0 flex-col border-r border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900"
      classList={{ 'ring-2 ring-inset ring-sky-500': over() }}
      onDragOver={e => {
        e.preventDefault()
        setOver(true)
      }}
      onDragLeave={() => setOver(false)}
      onDrop={e => {
        e.preventDefault()
        setOver(false)
        void addFiles([...(e.dataTransfer?.files ?? [])])
      }}
    >
      <div class="flex items-center justify-between px-4 pt-4 pb-2">
        <h2 class="text-xs font-semibold tracking-wide text-slate-500 uppercase">
          Parts <span class="font-normal normal-case">· {total()} pcs</span>
        </h2>
        <button
          class="rounded-md bg-slate-900 px-2.5 py-1 text-xs font-medium text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-slate-100 dark:text-slate-900 dark:hover:bg-white"
          disabled={state.nesting}
          onClick={() => input.click()}
        >
          Add DXF / SVG
        </button>
        <input
          ref={el => (input = el)}
          type="file"
          accept=".dxf,.svg,image/svg+xml"
          multiple
          class="hidden"
          onChange={e => {
            void addFiles([...(e.currentTarget.files ?? [])])
            e.currentTarget.value = ''
          }}
        />
      </div>

      <div class="min-h-0 flex-1 overflow-y-auto px-2 pb-3">
        <Show
          when={state.cards.length}
          fallback={
            <p class="mx-2 mt-4 rounded-lg border border-dashed border-slate-300 p-6 text-center text-sm text-slate-400 dark:border-slate-700">
              Drop DXF or SVG files here
            </p>
          }
        >
          <For each={state.cards}>
            {(card, i) => (
              <div
                class="group mb-1 cursor-pointer rounded-lg border px-3 py-2"
                classList={{
                  'border-sky-500 bg-sky-50 dark:bg-sky-950/40':
                    state.selected === card.info.id,
                  'border-transparent hover:bg-slate-50 dark:hover:bg-slate-800/60':
                    state.selected !== card.info.id
                }}
                onClick={() => setState('selected', card.info.id)}
              >
                <div class="flex items-center gap-2">
                  <span
                    class="h-3 w-3 shrink-0 rounded-sm"
                    style={{ background: `hsl(${cardHue(card.info.id)} 55% 62%)` }}
                  />
                  <span class="truncate text-sm font-medium" title={card.info.name}>
                    {card.info.name}
                  </span>
                  <button
                    class="ml-auto text-slate-400 opacity-0 group-hover:opacity-100 hover:text-red-600 disabled:hidden"
                    title="Remove"
                    disabled={state.nesting}
                    onClick={e => {
                      e.stopPropagation()
                      void remove(card.info.id)
                    }}
                  >
                    ✕
                  </button>
                </div>
                <div class="mt-0.5 text-[11px] text-slate-500">
                  {fmt(card.info.width)} × {fmt(card.info.height)} mm
                  <Show when={card.info.shapes.length > 1}>
                    {' '}
                    · {card.info.shapes.length} shapes
                  </Show>
                  <Show when={card.info.warnings.length}>
                    <span
                      class="ml-1 text-amber-600"
                      title={card.info.warnings.join('\n')}
                    >
                      ⚠ units
                    </span>
                  </Show>
                </div>
                <div class="mt-2 flex items-center gap-3 text-xs text-slate-600 dark:text-slate-300">
                  <label class="flex items-center gap-1">
                    Qty
                    <input
                      type="number"
                      min="0"
                      class="w-14 rounded border border-slate-300 bg-transparent px-1 py-0.5 dark:border-slate-700"
                      value={card.quantity}
                      onChange={e =>
                        setState(
                          'cards',
                          i(),
                          'quantity',
                          Math.max(0, Math.round(+e.currentTarget.value || 0))
                        )
                      }
                    />
                  </label>
                  <label class="flex items-center gap-1" title="Allowed rotations">
                    Rot
                    <select
                      class="rounded border border-slate-300 bg-transparent px-1 py-0.5 dark:border-slate-700"
                      value={card.rotations}
                      onChange={e =>
                        setState('cards', i(), 'rotations', +e.currentTarget.value)
                      }
                    >
                      <option value="1">0°</option>
                      <option value="2">180°</option>
                      <option value="4">90°</option>
                      <option value="8">45°</option>
                    </select>
                  </label>
                  <label class="flex items-center gap-1" title="Allow mirrored placement">
                    <input
                      type="checkbox"
                      checked={card.mirror}
                      onChange={e =>
                        setState('cards', i(), 'mirror', e.currentTarget.checked)
                      }
                    />
                    Flip
                  </label>
                </div>
              </div>
            )}
          </For>
        </Show>
      </div>
    </aside>
  )
}

export default PartsPanel
