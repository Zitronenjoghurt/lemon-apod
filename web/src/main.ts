import { createApp } from 'vue'
import PrimeVue from 'primevue/config'
import ConfirmationService from 'primevue/confirmationservice'
import ToastService from 'primevue/toastservice'
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
        cssLayer: { name: 'primevue', order: 'base, primevue' },
      },
    },
    ripple: false,
  })
  .use(ToastService)
  .use(ConfirmationService)
  .directive('tooltip', Tooltip)
  .mount('#app')
