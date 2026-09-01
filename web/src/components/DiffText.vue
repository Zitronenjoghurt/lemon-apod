<script lang="ts" setup>
import { computed } from 'vue'
import type { Change, ChangeKind } from '@/utils/diff'

const props = defineProps<{ changes: Change[] }>()

interface Piece {
  kind: ChangeKind
  text: string
  tail: string
}

const pieces = computed<Piece[]>(() =>
  props.changes.map((change) => {
    const tail = /\s*$/.exec(change.text)?.[0] ?? ''
    return {
      kind: change.kind,
      text: change.text.slice(0, change.text.length - tail.length),
      tail,
    }
  }),
)
</script>

<template>
  <p class="text">
    <template v-for="(piece, index) in pieces" :key="index"
      ><del v-if="piece.kind === 'removed'">{{ piece.text }}</del
      ><ins v-else-if="piece.kind === 'added'">{{ piece.text }}</ins
      ><span v-else>{{ piece.text }}</span
      >{{ piece.tail }}</template
    >
  </p>
</template>

<style scoped>
.text {
  margin: 0;
  font-size: var(--text-sm);
  line-height: 1.55;
  text-wrap: pretty;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

ins,
del {
  border-radius: var(--radius-sm);
  padding: 0.02em 0.12em;
  text-decoration: none;
}

ins {
  background: color-mix(in srgb, var(--diff-added) 24%, transparent);
  box-shadow: inset 0 -0.12em color-mix(in srgb, var(--diff-added) 70%, transparent);
}

del {
  background: color-mix(in srgb, var(--diff-removed) 20%, transparent);
  text-decoration: line-through;
  text-decoration-thickness: 1px;
  opacity: 0.75;
}
</style>
