from pathlib import Path
Path("frontend/src/config/chebo.ts").write_text("""import { CRYSTAL_GIRL_BASE } from './crystalGirl'

export const CHEBO_AVATAR = `${CRYSTAL_GIRL_BASE}/CrystalGirl_NeutralIdle.png`
export const CHEBO_NAME = 'Chebo'
""", encoding="utf-8", newline="\n")
print("chebo.ts ok")
