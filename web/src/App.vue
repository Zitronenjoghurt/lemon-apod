<script lang="ts" setup>
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { RouterLink, RouterView, useRoute, useRouter } from 'vue-router'
import ExternalLinkNotice from '@/components/ExternalLinkNotice.vue'
import SettingsDialog from '@/components/SettingsDialog.vue'
import { throttled } from '@/api/client'
import { useExternalLinks } from '@/composables/useExternalLinks'
import { useFavorites } from '@/composables/useFavorites'
import { useTheme } from '@/composables/useTheme'
import { MASTODON_URL, REPO_URL } from '@/utils/links'

const route = useRoute()
const router = useRouter()
const { theme, cycle } = useTheme()
const { count } = useFavorites()
const { intercept } = useExternalLinks()

const playingAGame = computed(() => {
  const play = route.query.play
  return String(route.name ?? '').startsWith('game-') && (play === 'daily' || play === 'free')
})

const menuOpen = ref(false)
const settingsOpen = ref(false)

const railQuery = window.matchMedia('(min-width: 48rem) and (max-width: 63.999rem)')
const rail = ref(railQuery.matches)
railQuery.addEventListener('change', (event) => (rail.value = event.matches))

onMounted(() => document.addEventListener('click', intercept, true))
onUnmounted(() => document.removeEventListener('click', intercept, true))

const themeIcon = { dark: 'pi-moon', light: 'pi-sun' }
const themeLabel = { auto: 'Auto', dark: 'Dark', light: 'Light' }

const groups = [
  {
    name: null,
    links: [{ to: '/', label: 'Home', icon: 'pi pi-home', exact: true }],
  },
  {
    name: 'Read',
    links: [
      { to: '/feed', label: 'Feed', icon: 'pi pi-bars' },
      { to: '/archive', label: 'Archive', icon: 'pi pi-calendar' },
      { to: '/random', label: 'Random', icon: 'pi pi-sync' },
      { to: '/favorites', label: 'Favorites', icon: 'pi pi-star' },
    ],
  },
  {
    name: 'Dive deeper',
    links: [
      { to: '/search', label: 'Search', icon: 'pi pi-search' },
      { to: '/pictures', label: 'Encores', icon: 'pi pi-replay' },
      { to: '/resources', label: 'Resources', icon: 'pi pi-link' },
      { to: '/stats', label: 'Stats', icon: 'pi pi-chart-bar' },
      { to: '/games', label: 'Games', icon: 'pi pi-play-circle' },
    ],
  },
  {
    name: 'Stay up to date',
    links: [
      { to: '/space-weather', label: 'Space weather', icon: 'pi pi-bolt' },
      { to: '/notifications', label: 'Notifications', icon: 'pi pi-bell' },
    ],
  },
]

type NavLink = { to: string; label: string; icon: string; exact?: boolean }

function isActive(link: NavLink): boolean {
  if (link.exact) return route.path === link.to
  return route.path === link.to || route.path.startsWith(`${link.to}/`)
}

const version = __APP_VERSION__

router.afterEach(() => (menuOpen.value = false))
</script>

