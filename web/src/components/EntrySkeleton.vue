<template>
  <div class="entry" aria-busy="true" aria-label="Loading entry">
    <Skeleton width="8rem" height="0.8rem" />
    <Skeleton width="60%" height="2rem" />

    <div class="layout">
      <div class="stack">
        <Skeleton width="100%" height="22rem" />
        <div class="row">
          <Skeleton width="5.5rem" height="2rem" />
          <Skeleton width="7rem" height="2rem" />
          <Skeleton width="6.5rem" height="2rem" />
        </div>
      </div>

      <div class="stack lines">
        <Skeleton width="100%" height="0.95rem" />
        <Skeleton width="100%" height="0.95rem" />
        <Skeleton width="100%" height="0.95rem" />
        <Skeleton width="45%" height="0.95rem" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.entry {
  display: flex;
  flex-direction: column;
  gap: var(--gap);
}

.layout {
  display: grid;
  gap: var(--gap);
  align-items: start;
}

.stack {
  gap: 0.9rem;
}

.lines {
  gap: 0.6rem;
}

@media (min-width: 62rem) {
  .layout {
    grid-template-columns: minmax(0, 1.15fr) minmax(0, 1fr);
    gap: 2rem;
  }
}
</style>
