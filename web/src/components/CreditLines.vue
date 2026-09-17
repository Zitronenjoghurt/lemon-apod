<script lang="ts" setup>
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import type { Contributor, Credit } from '@/api/types'
import { withInternalLinks } from '@/utils/apodLinks'
import { licenseName, roleLabel } from '@/utils/credits'
import { contributorTargets, withIndexLinks } from '@/utils/indexLinks'

const props = withDefaults(
  defineProps<{
    credits: Credit[]
    hasCopyright?: boolean
    licenseUrl?: string
    contributors?: Contributor[]
  }>(),
  { hasCopyright: false, licenseUrl: undefined, contributors: () => [] },
)

const router = useRouter()

const lines = computed(() => {
  const targets = contributorTargets(props.contributors)
  const placed = new Set<string>()

  return props.credits.map((credit) => ({
    label: roleLabel(credit.role),
    html: withIndexLinks(withInternalLinks(credit.html), targets, placed),
  }))
})

const license = computed(() =>
  props.licenseUrl ? { url: props.licenseUrl, name: licenseName(props.licenseUrl) } : null,
)

function onInternalLink(event: MouseEvent) {
  if (event.defaultPrevented || event.button !== 0) return
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return

  const href = (event.target as HTMLElement | null)?.closest('a')?.getAttribute('href')
  if (!href?.startsWith('/')) return

  event.preventDefault()
  void router.push(href)
}
</script>

<template>
  <dl class="credits muted" @click="onInternalLink">
    <template v-for="(line, index) in lines" :key="line.label + index">
      <dt>{{ line.label }}</dt>
      <dd>
        <span v-html="line.html" />
        <span
          v-if="index === 0 && hasCopyright"
          class="rights"
          title="Credited to a named copyright holder rather than released as public domain by NASA"
        >
          Copyrighted
        </span>
        <a
          v-if="index === 0 && license"
          :href="license.url"
          class="rights"
          rel="noopener license"
          target="_blank"
          title="Released under this licence rather than as public domain by NASA"
        >
          {{ license.name }}
        </a>
      </dd>
    </template>
  </dl>
</template>

<style scoped>
.credits {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: var(--space-1) var(--space-3);
  font-size: var(--text-sm);
  margin: 0;
}

.credits dt {
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  padding-top: var(--space-0);
  opacity: 0.75;
}

.credits dd {
  margin: 0;
}

.rights {
  display: inline-block;
  margin-left: var(--space-2);
  padding: var(--space-0) var(--space-2);
  border: 1px solid var(--border);
  border-radius: var(--radius-pill);
  font-size: var(--text-xs);
  white-space: nowrap;
  vertical-align: 0.05em;
  text-decoration: none;
}

a.rights:hover {
  border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
}
</style>
