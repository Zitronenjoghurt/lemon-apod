import { createRouter, createWebHistory } from 'vue-router'

const DATE = '\\d{4}-\\d{2}-\\d{2}'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', name: 'home', component: () => import('@/views/HomeView.vue') },
    { path: '/search', name: 'search', component: () => import('@/views/SearchView.vue') },
    { path: '/favorites', name: 'favorites', component: () => import('@/views/FavoritesView.vue') },
    { path: '/random', name: 'random', component: () => import('@/views/RandomView.vue') },
    {
      path: '/archive/:year(\\d{4})?/:month(\\d{2})?',
      name: 'archive',
      component: () => import('@/views/ArchiveView.vue'),
    },
    {
      path: `/:date(${DATE})`,
      name: 'entry',
      component: () => import('@/views/EntryView.vue'),
    },
    {
      path: '/:pathMatch(.*)*',
      name: 'not-found',
      component: () => import('@/views/NotFoundView.vue'),
    },
  ],
  scrollBehavior(to, from, saved) {
    if (saved) return saved
    // Paging through search results shouldn't yank you back to the top.
    if (to.name === from.name && to.name === 'search') return {}
    return { top: 0 }
  },
})

export default router