<template>
  <a class="skip" href="#main">Skip to content</a>

  <header class="site-header">
    <div class="container bar">
      <RouterLink class="brand" to="/">
        <svg aria-hidden="true" class="mark" viewBox="0 0 24 24">
          <ellipse
            cx="12"
            cy="12"
            fill="none"
            rx="11"
            ry="4.2"
            stroke="currentColor"
            stroke-width="1.6"
            transform="rotate(-22 12 12)"
          />
          <circle cx="12" cy="12" fill="var(--bg)" r="6.2" />
          <circle cx="12" cy="12" fill="currentColor" fill-opacity="0.22" r="6.2" />
          <circle cx="12" cy="12" fill="none" r="6.2" stroke="currentColor" stroke-width="1.6" />
        </svg>
        <span>APOD Archive</span>
      </RouterLink>

      <div class="row trailing">
        <Button
          v-tooltip.bottom="{ value: `Theme: ${themeLabel[theme]}`, class: 'tip-tight' }"
          :aria-label="`Theme: ${themeLabel[theme]}. Activate to change.`"
          rounded
          severity="secondary"
          text
          @click="cycle"
        >
          <i v-if="theme !== 'auto'" :class="`pi ${themeIcon[theme]}`" aria-hidden="true" />
          <svg v-else aria-hidden="true" class="auto-mark" viewBox="0 0 16 16">
            <circle cx="8" cy="8" fill="none" r="6.4" stroke="currentColor" stroke-width="1.5" />
            <path d="M8 1.6a6.4 6.4 0 0 0 0 12.8z" fill="currentColor" />
          </svg>
        </Button>
        <Button
          v-tooltip.bottom="{ value: 'Settings', class: 'tip-tight' }"
          aria-label="Settings"
          icon="pi pi-cog"
          rounded
          severity="secondary"
          text
          @click="settingsOpen = true"
        />
        <Button
          aria-label="Open menu"
          class="narrow-only"
          icon="pi pi-bars"
          rounded
          severity="secondary"
          text
          @click="menuOpen = true"
        />
      </div>
    </div>
  </header>

  <Drawer v-model:visible="menuOpen" header="Menu" position="right">
    <nav aria-label="Main" class="menu">
      <template v-for="group in groups" :key="group.name ?? 'top'">
        <p v-if="group.name" class="group">{{ group.name }}</p>
        <RouterLink
          v-for="link in group.links"
          :key="link.to"
          :class="{ on: isActive(link) }"
          :to="link.to"
          active-class=""
          class="nav-link"
          exact-active-class=""
        >
          <i :class="link.icon" aria-hidden="true" />
          <span class="label">{{ link.label }}</span>
          <span v-if="link.to === '/favorites' && count" class="count">{{ count }}</span>
        </RouterLink>
      </template>
    </nav>
  </Drawer>

  <Transition name="fade">
    <div v-if="throttled" class="throttle" role="status">
      <i aria-hidden="true" class="pi pi-clock" /> Slowing down for a moment…
    </div>
  </Transition>

  <div class="shell">
    <aside class="sidebar">
      <nav aria-label="Main" class="menu">
        <template v-for="group in groups" :key="group.name ?? 'top'">
          <p v-if="group.name" class="group">{{ group.name }}</p>
          <RouterLink
            v-for="link in group.links"
            :key="link.to"
            v-tooltip.right="{ value: link.label, disabled: !rail, class: 'tip-tight' }"
            :class="{ on: isActive(link) }"
            :to="link.to"
            active-class=""
            class="nav-link"
            exact-active-class=""
          >
            <i :class="link.icon" aria-hidden="true" />
            <span class="label">{{ link.label }}</span>
            <span v-if="link.to === '/favorites' && count" class="count">{{ count }}</span>
          </RouterLink>
        </template>
      </nav>
    </aside>

    <div class="column">
      <main id="main" :class="['container', 'page', { wide: playingAGame }]">
        <RouterView v-slot="{ Component }">
          <Transition mode="out-in" name="fade">
            <KeepAlive :include="['FeedView']" :max="1">
              <component :is="Component" />
            </KeepAlive>
          </Transition>
        </RouterView>
      </main>

      <footer class="site-footer">
        <span class="version muted">v{{ version }}</span>

        <div class="container stack foot">
          <nav aria-label="Elsewhere" class="row foot-links">
            <RouterLink to="/contact"
              ><i aria-hidden="true" class="pi pi-envelope" />Contact
            </RouterLink>
            <a :href="REPO_URL" data-ours rel="noopener" target="_blank">
              <i aria-hidden="true" class="pi pi-github" />
              Source
            </a>
            <a :href="MASTODON_URL" data-ours rel="me noopener" target="_blank">
              <i aria-hidden="true" class="pi pi-at" />
              Mastodon
            </a>
          </nav>

          <p class="muted">
            An unofficial archive of NASA's
            <a href="https://apod.nasa.gov/apod/" rel="noopener" target="_blank">
              Astronomy Picture of the Day</a
            >. Not affiliated with or endorsed by NASA. The text and media of all APOD entries
            originate from NASA and belong to the credited people and institutions.
          </p>
        </div>
      </footer>
    </div>
  </div>

  <SettingsDialog v-model:visible="settingsOpen" />
  <ExternalLinkNotice />

  <Toast position="bottom-center" />
  <ConfirmDialog />
</template>

<style scoped>
.skip {
  position: absolute;
  left: -9999px;
  top: 0;
  background: var(--bg-elevated);
  padding: 0.6rem 1rem;
  z-index: 10;
}

.skip:focus {
  left: 0.5rem;
  top: 0.5rem;
}

.site-header {
  position: sticky;
  top: 0;
  z-index: 5;
  background: color-mix(in srgb, var(--bg) 85%, transparent);
  backdrop-filter: blur(10px);
  border-bottom: 1px solid var(--border);
}

.bar {
  display: flex;
  align-items: center;
  gap: 1rem;
  min-height: var(--header-h);
}

.brand {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  font-weight: 650;
  letter-spacing: -0.01em;
  text-decoration: none;
  color: inherit;
  margin-right: auto;
}

