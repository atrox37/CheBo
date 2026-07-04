# Chebo 桌宠最终形态：Ambient Agent 技术基准
> **版本**：v3.0 Ambient Agent（2026-07-03）
> **状态**：架构裁剪已落地；本文档为桌宠形态后续开发的唯一基准
> **原则**：AI 能力是核心，桌宠形态是入口，拟人化表现是体验，主工作台承接复杂操作

---

## 1. 一句话定位

**不要做成传统养宠物系统，而是做成一个有桌宠外壳、有情绪反馈、有轻度趣味的 AI 桌面智能体。**

| 传统桌宠 | Chebo Ambient Agent |
|----------|---------------------|
| 用户照顾宠物（喂食、赚金币、升级） | 桌宠帮助用户（聊天、提醒、判断、轻量执行） |
| 数值条驱动体验 | Agent 状态 + 情绪驱动体验 |
| 定时自言自语 | 事件驱动主动智能（有理由才开口） |
| 面板是主入口 | 对话与感知是主入口，面板是监视器 |

---

## 2. 已清除的传统模式

### 2.1 后端后台循环（pet.rs 已清空）

| 已删除 | 原作用 | 替代 |
|--------|--------|------|
| tick_loop | 定时衰减饥饿/精力/心情 | 无自动衰减；DB 字段保留供迁移 |
| ai_comment_loop | 定时调用 LLM 主动发言 | **禁止**；仅 Agent 任务/用户触发可发气泡 |
| task_watch_loop | 宠物学习/工作倒计时任务 | Agent 长期任务见 task/ 模块 |

### 2.2 已注销的 Tauri 命令

feed / pet_action / buy_item / get_foods / get_inventory / get_tasks / start_task / cancel_task / set_keep_perfect / set_care_mode 等养成命令已从 invoke_handler 移除。

DB 表（foods / tasks / inventory）暂不删除，新逻辑不读写。

### 2.3 前端 UI 裁剪

桌宠模式仅保留：立绘 + 气泡 + 底部「工作台」切换。伙伴/设置/任务/记忆均在工作台模式。

已删除面板：FeedPanel / ShopPanel / ActionPanel / StatusPanel / LevelUpToast。

### 2.4 主动发言策略

| 事件 | 气泡 |
|------|------|
| 用户消息 -> LLM 回复 | 是 |
| Agent 任务进度/完成 | 是 |
| 睡眠/托盘 about 等 | 否 |
| 定时 LLM 自言自语 | 否，永久禁止 |

---

## 3. 双模式架构

桌宠模式：立绘、智能气泡、双击输入、L0-L3 确认、底部工作台入口。

助手模式：完整聊天、记忆树、Agent 任务、模型与沙盒配置。

复杂事项通过 open_assistant 跳转大窗。

---

## 4. 核心状态模型

### AgentState（camelCase）

- `idle`：待机
- `thinking`：思考
- `talking`：说话
- `executingTool`：工具执行
- `waitingConfirm`：等待确认
- `working`：任务中
- `sleeping`：休息
- `observing`：感知
- `errorRecover`：错误恢复

立绘主绑定 agentState + emotion，不再绑定饥饿/精力条。

### Emotion

[EMOTION:xxx] 标签解析驱动表情。Phase B：流式剥离标签。

### 默契度 affection

唯一可见软数值，影响语气亲密度。

---

## 5. L0-L3 执行策略

L0 静默自动；L1 自动+气泡；L2/L3 确认框。

---

## 6. 主动智能（非定时）

感知变化、Agent 任务生命周期、用户提醒、一次性系统通知。

---

## 7. 聊天管线

send_message -> build_system_prompt -> build_rich_context_string -> stream_agent_turn -> assistant_chunk/done

---

## 8-9. 模块映射

前端：App / CompanionPanel / AssistantLayout / chat.ts / tauriService.ts

后端：agent / commands / character / pet(空) / task / perception / memory_vector
---

## 10. Phase B 详细规划（2026-07-03）

Phase B 聚焦体验打磨与界面分工。

### 10.1 双模式 UI 重构（已落地）

| 区域 | 桌宠模式 | 工作台模式 |
|------|----------|------------|
| 核心 | 立绘+气泡+双击输入 | 聊天/任务/记忆/设置 |
| 伙伴/设置 | 不展示 | 左侧导航页 |
| 模式切换 | 底部「工作台」 | 底部「返回桌宠」 |

### 10.2 气泡自动隐藏（已落地）

- 普通聊天：说完口型尾韵后再等 3.5s 淡出
- 确认类：waitingConfirm / toolConfirmOpen）：气泡保持直到用户操作
- 字段：petBubbleVisible, bubblePinned, PET_BUBBLE_LINGER_MS=3500

### 10.3 流式剥离 [EMOTION]（待实现）

流式过程中剥离标签，情绪仅写入 currentEmotion。

### 10.4 立绘绑定全 agentState（待实现）

idle/thinking/talking/executingTool/waitingConfirm/working/sleeping/observing/errorRecover 各有独立姿态。

### 10.5 默契度微增（待实现）

有意义对话每轮 +0.2，上限 100。

### 10.6 死代码清理

已完成 commands 养成函数、keep_perfect/care_mode。待完成 pet.ts 缓存。

### 10.7 验收清单

- [x] 桌宠无侧栏伙伴/设置
- [x] 底部工作台切换
- [x] 普通气泡自动隐藏
- [x] 确认气泡保持
- [x] EMOTION 剥离
- [x] 立绘全状态绑定
- [x] 默契度微增

---

## 11. 阶段总览

Phase A ✅ | Phase B ✅ | Phase C/D 待定

---

## 12. 明确不做

重养成、定时自言自语、桌宠重配置、数值条替代 Agent 状态。

---

本文档为桌宠形态的唯一技术基准。
