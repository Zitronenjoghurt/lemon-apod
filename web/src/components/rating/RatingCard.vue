<script lang="ts" setup>
import { computed, onMounted, ref } from 'vue'
import { RouterLink } from 'vue-router'
import RatingProgress from '@/components/rating/RatingProgress.vue'
import { api } from '@/api/client'
import type { Board } from '@/api/types'
import { useRatingCard } from '@/composables/useRating'

const { dismissed, dismiss } = useRatingCard()

const board = ref<Board | null>(null)

onMounted(async () => {
  if (dismissed.value) return
  try {
    board.value = await api.rating.board('beautiful', { limit: 0 })
  } catch {
    board.value = null
  }
})

const votes = computed(() => board.value?.votes.toLocaleString() ?? '0')
const filling = computed(() => board.value?.provisional ?? true)
</script>

<template>
  <section v-if="!dismissed && board" class="card rating-card">
    <div class="body">
      <p class="muted kicker">
        <AppIcon name="vote" />
        Vote for your favorite APOD
      </p>
      <h2>Which picture do users think is the best?</h2>
      <p v-if="filling" class="muted line">
        {{ votes }} votes so far, the results are not significant enough yet.
      </p>
      <p v-else class="muted line">
        {{ votes }} votes in total. The results are significant, more votes will sharpen it further
        though.
      </p>

      <RatingProgress v-if="filling" :progress="board.progress" />

      <div class="row actions">
        <RouterLink v-slot="{ navigate }" custom to="/rating/vote">
          <Button label="Get voting!" size="small" @click="navigate">
            <template #icon><AppIcon name="vote" /></template>
          </Button>
        </RouterLink>
        <RouterLink v-slot="{ navigate }" custom to="/rating">
          <Button
            label="See the results"
            outlined
            severity="secondary"
            size="small"
            @click="navigate"
          >
            <template #icon><AppIcon name="list" /></template>
          </Button>
        </RouterLink>
      </div>
    </div>

    <Button
      aria-label="Dismiss this card"
      class="close"
      rounded
      severity="secondary"
      text
      @click="dismiss"
    >
      <template #icon><AppIcon name="times" /></template>
    </Button>
  </section>
</template>

<style scoped>
.rating-card {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-4) var(--space-4) var(--space-5);
  border-color: color-mix(in srgb, var(--accent) 30%, var(--border));
}

.body {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
  flex: 1 1 auto;
}

.kicker {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.07em;
}

.kicker .icon {
  color: var(--accent);
}

h2 {
  font-size: var(--text-md);
  text-wrap: balance;
}

.line {
  margin: 0;
  font-size: var(--text-sm);
  text-wrap: pretty;
}

.actions {
  gap: var(--space-2);
  margin-top: var(--space-0);
  flex-wrap: wrap;
}

.close {
  flex: none;
  margin-left: auto;
}
</style>
