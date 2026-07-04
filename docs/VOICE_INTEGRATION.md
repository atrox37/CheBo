# Chebo 语音接入指南

## 架构

- Rust `voice.rs`：OpenAI 兼容 TTS/STT HTTP 调用
- Tauri 命令：`voice_get_config` / `voice_update_config` / `voice_synthesize` / `voice_transcribe`
- 前端：`stores/voice.ts`、`composables/useVoice.ts`、设置页开关、输入框麦克风

## 配置（工作台 - 设置 - 语音）

| 项 | 说明 |
|----|------|
| 朗读回复 | 助手说完后播放 MP3，口型时长对齐音频 |
| 语音输入 | 输入框按住麦克风，松开后 Whisper 转文字 |
| API Base URL | 默认 `https://api.openai.com/v1`，可填代理/兼容网关 |
| 语音 API Key | 可空；空则复用 LLM Key |

## 使用步骤

1. 先配置 LLM Key（设置页）
2. 打开 TTS/STT 开关并保存
3. 桌宠模式：双击聊天，助手回复后可朗读
4. 启用 STT 后，输入框旁出现麦克风，按住说话

## 扩展

- 流式 TTS、本地 whisper.cpp、唤醒词、唇形同步见 Phase C 规划
