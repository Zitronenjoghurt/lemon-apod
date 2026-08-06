<script setup lang="ts">
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'
import EntrySkeleton from '@/components/EntrySkeleton.vue'
import { api } from '@/api/client'

const router = useRouter()

onMounted(async () => {
  try {
    const entry = await api.random()
    await router.replace(`/${entry.date}`)
  } catch {
    await router.replace('/')
  }
})
</script>

<template>
  <EntrySkeleton />
</template>
