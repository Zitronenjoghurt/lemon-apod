<script lang="ts" setup>
import { computed, ref, watch } from 'vue'
import { useConfirm } from 'primevue/useconfirm'
import { useToast } from 'primevue/usetoast'
import type { GameSlug } from '@/api/types'
import { copyText, type GameResult, shareText, summarise, useGame } from '@/composables/useGames'
import { useNarrow } from '@/composables/useNarrow'

const props = defineProps<{ slug: GameSlug; title: string }>()

const PAGE = 10

const { history, clear } = useGame(props.slug)
const { pageLinks } = useNarrow()
const confirm = useConfirm()
const toast = useToast()

const dailies = computed(() => history.value.filter((result) => result.day))
const frees = computed(() => history.value.filter((result) => !result.day))

const daily = computed(() => summarise(dailies.value))
const free = computed(() => summarise(frees.value))

type Filter = 'all' | 'daily' | 'free'

const filters: { label: string; value: Filter }[] = [
  { label: 'All', value: 'all' },
  { label: 'Daily', value: 'daily' },
  { label: 'Free play', value: 'free' },
]

const filter = ref<Filter>('all')
const first = ref(0)

const listed = computed(() =>
  filter.value === 'daily' ? dailies.value : filter.value === 'free' ? frees.value : history.value,
)
const page = computed(() => listed.value.slice(first.value, first.value + PAGE))

watch(listed, () => {
  first.value = 0
})

function when(result: { day?: string; at: string }): string {
  return result.day ?? new Date(result.at).toISOString().slice(0, 10)
}

async function copy(result: GameResult): Promise<void> {
  const copied = await copyText(shareText(props.slug, result))

  toast.add({
    severity: copied ? 'success' : 'warn',
    summary: copied ? 'Copied' : 'Could not copy',
    detail: copied ? `${props.title}, ${when(result)}` : 'Your browser would not allow it.',
    life: 2000,
  })
}

function confirmClear() {
  confirm.require({
    header: `Forget your ${props.title} history?`,
    message: `This removes all ${history.value.length} recorded games from this browser, streak included. There is no undo.`,
    icon: 'pi pi-exclamation-triangle',
    rejectProps: { label: 'Cancel', severity: 'secondary', outlined: true },
    acceptProps: { label: 'Forget it', severity: 'danger' },
    accept: () => {
      clear()
      toast.add({ severity: 'success', summary: 'History cleared', life: 2000 })
    },
  })
}
</script>

<template>
  <section class="card record">
    <h2>Your record</h2>

    <div class="split">
      <div class="side">
        <h3><i aria-hidden="true" class="pi pi-calendar" /> Daily</h3>
        <div class="figures">
          <div class="figure">
            <strong>{{ daily.played.toLocaleString() }}</strong>
            <span class="muted">played</span>
          </div>
          <div class="figure">
            <strong>{{ daily.streak }}</strong>
            <span class="muted">day streak</span>
          </div>
          <div class="figure">
            <strong>{{ daily.longest }}</strong>
            <span class="muted">longest</span>
          </div>
          <div v-if="daily.solvable" class="figure">
            <strong>{{ daily.wins }}</strong>
            <span class="muted">solved</span>
          </div>
        </div>
        <p class="best">
          <span class="muted">Best</span>
          <strong>{{ daily.best?.label ?? 'none yet' }}</strong>
        </p>
      </div>

      <div class="side">
        <h3><i aria-hidden="true" class="pi pi-sync" /> Free play</h3>
        <div class="figures">
          <div class="figure">
            <strong>{{ free.played.toLocaleString() }}</strong>
            <span class="muted">played</span>
          </div>
          <div v-if="free.solvable" class="figure">
            <strong>{{ free.wins }}</strong>
            <span class="muted">solved</span>
          </div>
        </div>
        <p class="best">
          <span class="muted">Best</span>
          <strong>{{ free.best?.label ?? 'none yet' }}</strong>
        </p>
      </div>
    </div>

    <div v-if="history.length" class="history">
      <div class="row history-head">
        <h3>History</h3>
        <SelectButton
          v-model="filter"
          :allow-empty="false"
          :options="filters"
          aria-label="Which games to list"
          option-label="label"
          option-value="value"
          size="small"
        />
      </div>

      <table class="games">
        <caption class="sr-only">
          Every game this browser has kept
        </caption>
        <tbody>
          <tr v-for="result in page" :key="result.id">
            <td class="stamp muted">{{ when(result) }}</td>
            <td class="kind">
              <i
                v-tooltip.top="result.day ? 'Daily' : 'Free play'"
                :class="['pi', result.day ? 'pi-calendar' : 'pi-sync']"
                aria-hidden="true"
              />
            </td>
            <td class="outcome">{{ result.label }}</td>
            <td class="marks">
              <button
                v-tooltip.left="'Copy this result'"
                class="copy"
                type="button"
                @click="copy(result)"
              >
                <GameBands
                  v-if="result.bands?.length"
                  :bands="result.bands"
                  class="squares"
                  size="small"
                />
                <i aria-hidden="true" class="pi pi-clipboard" />
                <span class="sr-only">Copy this result</span>
              </button>
            </td>
          </tr>
        </tbody>
      </table>

      <p v-if="!page.length" class="muted empty">You haven't played this mode yet.</p>

      <Paginator
        v-if="listed.length > PAGE"
        :first="first"
        :page-link-size="pageLinks"
        :rows="PAGE"
        :total-records="listed.length"
        @page="first = $event.first"
      />
    </div>
    <p v-else class="muted empty">You haven't played any mode yet.</p>

    <footer v-if="history.length" class="row foot">
      <Button
        icon="pi pi-trash"
        label="Clear history"
        severity="danger"
        size="small"
        text
        @click="confirmClear"
      />
    </footer>
  </section>
