import { createRouter, createWebHistory } from 'vue-router'
import { archiveTitle, pageTitle, setTitle, SITE } from '@/utils/title'

const DATE = '\\d{4}-\\d{2}-\\d{2}'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', name: 'status', component: () => import('@/views/StatusView.vue') },
    {
      path: '/feed',
      name: 'feed',
      component: () => import('@/views/FeedView.vue'),
      meta: { title: 'Feed' },
    },
    {
      path: '/search',
      name: 'search',
      component: () => import('@/views/SearchView.vue'),
      meta: { title: 'Search' },
    },
    {
      path: '/favorites',
      name: 'favorites',
      component: () => import('@/views/FavoritesView.vue'),
      meta: { title: 'Favorites' },
    },
    {
      path: '/random',
      name: 'random',
      component: () => import('@/views/RandomView.vue'),
      meta: { title: 'A random entry' },
    },
    {
      path: '/stats',
      name: 'stats',
      component: () => import('@/views/StatsView.vue'),
      meta: { title: 'Statistics' },
    },
    {
      path: '/space-weather',
      name: 'space-weather',
      component: () => import('@/views/SpaceWeatherView.vue'),
      meta: { title: 'Space weather' },
    },
    {
      path: '/games',
      name: 'games',
      component: () => import('@/views/GamesView.vue'),
      meta: { title: 'Games' },
    },
    {
      path: '/games/date',
      name: 'game-date',
      component: () => import('@/views/games/GuessDateView.vue'),
      meta: { title: 'Guess the Date' },
    },
    {
      path: '/games/words',
      name: 'game-words',
      component: () => import('@/views/games/FillWordsView.vue'),
      meta: { title: 'Fill the Words' },
    },
    {
      path: '/games/order',
      name: 'game-order',
      component: () => import('@/views/games/OrderView.vue'),
      meta: { title: 'Older or Newer' },
    },
    {
      path: '/games/match',
      name: 'game-match',
      component: () => import('@/views/games/MatchView.vue'),
      meta: { title: 'Match the Picture' },
    },
    {
      path: '/rating',
      name: 'rating',
      component: () => import('@/views/rating/BoardView.vue'),
      meta: { title: 'Reader ratings' },
    },
    {
      path: '/rating/vote',
      name: 'rating-vote',
      component: () => import('@/views/rating/VoteView.vue'),
      meta: { title: 'Vote on a pair' },
    },
    {
      path: '/resources',
      name: 'resources',
      component: () => import('@/views/ResourcesView.vue'),
      meta: { title: 'Resources' },
    },
    {
      path: '/pictures',
      name: 'pictures',
      component: () => import('@/views/PicturesView.vue'),
      meta: { title: 'Encores' },
    },
    {
      path: '/notifications',
      name: 'notifications',
      component: () => import('@/views/NotificationsView.vue'),
      meta: { title: 'Notifications' },
    },
    {
      path: '/contact',
      name: 'contact',
      component: () => import('@/views/ContactView.vue'),
      meta: { title: 'Contact' },
    },
    {
      path: '/resources/:id(\\d+)',
      name: 'resource',
      component: () => import('@/views/ResourceView.vue'),
    },
    {
      path: `/pictures/:date(${DATE})`,
      name: 'picture',
      component: () => import('@/views/PictureView.vue'),
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
    if (
      to.name === from.name &&
      ['search', 'resources', 'pictures', 'stats', 'rating'].includes(String(to.name))
    ) {
      return {}
    }
    return { top: 0 }
  },
})

router.afterEach((to) => {
  if (typeof to.meta.title === 'string') {
    setTitle(pageTitle(to.meta.title))
    return
  }

  if (to.name === 'archive') {
    const { year, month } = to.params
    setTitle(archiveTitle(year as string | undefined, month as string | undefined))
    return
  }

  if (to.name === 'status' || to.name === 'not-found') setTitle(SITE)
})

export default router
