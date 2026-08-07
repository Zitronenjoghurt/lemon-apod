<script lang="ts" setup>
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'
import EntrySkeleton from '@/components/EntrySkeleton.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import { useAsync } from '@/composables/useAsync'

const router = useRouter()

const { error, loading, run } = useAsync(async (signal) => {
  const entry = await api.random(undefined, signal)
  await router.replace(`/${entry.date}`)
})

onMounted(run)
</script>

<template>
  <div v-if="error" class="stack notice">
    <RetryNotice :busy="loading" :message="error" @retry="run" />
  </div>

  <EntrySkeleton v-else />
</template>

<style scoped>
.notice {
  max-width: 34rem;
  margin-inline: auto;
}
</style>
