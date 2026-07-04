<script setup lang="ts">
import { CHEBO_AVATAR, CHEBO_NAME } from '@/config/chebo'

withDefaults(defineProps<{
  size?: number
  alt?: string
  ring?: boolean
}>(), {
  size: 28,
  alt: CHEBO_NAME,
  ring: false,
})
</script>

<template>
  <div class="chebo-avatar" :class="{ ring }" :style="{ width: size + 'px', height: size + 'px' }">
    <img :src="CHEBO_AVATAR" :alt="alt" class="chebo-avatar-img" draggable="false" />
  </div>
</template>

<style scoped>
.chebo-avatar {
  flex-shrink: 0;
  border-radius: 50%;
  overflow: hidden;
  background: #fff;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}
.chebo-avatar.ring {
  box-shadow: 0 0 0 2px #fff, 0 0 0 3px rgba(232, 112, 144, 0.45);
}
.chebo-avatar-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  object-position: top center;
}
</style>
