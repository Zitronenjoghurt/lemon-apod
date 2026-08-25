<script lang="ts" setup>
import { computed, onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import ApodCredit from '@/components/ApodCredit.vue'
import RatingHelp from '@/components/rating/RatingHelp.vue'
import RatingOdds from '@/components/rating/RatingOdds.vue'
import RatingProgress from '@/components/rating/RatingProgress.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import type { Board, BoardRow, RatingCategory } from '@/api/types'
import { CATEGORIES, CATEGORY_ICONS, isCategory, ORDER } from '@/composables/useRating'
import { useAsync } from '@/composables/useAsync'
import { useNarrow } from '@/composables/useNarrow'
import { formatDate } from '@/utils/date'

const PAGE_SIZE = 48

const route = useRoute()
const router = useRouter()
const { pageLinks } = useNarrow()

const category = ref<RatingCategory>(
  isCategory(route.query.category) ? route.query.category : 'beautiful',
)
const page = ref(Number.parseInt(String(route.query.page ?? '1'), 10) || 1)
const helpOpen = ref(false)

const {
  data: board,
  error,
  loading,
  run,
} = useAsync<Board>((signal) =>
  api.rating.board(
    category.value,
    { limit: PAGE_SIZE, offset: (page.value - 1) * PAGE_SIZE },
    signal,
  ),
)

const question = computed(() => CATEGORIES[category.value])
const first = computed(() => (page.value - 1) * PAGE_SIZE)

const tiers = computed(() => {
  const groups: { tier: number; rows: BoardRow[]; from: boolean; into: boolean }[] = []

  for (const row of board.value?.rows ?? []) {
    const open = groups.at(-1)
    if (open && open.tier === row.tier) open.rows.push(row)
    else groups.push({ tier: row.tier, rows: [row], from: false, into: false })
  }

  for (const group of groups) {
    group.rows.sort((one, other) => other.score - one.score)
  }

  const last = groups.at(-1)
  if (groups[0] && first.value > 0) groups[0].from = true
  if (last && first.value + (board.value?.rows.length ?? 0) < (board.value?.ranked ?? 0)) {
    last.into = true
  }

  return groups
})

const fitted = computed(() => {
  const at = board.value?.fitted_at
  if (!at) return null
  const when = new Date(at)
  return Number.isNaN(when.getTime()) ? null : when.toLocaleString()
})

function locate(): void {
  void router.replace({
    name: 'rating',
    query: {
      category: category.value === 'beautiful' ? undefined : category.value,
      page: page.value === 1 ? undefined : String(page.value),
    },
  })
}

function switchTo(next: RatingCategory): void {
  if (next === category.value) return
  category.value = next
  page.value = 1
  locate()
  void run()
}

function onPage(event: { page: number }): void {
  page.value = event.page + 1
  locate()
  void run()
}

watch(board, (shown) => {
  if (!shown || shown.rows.length || page.value === 1) return
  page.value = Math.max(1, Math.ceil(shown.ranked / PAGE_SIZE))
  locate()
  void run()
})

onMounted(() => void run())
</script>

<template>
  <div class="stack board">
    <header class="row head">
      <h1>Best APOD Voting</h1>

      <div class="row tools">
        <SelectButton
          :allow-empty="false"
          :model-value="category"
          :options="ORDER"
          aria-label="Which board to show"
          size="small"
          @update:model-value="switchTo"
        >
          <template #option="{ option }">
            <span class="pick-label">
              <i :class="CATEGORY_ICONS[option as RatingCategory]" aria-hidden="true" />
              {{ CATEGORIES[option as RatingCategory].short }}
            </span>
          </template>
        </SelectButton>

        <button
          v-tooltip.bottom="'How this works'"
          class="help"
          type="button"
          @click="helpOpen = true"
        >
          <i aria-hidden="true" class="pi pi-question-circle" />
          <span class="sr-only">How this works</span>
        </button>
      </div>
    </header>

    <RetryNotice v-if="error" :busy="loading" :message="error" @retry="run" />

    <div v-else-if="!board" class="stack">
      <Skeleton height="6rem" width="100%" />
      <Skeleton height="18rem" width="100%" />
    </div>

    <template v-else>
      <section class="card state">
        <div class="row banner">
          <Tag v-if="board.provisional" severity="warn" value="Provisional" />
          <p class="ask">{{ question.ask }}</p>
          <p class="muted asked">
            {{ board.votes.toLocaleString() }} votes, {{ board.voters.toLocaleString() }} people
          </p>
        </div>

        <!-- A finished board has nothing to fill, so a full bar reading "15,000 of 15,000" and a
             badge saying Settled are both just furniture. The absence of the Provisional tag is the
             signal, and the numbers on the cards are the result. -->
        <RatingProgress v-if="board.provisional" :progress="board.progress" />

        <!-- The board is filled by voting, so the way to vote is the loudest thing on it rather
             than an icon in a corner. -->
        <div class="row call">
          <RouterLink
            v-slot="{ navigate }"
            :to="{ path: '/rating/vote', query: { category } }"
            custom
          >
            <Button icon="pi pi-images" label="Get voting!" @click="navigate" />
          </RouterLink>
        </div>

        <dl class="facts">
          <div>
            <dt class="muted">Pictures qualified</dt>
            <dd>{{ board.ranked.toLocaleString() }} of {{ board.pool.toLocaleString() }}</dd>
          </div>
          <div>
            <dt class="muted">Needed to qualify</dt>
            <dd>{{ board.min_comparisons }} votes</dd>
          </div>
          <div v-if="board.side_bias !== null">
            <dt class="muted">Left-hand bias</dt>
            <dd>{{ board.side_bias > 0 ? '+' : '' }}{{ board.side_bias.toFixed(3) }}</dd>
          </div>
          <div v-if="fitted">
            <dt class="muted">Tiers last calculated at</dt>
            <dd>{{ fitted }}</dd>
          </div>
        </dl>
      </section>

      <p v-if="!board.rows.length" class="muted empty">
        Nothing has had {{ board.min_comparisons }} votes yet. The board fills as votes come in and
        the fastest way to fill it is to
        <RouterLink :to="{ path: '/rating/vote', query: { category } }">vote yourself</RouterLink>!
      </p>

      <template v-else>
        <ApodCredit class="credit" variant="banner" />

        <ol class="tiers">
          <li v-for="group in tiers" :key="group.tier" class="tier">
            <p class="tier-head">
              <span class="tier-name">Tier {{ group.tier }}</span>
              <span
                v-if="group.rows.length > 1 || group.from || group.into"
                class="muted tier-note"
              >
                <template v-if="group.from && group.into"
                  >tied, and wraps to the next page</template
                >
                <template v-else-if="group.from">tied, carried on from the page before</template>
                <template v-else-if="group.into">tied, and carries on to the next page</template>
                <template v-else>{{ group.rows.length }} pictures, tied</template>
              </span>
              <span v-else-if="group.tier === 1 && board.favourite" class="muted tier-note">
                the current user favorite
              </span>
            </p>

            <ul class="grid cards">
              <li v-for="row in group.rows" :key="row.date">
                <article class="card entry">
                  <RouterLink :to="`/${row.date}`" class="thumb">
                    <img
                      v-if="row.media.thumb_url"
                      :alt="row.title"
                      :height="row.media.thumb_height ?? 300"
                      :src="row.media.thumb_url"
                      :width="row.media.thumb_width ?? 480"
                      decoding="async"
                      loading="lazy"
                    />
                    <span v-else class="fallback">
                      <i aria-hidden="true" class="pi pi-image" />
                    </span>
                  </RouterLink>

                  <div class="body">
                    <h3>
                      <RouterLink :to="`/${row.date}`">{{ row.title }}</RouterLink>
                    </h3>
                    <p class="muted when">
                      <time :datetime="row.date">{{ formatDate(row.date) }}</time>
                      <span v-if="row.dates.length > 1" class="tag">
                        <i aria-hidden="true" class="pi pi-replay" /> shown
                        {{ row.dates.length }}&times;
                      </span>
                    </p>

                    <RatingOdds
                      :comparisons="row.comparisons + Math.round(row.inherited ?? 0)"
                      :lower="row.lower"
                      :score="row.score"
                      :upper="row.upper"
                      class="odds"
                    />

                    <p v-if="row.credit?.length" class="muted credit-lines">
                      <span v-for="line in row.credit" :key="line">{{ line }}</span>
                    </p>
                  </div>
                </article>
              </li>
            </ul>
          </li>
        </ol>

        <Paginator
          v-if="board.ranked > PAGE_SIZE"
          :first="first"
          :page-link-size="pageLinks"
          :rows="PAGE_SIZE"
          :total-records="board.ranked"
          @page="onPage"
        />
      </template>
    </template>

    <RatingHelp v-model:visible="helpOpen" />
  </div>
</template>

<style scoped>
.board {
  gap: 1.1rem;
}

.head {
  gap: 0.75rem;
  align-items: center;
  flex-wrap: wrap;
}

h1 {
  font-size: 1.6rem;
  text-wrap: balance;
  margin-right: auto;
}

.tools {
  gap: 0.5rem;
  flex-wrap: wrap;
}

.pick-label {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
}

.pick-label i {
  font-size: 0.85em;
}

.help {
  display: grid;
  place-items: center;
  width: 2rem;
  height: 2rem;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: none;
  color: var(--text-muted);
  font-size: 1.05rem;
  cursor: pointer;
}

.help:hover,
.help:focus-visible {
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 10%, transparent);
}

