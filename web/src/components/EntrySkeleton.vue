<template>
  <div class="stack" aria-busy="true" aria-label="Loading entry">
    <div class="skeleton line date" />
    <div class="skeleton line title" />
    <div class="skeleton media" />
    <div class="skeleton line" />
    <div class="skeleton line" />
    <div class="skeleton line" />
    <div class="skeleton line short" />
  </div>
</template>

<style scoped>
.line {
  height: 0.95rem;
}

.line.date {
  width: 8rem;
  height: 0.8rem;
}

.line.title {
  width: 60%;
  height: 2rem;
  margin-bottom: 0.5rem;
}

.line.short {
  width: 45%;
}

.media {
  aspect-ratio: 16 / 10;
  width: 100%;
}
</style>
