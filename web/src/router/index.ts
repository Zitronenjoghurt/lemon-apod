import { createRouter, createWebHistory } from 'vue-router'

const DATE = '\\d{4}-\\d{2}-\\d{2}'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', name: 'status', component: () => import('@/views/StatusView.vue') },
    { path: '/feed', name: 'feed', component: () => import('@/views/FeedView.vue') },
    { path: '/search', name: 'search', component: () => import('@/views/SearchView.vue') },
    { path: '/favorites', name: 'favorites', component: () => import('@/views/FavoritesView.vue') },
    { path: '/random', name: 'random', component: () => import('@/views/RandomView.vue') },
    { path: '/stats', name: 'stats', component: () => import('@/views/StatsView.vue') },
    { path: '/resources', name: 'resources', component: () => import('@/views/ResourcesView.vue') },
    { path: '/contact', name: 'contact', component: () => import('@/views/ContactView.vue') },
    {
      path: '/resources/:id(\\d+)',
      name: 'resource',
      component: () => import('@/views/ResourceView.vue'),
    },
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
    if (to.name === 'feed') return false
    if (saved) return saved
    if (to.name === from.name && ['search', 'resources', 'stats'].includes(String(to.name))) {
      return {}
    }
    return { top: 0 }
  },
})

export default router
