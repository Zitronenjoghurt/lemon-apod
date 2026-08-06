import { createApp } from 'vue'
import PrimeVue from 'primevue/config'
import Tooltip from 'primevue/tooltip'

import App from './App.vue'
import router from './router'
import { ApodPreset } from './theme'

import 'primeicons/primeicons.css'
import './assets/main.css'

createApp(App)
  .use(router)
  .use(PrimeVue, {
    theme: {
      preset: ApodPreset,
      options: {
        darkModeSelector: '.app-dark',
        cssLayer: { name: 'primevue', order: 'theme, base, primevue' },
      },
    },
    ripple: false,
  })
  .directive('tooltip', Tooltip)
  .mount('#app')
