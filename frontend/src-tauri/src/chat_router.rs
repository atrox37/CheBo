// chat_router.rs — chat mode routing
#![allow(dead_code)]

pub fn should_run_agent_task(content: &str, deep_think: bool) -> bool {
    let c = content.trim();
    if c.len() < 10 {
        return false;
    }
    let strong = [
        "帮我整理", "帮我完成", "帮我处理", "帮我执行", "帮我写一份",
        "持续监控", "持续关注", "定期检查", "分步骤", "一步步",
        "多步", "整个文件夹", "整个项目", "批量处理", "制定计划",
        "完成以下", "按计划",
    ];
    if strong.iter().any(|s| c.contains(s)) {
        return true;
    }
    if deep_think {
        let action = ["帮我", "请帮", "整理", "分析", "总结", "生成", "导出"];
        if action.iter().any(|s| c.contains(s)) && c.len() >= 20 {
            return true;
        }
    }
    false
}

pub const DEEP_THINK_SYSTEM_ADDITION: &str = r#"
【深度思考】
用户希望更审慎地处理本条消息。请先拆解问题，必要时多轮调用工具，再给出结论。
"#;

pub const DEEP_THINK_MAX_TOOL_TURNS: usize = 14;