.state {
  padding: 0.95rem 1.15rem 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.7rem;
}

.banner {
  gap: 0.6rem;
  flex-wrap: wrap;
}

.ask {
  margin: 0;
  flex: 1 1 16rem;
  font-size: 0.95rem;
  font-weight: 600;
  text-wrap: pretty;
}

.asked {
  margin: 0;
  font-size: 0.82rem;
  font-variant-numeric: tabular-nums;
}

.call {
  gap: 0.8rem;
  flex-wrap: wrap;
  padding: 0.7rem 0 0.15rem;
}

.call :deep(.p-button) {
  font-weight: 600;
}

.facts {
  display: flex;
  gap: 1.5rem;
  margin: 0;
  flex-wrap: wrap;
}

.facts dt {
  font-size: 0.68rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.facts dd {
  margin: 0;
  font-size: 0.92rem;
  font-variant-numeric: tabular-nums;
}

.credit {
  font-size: 0.85rem;
}

.empty {
  margin: 0;
  text-wrap: pretty;
}

.tiers {
  display: flex;
  flex-direction: column;
  gap: 1.4rem;
  list-style: none;
  margin: 0;
  padding: 0;
}

.tier {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.tier-head {
  position: sticky;
  top: var(--header-h);
  z-index: 3;
  display: flex;
  align-items: baseline;
  gap: 0.6rem;
  margin: 0;
  padding: 0.45rem 0 0.35rem;
  border-bottom: 1px solid var(--border);
  background: color-mix(in srgb, var(--bg) 88%, transparent);
  backdrop-filter: blur(10px);
  flex-wrap: wrap;
}

.tier-name {
  font-size: 0.95rem;
  font-weight: 650;
}

.tier-note {
  font-size: 0.78rem;
}

.cards {
  list-style: none;
  margin: 0;
  padding: 0;
  grid-template-columns: repeat(auto-fill, minmax(min(100%, 15rem), 1fr));
}

.entry {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.thumb {
  position: relative;
  display: block;
  aspect-ratio: 4 / 3;
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.thumb img,
.fallback {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: grid;
  place-items: center;
  color: var(--text-muted);
  font-size: 1.6rem;
}

.entry .body {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  padding: 0.7rem 0.85rem 0.8rem;
  min-width: 0;
}

h3 {
  font-size: 0.95rem;
  line-height: 1.3;
  text-wrap: balance;
}

h3 a {
  color: inherit;
  text-decoration: none;
}

h3 a:hover {
  color: var(--accent);
}

.when {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  margin: 0;
  font-size: 0.78rem;
}

.tag {
  display: inline-flex;
  align-items: center;
  gap: 0.2rem;
  font-size: 0.7rem;
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 0 0.4rem;
}

.tag i {
  font-size: 0.7em;
}

.odds {
  margin-top: 0.3rem;
}

.credit-lines {
  display: flex;
  flex-direction: column;
  gap: 0.05rem;
  margin: 0.15rem 0 0;
  font-size: 0.72rem;
  line-height: 1.35;
  text-wrap: pretty;
}
</style>
