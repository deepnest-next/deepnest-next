/*
 * Copyright (c) 2025 DeineAgentur UG (haftungsbeschränkt) <https://www.deineagentur.com>
 *
 * SPDX-License-Identifier: AGPL-v3-only OR LicenseRef-Deepnest-Commercial
 * Please see LICENSE files in the repository root for full details.
 *
 * Created Date: Monday, June 2nd 2025, 11:07:50 am
 * Author: Josef (Dexus) Fröhle - <github@josef-froehle.de> - <https://www.deepnest.net>
 *
 * HISTORY:
 * Date      	By     	Comments
 * ----------	-------	----------------------------------------------------------
 * 2025-06-02	Dexus	  Initial creation
 */
import PartsPanel from '@components/PartsPanel'
import SettingsPanel from '@components/SettingsPanel'
import Toolbar from '@components/Toolbar'
import Viewport from '@components/Viewport'
import { For, type Component } from 'solid-js'
import { state } from './lib/store'

const App: Component = () => {
  return (
    <div class="flex h-screen flex-col bg-slate-100 text-slate-900 select-none dark:bg-slate-950 dark:text-slate-100">
      <Toolbar />
      <div class="flex min-h-0 flex-1">
        <PartsPanel />
        <main class="min-w-0 flex-1">
          <Viewport />
        </main>
        <SettingsPanel />
      </div>
      <div class="pointer-events-none fixed right-4 bottom-4 flex w-96 flex-col gap-2">
        <For each={state.notices}>
          {n => (
            <div
              class="rounded-md px-3 py-2 text-sm shadow-lg"
              classList={{
                'bg-red-600 text-white': n.kind === 'error',
                'bg-slate-800 text-slate-100': n.kind === 'info'
              }}
            >
              {n.text}
            </div>
          )}
        </For>
      </div>
    </div>
  )
}

export default App
