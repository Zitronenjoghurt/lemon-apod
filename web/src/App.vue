<script setup lang="ts">
import { RouterLink, RouterView } from 'vue-router'
import { throttled } from '@/api/client'
import { useFavorites } from '@/composables/useFavorites'
import { useTheme } from '@/composables/useTheme'

const { theme, cycle } = useTheme()
const { count } = useFavorites()

const themeIcon = { auto: 'pi-desktop', dark: 'pi-moon', light: 'pi-sun' }
</script>

<template>
  <a href="#main" class="skip">Skip to content</a>

  <header class="site-header">
    <div class="container row justify">
      <RouterLink to="/" class="brand">
        <!-- A ringed planet, drawn rather than set as a glyph. The four-pointed star this
             replaced had turned into the Gemini logo, and rare astronomy glyphs like U+2609
             fall back to tofu on plenty of systems. -->
        <svg class="mark" viewBox="0 0 24 24" aria-hidden="true">
          <ellipse
            cx="12"
            cy="12"
            rx="11"
            ry="4.2"
            transform="rotate(-22 12 12)"
            fill="none"
            stroke="currentColor"
            stroke-width="1.6"
          />
          <circle cx="12" cy="12" r="6.2" fill="var(--bg)" />
          <circle cx="12" cy="12" r="6.2" fill="currentColor" fill-opacity="0.22" />
          <circle cx="12" cy="12" r="6.2" fill="none" stroke="currentColor" stroke-width="1.6" />
        </svg>
        <span>APOD Archive</span>
      </RouterLink>

      <nav class="row nav" aria-label="Main">
        <RouterLink to="/archive">Archive</RouterLink>
        <RouterLink to="/search">Search</RouterLink>
        <RouterLink to="/favorites">
          Favorites<span v-if="count" class="count">{{ count }}</span>
        </RouterLink>
        <RouterLink to="/random" title="A random entry">Random</RouterLink>
        <button
          type="button"
          class="theme"
          :title="`Theme: ${theme}`"
          :aria-label="`Theme: ${theme}. Click to change.`"
          @click="cycle"
        >
          <i class="pi" :class="themeIcon[theme]" aria-hidden="true" />
        </button>
      </nav>
    </div>
  </header>

  <!-- The API throttles rather than fails; saying so beats a spinner that looks stuck. -->
  <Transition name="fade">
    <div v-if="throttled" class="throttle" role="status">
      <i class="pi pi-clock" aria-hidden="true" /> Slowing down for a moment…
    </div>
  </Transition>

  <main id="main" class="container page">
    <RouterView v-slot="{ Component }">
      <Transition name="fade" mode="out-in">
        <component :is="Component" />
      </Transition>
    </RouterView>
  </main>

  <footer class="site-footer">
    <div class="container stack">
      <p class="muted">
        An unofficial archive of NASA's
        <a href="https://apod.nasa.gov/apod/" target="_blank" rel="noopener">
          Astronomy Picture of the Day</a
        >. Not affiliated with or endorsed by NASA.
      </p>
      <p class="muted small">
        Pictures and videos load from NASA's own servers and belong to the people and institutions
        credited on each entry. Explanations come from the original APOD pages, which every entry
        links to.
      </p>
    </div>
  </footer>
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

.site-header .container {
  min-height: 3.75rem;
}

.justify {
  justify-content: space-between;
}

.brand {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  font-weight: 650;
  letter-spacing: -0.01em;
  text-decoration: none;
  color: inherit;
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
  text-decoration: none;
  color: var(--text-muted);
  padding: 0.2rem 0;
  border-bottom: 2px solid transparent;
}

.nav a:hover {
  color: var(--text);
}

.nav a.router-link-active {
  color: var(--text);
  border-bottom-color: var(--accent);
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

.theme {
  background: none;
  border: 0;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 1rem;
  padding: 0.25rem;
}

.theme:hover {
  color: var(--text);
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
  padding-block: 2rem 4rem;
  min-height: 60vh;
}

.site-footer {
  border-top: 1px solid var(--border);
  padding-block: 2rem 3rem;
  font-size: 0.9rem;
}

.site-footer .stack {
  gap: 0.6rem;
}

.site-footer p {
  margin: 0;
}

.small {
  font-size: 0.82rem;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

@media (max-width: 34rem) {
  .nav {
    gap: 0.75rem;
    font-size: 0.86rem;
  }

  .brand span:last-child {
    display: none;
  }
}
</style>
