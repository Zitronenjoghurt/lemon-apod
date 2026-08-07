<script lang="ts" setup>
import { ref } from 'vue'
import { RouterLink, RouterView, useRouter } from 'vue-router'
import { throttled } from '@/api/client'
import { useFavorites } from '@/composables/useFavorites'
import { useTheme } from '@/composables/useTheme'

const router = useRouter()
const { theme, cycle } = useTheme()
const { count } = useFavorites()

const menuOpen = ref(false)

const themeIcon = { dark: 'pi-moon', light: 'pi-sun' }
const themeLabel = { auto: 'Match system', dark: 'Dark', light: 'Light' }

const links = [
  { to: '/feed', label: 'Feed', icon: 'pi pi-bars' },
  { to: '/archive', label: 'Archive', icon: 'pi pi-calendar' },
  { to: '/search', label: 'Search', icon: 'pi pi-search' },
  { to: '/favorites', label: 'Favorites', icon: 'pi pi-star' },
  { to: '/random', label: 'Random', icon: 'pi pi-sync' },
]

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

      <nav aria-label="Main" class="row nav wide-only">
        <RouterLink v-for="link in links" :key="link.to" :to="link.to">
          <i :class="link.icon" aria-hidden="true" />
          {{ link.label }}
          <span v-if="link.to === '/favorites' && count" class="count">{{ count }}</span>
        </RouterLink>
      </nav>

      <div class="row trailing">
        <Button
          v-tooltip.bottom="`Theme: ${themeLabel[theme]}`"
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
      <RouterLink v-for="link in links" :key="link.to" :to="link.to" class="menu-link">
        <i :class="link.icon" aria-hidden="true" />
        <span>{{ link.label }}</span>
        <span v-if="link.to === '/favorites' && count" class="count">{{ count }}</span>
      </RouterLink>
    </nav>
  </Drawer>

  <Transition name="fade">
    <div v-if="throttled" class="throttle" role="status">
      <i aria-hidden="true" class="pi pi-clock" /> Slowing down for a moment…
    </div>
  </Transition>

  <main id="main" class="container page">
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

    <div class="container">
      <p class="muted">
        An unofficial archive of NASA's
        <a href="https://apod.nasa.gov/apod/" rel="noopener" target="_blank">
          Astronomy Picture of the Day</a
        >. Not affiliated with or endorsed by NASA. Pictures and videos load from NASA's own servers
        and belong to the people and institutions credited on each entry; explanations come from the
        original APOD pages, which every entry links to.
      </p>
    </div>
  </footer>

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
  min-height: 3.75rem;
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

.nav {
  gap: 1rem;
  font-size: 0.94rem;
}

.nav a {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  text-decoration: none;
  color: var(--text-muted);
  padding: 0.2rem 0;
  border-bottom: 2px solid transparent;
}

.nav a i {
  font-size: 0.82em;
  opacity: 0.85;
}

.auto-mark {
  width: 1rem;
  height: 1rem;
}

.nav a:hover {
  color: var(--text);
}

.nav a.router-link-active {
  color: var(--text);
  border-bottom-color: var(--accent);
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

.menu-link {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.85rem 0.75rem;
  border-radius: 0.6rem;
  text-decoration: none;
  color: var(--text);
}

.menu-link:hover {
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.menu-link.router-link-active {
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 12%, transparent);
}

.menu-link i {
  width: 1.25rem;
  text-align: center;
}

.menu-link .count {
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

.site-footer p {
  margin-inline: auto;
  margin-block: 0;
  max-width: 80ch;
  line-height: 1.5;
  text-align: center;
  text-wrap: pretty;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.narrow-only {
  display: none;
}

@media (max-width: 48rem) {
  .wide-only {
    display: none;
  }

  .narrow-only {
    display: inline-flex;
  }
}
</style>
