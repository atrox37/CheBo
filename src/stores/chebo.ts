import { defineStore } from 'pinia'
import { ref } from 'vue'
import { CRYSTAL_GIRL_BASE } from '@/config/crystalGirl'

export const useCheboStore = defineStore('chebo', () => {
  /** 是否使用 CrystalGirl PNGTuber 资源（桌宠主界面） */
  const useCrystalGirl = ref(true)

  /** 自定义立绘路径；CrystalGirl 模式下由 composable 驱动，此项作回退 */
  const characterImage = ref<string>(
    `${CRYSTAL_GIRL_BASE}/CrystalGirl_NeutralIdle.png`,
  )

  const currentMotion = ref<string>('idle')
  const currentExpression = ref<string>('normal')

  function setMotion(motion: string) {
    currentMotion.value = motion
  }

  function setExpression(expr: string) {
    currentExpression.value = expr
  }

  function setCharacterImage(path: string) {
    characterImage.value = path
  }

  return {
    useCrystalGirl,
    characterImage,
    currentMotion,
    currentExpression,
    setMotion,
    setExpression,
    setCharacterImage,
  }
})
