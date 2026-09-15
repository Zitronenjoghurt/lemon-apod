<script lang="ts" setup>
import { useRoute } from 'vue-router'
import TabBar, { type Tab } from './TabBar.vue'

const TABS: Tab[] = [
  { to: '/objects', label: 'Objects' },
  { to: '/credits', label: 'Credits' },
  { to: '/pictures', label: 'Encores' },
  { to: '/resources', label: 'Resources' },
]

const route = useRoute()

function on(tab: Tab): boolean {
  return !!tab.to && (route.path === tab.to || route.path.startsWith(`${tab.to}/`))
}
</script>

<template>
  <TabBar :on="on" :tabs="TABS" label="Indexes" />
</template>