</template>

<style scoped>
.record {
  padding: var(--space-4) var(--space-5) var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

h2 {
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.07em;
  color: var(--text-muted);
  font-weight: 600;
}

h3 {
  font-size: var(--text-sm);
  font-weight: 600;
  display: flex;
  align-items: center;
  gap: var(--space-1);
}

h3 i {
  font-size: 0.85em;
  color: var(--accent);
}

.split {
  display: grid;
  gap: var(--space-4);
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 14rem), 1fr));
}

.side {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  border: 1px solid var(--border);
  border-radius: var(--radius);
}

.figures {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(4.5rem, 1fr));
  gap: var(--space-3) var(--space-4);
}

.figure {
  display: flex;
  flex-direction: column;
  line-height: 1.25;
}

.figure strong {
  font-size: var(--text-lg);
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.02em;
}

.figure span {
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.best {
  display: flex;
  flex-direction: column;
  gap: var(--space-0);
  margin: auto 0 0;
  font-size: var(--text-md);
}

.best span {
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.history {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.history-head {
  justify-content: space-between;
  gap: var(--space-3);
}

.games {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--text-sm);
}

.games td {
  padding: var(--space-2) var(--space-5) var(--space-2) 0;
  border-top: 1px solid var(--border);
  vertical-align: middle;
}

.games td:last-child {
  padding-right: 0;
}

.stamp {
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  width: 1%;
}

.kind {
  width: 1%;
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.marks {
  width: 1%;
  text-align: right;
  white-space: nowrap;
}

.copy {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-1);
  margin-right: -0.3rem;
  border: 0;
  border-radius: 0.4rem;
  background: none;
  color: var(--text-muted);
  cursor: pointer;
}

.copy .pi {
  font-size: var(--text-xs);
  opacity: 0.45;
  transition: opacity 0.15s ease;
}

.copy:hover,
.copy:focus-visible {
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.copy:hover .pi,
.copy:focus-visible .pi {
  opacity: 1;
  color: var(--accent);
}

.empty {
  margin: 0;
  font-size: var(--text-sm);
}

.foot {
  justify-content: space-between;
  gap: var(--space-2);
  border-top: 1px solid var(--border);
  padding-top: var(--space-2);
}

:deep(.p-paginator) {
  background: transparent;
  padding: var(--space-1) 0;
}

@media (max-width: 32rem) {
  .games {
    font-size: var(--text-sm);
  }

  .games td {
    padding-right: var(--space-2);
  }

  .squares {
    display: none;
  }

  .copy {
    justify-content: flex-end;
    min-width: 2.5rem;
    padding: var(--space-2) var(--space-2);
    margin: -0.45rem -0.4rem -0.45rem 0;
  }
}
</style>
