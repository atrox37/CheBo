from pathlib import Path
p = Path("frontend/src-tauri/src/commands.rs")
t = p.read_text(encoding="utf-8")
start = "// \u2500\u2500\u2500 \u684c\u5ba0\u65e5\u5e38\u4e92\u52a8"
end = "// \u2500\u2500\u2500 \u83b7\u53d6/\u66f4\u65b0 App \u914d\u7f6e"
i, j = t.find(start), t.find(end)
if i == -1 or j == -1:
    raise SystemExit(f"markers not found {i} {j}")
t = t[:i] + t[j:]
block = (
    "/// \u8bbe\u7f6e\u5173\u6000/\u9759\u9ed8\u6a21\u5f0f\n"
    "#[tauri::command]\n"
    "pub fn set_care_mode(state: State<'_, AppState>, enabled: bool) -> CmdResult<()> {\n"
    "    let mut guard = state.care_mode.lock().map_err(e)?;\n"
    "    *guard = enabled;\n"
    "    log::info!(\"care_mode -> {}\", enabled);\n"
    "    Ok(())\n"
    "}\n\n"
)
t = t.replace(block, "")
old_imp = "use crate::db::{self, Food, InventoryItem, Message, PetStatus, StatusPatch, Task};"
new_imp = "use crate::db::{self, Message, PetStatus, StatusPatch};"
t = t.replace(old_imp, new_imp)
p.write_text(t, encoding="utf-8", newline="\n")
print("ok", p.stat().st_size)
