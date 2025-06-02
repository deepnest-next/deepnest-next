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
import Footer from '@components/Footer'
import Header from '@components/Header'
import Main from '@components/Main'
import type { Component } from 'solid-js'

const App: Component = () => {
  return (
    <div class="min-h-screen flex flex-col items-center justify-center bg-gray-100">
      <Header />
      <Main />
      <Footer />
    </div>
  )
}

export default App

