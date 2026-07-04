<script setup lang="ts">
import { computed } from 'vue'
import { usePetStore } from '@/stores/pet'

const pet = usePetStore()

const expPct = computed(() => Math.min(100, Math.round(pet.expProgress * 100)))
</script>

<template>
  <div class="status-bar" @click.stop @mousedown.stop>
    <!-- 饱腹 -->
    <div class="stat" title="饱腹度">
      <span class="ico">🍜</span>
      <div class="track"><div class="fill food"  :style="{ width: pet.hunger + '%' }" /></div>
    </div>
    <!-- 精力 -->
    <div class="stat" title="精力">
      <span class="ico">⚡</span>
      <div class="track"><div class="fill energy" :style="{ width: pet.energy + '%' }" /></div>
    </div>
    <!-- 心情 -->
    <div class="stat" title="心情">
      <span class="ico">❤</span>
      <div class="track"><div class="fill mood"   :style="{ width: pet.mood + '%' }" /></div>
    </div>
    <!-- 等级 + 经验 -->
    <div class="lv-exp" :title="`Lv${pet.level}  ${pet.exp}/${pet.nextLevelExp} exp`">
      <span class="lv-badge">Lv{{ pet.level }}</span>
      <div class="exp-track"><div class="exp-fill" :style="{ width: expPct + '%' }" /></div>
    </div>
    <!-- 金币 -->
    <div class="coins" title="金币">
      <span class="ico">💰</span>
      <span class="num">{{ Math.floor(pet.coins) }}</span>
    </div>
  </div>
</template>

<style scoped>
.status-bar {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 4px 10px;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  border-radius: 20px;
  border: 1px solid rgba(255,255,255,0.08);
}

.stat {
  display: flex;
  align-items: center;
  gap: 2px;
}

.ico  { font-size: 9px; line-height: 1; }

.track {
  width: 36px;
  height: 3px;
  background: rgba(255,255,255,0.12);
  border-radius: 2px;
  overflow: hidden;
}
.fill {
  height: 100%;
  border-radius: 2px;
  transition: width 0.6s ease;
}
.fill.food   { background: #ffa94d; }
.fill.energy { background: #74c0fc; }
.fill.mood   { background: #ff6b9d; }

.lv-exp {
  display: flex;
  align-items: center;
  gap: 2px;
}
.lv-badge {
  font-size: 8.5px;
  font-weight: 700;
  color: #c0a0ff;
  white-space: nowrap;
}
.exp-track {
  width: 24px;
  height: 3px;
  background: rgba(255,255,255,0.12);
  border-radius: 2px;
  overflow: hidden;
}
.exp-fill {
  height: 100%;
  background: #c0a0ff;
  border-radius: 2px;
  transition: width 0.4s ease;
}

.coins {
  display: flex;
  align-items: center;
  gap: 2px;
}
.num {
  font-size: 9px;
  font-weight: 700;
  color: #ffd700;
  min-width: 20px;
}
</style>