.mark {
  color: var(--accent);
  width: 1.35rem;
  height: 1.35rem;
  flex: none;
  overflow: visible;
}

.auto-mark {
  width: 1rem;
  height: 1rem;
}

.shell {
  flex: 1 0 auto;
  display: flex;
  align-items: flex-start;
  width: 100%;
}

.column {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  align-self: stretch;
}

.sidebar {
  display: none;
}

@media (min-width: 48rem) {
  .bar {
    max-width: none;
  }

  .sidebar {
    display: block;
    flex: none;
    align-self: stretch;
    width: var(--rail-w);
    padding: 0.9rem 0.5rem;
    border-right: 1px solid var(--border);
  }

  .sidebar .menu {
    position: sticky;
    top: calc(var(--header-h) + 1px + 0.9rem);
  }

  .sidebar .nav-link {
    position: relative;
    justify-content: center;
    padding: 0.7rem 0.5rem;
  }

  .sidebar .nav-link .label {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }

  .sidebar .group {
    height: 1px;
    margin: 0.5rem 0.55rem;
    padding: 0;
    overflow: hidden;
    color: transparent;
    background: var(--border);
  }

  .sidebar .count {
    position: absolute;
    top: 0.15rem;
    right: 0.2rem;
    margin: 0;
    font-size: 0.6rem;
    padding: 0 0.25rem;
  }
}

@media (min-width: 64rem) {
  .sidebar {
    width: var(--sidebar-w);
    padding: 1rem 0.6rem;
  }

  .sidebar .menu {
    top: calc(var(--header-h) + 1px + 1rem);
  }

  .sidebar .nav-link {
    justify-content: flex-start;
    padding: 0.55rem 0.65rem;
  }

  .sidebar .nav-link .label {
    position: static;
    width: auto;
    height: auto;
    clip-path: none;
  }

  .sidebar .group {
    height: auto;
    margin: 0.75rem 0 0.15rem;
    padding: 0 0.65rem;
    overflow: visible;
    color: var(--text-muted);
    background: none;
  }

  .sidebar .count {
    position: static;
    margin-left: auto;
    font-size: 0.72rem;
    padding: 0 0.4rem;
  }
}

.trailing {
  gap: 0.15rem;
  flex: none;
}

.count {
  display: inline-block;
  margin-left: 0.35rem;
  font-size: 0.72rem;
  background: color-mix(in srgb, var(--accent) 22%, transparent);
  color: var(--accent);
  border-radius: 999px;
  padding: 0 0.4rem;
}

.menu {
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}

.group {
  margin: 0.75rem 0 0.15rem;
  padding: 0 0.65rem;
  font-size: 0.66rem;
  text-transform: uppercase;
  letter-spacing: 0.09em;
  font-weight: 600;
  color: var(--text-muted);
}

.menu > .group:first-child {
  margin-top: 0;
}

.nav-link {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.85rem 0.75rem;
  border-radius: 0.6rem;
  text-decoration: none;
  color: var(--text);
}

.nav-link:hover {
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.nav-link.on {
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 12%, transparent);
}

.nav-link i {
  width: 1.25rem;
  text-align: center;
}

.nav-link .count {
  margin-left: auto;
}

.throttle {
  position: fixed;
  bottom: 1.25rem;
  left: 50%;
  transform: translateX(-50%);
  z-index: 20;
  background: var(--bg-elevated);
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 0.45rem 1rem;
  font-size: 0.88rem;
  box-shadow: var(--shadow);
  display: flex;
  gap: 0.45rem;
  align-items: center;
}

.page {
  padding-block: 2rem 3rem;
  flex: 1 0 auto;
}

.page.wide {
  --page-max: 86rem;
}

.site-footer {
  position: relative;
  border-top: 1px solid var(--border);
  padding-block: 1.1rem 1.4rem;
  font-size: 0.8rem;
}

.version {
  position: absolute;
  left: 0.75rem;
  bottom: 0.5rem;
  font-size: 0.72rem;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.02em;
}

.foot {
  gap: 0.7rem;
}

.foot-links {
  justify-content: center;
  gap: 1.25rem;
}

.foot-links a {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  color: var(--text-muted);
  text-decoration: none;
}

.foot-links a:hover {
  color: var(--text);
}

.foot-links i {
  font-size: 0.85em;
}

.site-footer p {
  margin-inline: auto;
  margin-block: 0;
  max-width: 80ch;
  line-height: 1.5;
  text-align: center;
  text-wrap: pretty;
}

.narrow-only {
  display: none;
}

@media (max-width: 47.999rem) {
  .narrow-only {
    display: inline-flex;
  }
}
</style>
