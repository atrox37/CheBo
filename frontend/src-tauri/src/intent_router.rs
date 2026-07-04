// ─── intent_router.rs ────────────────────────────────────────────────────────
// 语义工具路由器（三级递进）
//
// 解决问题：用户不需要说"用搜索工具"，系统自动根据消息内容决定
//           把哪些工具注入到 LLM 的 system prompt 中。
//
// 三级路由：
//
//   Tier 1 — 关键词规则（~60% 覆盖）
//     精确关键词/前缀匹配 → 直接命中工具列表
//     例："最近发生什么" → ["web_search"]
//
//   Tier 2 — 启发式语义评分（~25% 额外覆盖）
//     即使没有关键词，通过语言特征（问号/时态/文件扩展名/路径格式）打分
//     例："这个.rs 文件报错了" → read_file 得分高 → ["read_file"]
//
//   Tier 3 — Fallback（~15%）
//     两级都没有高置信度 → 返回空列表（调用方注入全量工具，让 LLM 自决）
//
// 返回空列表 = 使用所有工具（完全由 LLM 决定）
// 返回非空列表 = 只注入这些工具（减少 prompt token、提升 LLM 专注度）
// ─────────────────────────────────────────────────────────────────────────────
#![allow(dead_code)]

use std::collections::HashMap;

// ─── 返回类型 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum HintSource {
    /// 精确关键词命中
    Keyword,
    /// 启发式特征评分
    Heuristic,
    /// 对话上下文推断
    Context,
    /// 无明显意图，回退到全量工具
    Fallback,
}

#[derive(Debug, Clone)]
pub struct ToolHint {
    /// 推荐注入的工具名列表；空 = 全量工具
    pub tools: Vec<String>,
    /// 置信度 0.0–1.0
    pub confidence: f32,
    /// 命中来源
    pub source: HintSource,
}

impl ToolHint {
    pub fn all() -> Self {
        Self { tools: vec![], confidence: 0.0, source: HintSource::Fallback }
    }

    pub fn keyword(tools: Vec<&str>) -> Self {
        Self {
            tools:      tools.iter().map(|s| s.to_string()).collect(),
            confidence: 1.0,
            source:     HintSource::Keyword,
        }
    }

    /// 是否使用全量工具（即不需要筛选）
    pub fn use_all_tools(&self) -> bool {
        self.tools.is_empty()
    }
}

// ─── Tier 1：关键词规则表 ────────────────────────────────────────────────────
//
// 格式：(关键词列表, 对应工具列表)
// 匹配规则：消息中包含任意一个关键词即命中

static KEYWORD_RULES: &[(&[&str], &[&str])] = &[
    // 网络搜索意图
    (
        &[
            "搜索", "查一下", "查查", "搜一下", "查找", "百度", "谷歌",
            "最新", "新闻", "什么是", "谁是", "哪里有", "如何", "怎么",
            "为什么", "是什么", "有没有", "能否", "介绍一下",
            "今天发生", "最近发生", "现在的", "当前的",
            "帮我了解", "帮我查",
        ],
        &["web_search"],
    ),
    // 文件读取意图
    (
        &[
            "文件", "读取", "打开", "看看代码", "查看代码",
            ".rs", ".py", ".ts", ".tsx", ".vue", ".js", ".jsx",
            ".json", ".toml", ".yaml", ".yml", ".md", ".txt",
            ".env", ".conf", ".cfg", ".ini", ".lock",
            "源码", "源文件",
        ],
        &["read_file"],
    ),
    // 目录浏览意图
    (
        &[
            "目录", "文件夹", "列出", "有哪些文件", "看看结构", "项目结构",
        ],
        &["list_dir", "read_file"],
    ),
    // Git 意图
    (
        &[
            "git", "提交", "commit", "分支", "branch", "仓库",
            "合并", "merge", "变更", "diff", "暂存", "stash",
            "最近的提交", "最新的提交",
        ],
        &["git_status"],
    ),
    // 剪贴板意图
    (
        &[
            "剪贴板", "复制的内容", "刚才复制", "我复制了",
            "粘贴", "clipboard",
        ],
        &["clipboard_read"],
    ),
    // 记忆回溯意图
    (
        &[
            "之前说过", "我记得", "你记得", "历史记录", "曾经说",
            "记忆", "回忆", "你知道我", "我之前", "我上次",
        ],
        &["memory_recall"],
    ),
    // Shell 执行意图（L3，高危，但让 LLM 判断是否真的需要）
    (
        &[
            "执行命令", "运行脚本", "终端", "shell命令",
            "cargo run", "cargo build", "pnpm", "npm run",
            "python ", "node ", "运行程序",
        ],
        &["safe_shell"],
    ),
];

// ─── Tier 2：启发式评分特征 ──────────────────────────────────────────────────

struct HeuristicFeature {
    /// 检测函数
    check:       fn(&str) -> bool,
    /// 命中时给哪些工具加分
    tool_scores: &'static [(&'static str, f32)],
}

// 编译期静态特征表
fn heuristic_features() -> Vec<HeuristicFeature> {
    vec![
        // 含问号 + 时间词 → 很可能要搜索
        HeuristicFeature {
            check: |m| {
                (m.contains('?') || m.contains('？'))
                    && (m.contains("最近") || m.contains("现在") || m.contains("今天")
                        || m.contains("最新") || m.contains("这几天"))
            },
            tool_scores: &[("web_search", 0.8)],
        },
        // 含问号（事实型问题）
        HeuristicFeature {
            check: |m| m.contains('?') || m.contains('？'),
            tool_scores: &[("web_search", 0.4)],
        },
        // 含文件扩展名（带点的）
        HeuristicFeature {
            check: |m| {
                let exts = [".rs", ".py", ".ts", ".tsx", ".vue", ".js",
                            ".json", ".toml", ".yaml", ".yml", ".md", ".txt"];
                exts.iter().any(|e| m.contains(e))
            },
            tool_scores: &[("read_file", 0.8), ("list_dir", 0.3)],
        },
        // 含路径分隔符（看起来像文件路径）
        HeuristicFeature {
            check: |m| {
                (m.contains('/') || m.contains('\\'))
                    && (m.contains('.') || m.len() > 20)
            },
            tool_scores: &[("read_file", 0.6), ("list_dir", 0.4)],
        },
        // 含 "帮我" + 动词 → 可能需要工具辅助完成任务
        HeuristicFeature {
            check: |m| {
                m.contains("帮我") && (
                    m.contains("写") || m.contains("创建") || m.contains("生成")
                    || m.contains("修改") || m.contains("更新")
                )
            },
            tool_scores: &[("read_file", 0.4), ("safe_shell", 0.3)],
        },
        // 纯聊天（短消息、无问号、无任务词）→ 不需要工具
        HeuristicFeature {
            check: |m| {
                let len = m.chars().count();
                len < 12 && !m.contains('?') && !m.contains('？')
            },
            tool_scores: &[], // 空 = 不推荐工具
        },
    ]
}

// ─── 主路由入口 ───────────────────────────────────────────────────────────────

/// 分析用户消息，返回建议注入的工具集合
///
/// `message`      — 用户当前输入的原文
/// `recent_tools` — 上一轮对话中已使用的工具名（用于上下文连续性判断）
pub fn route(message: &str, recent_tools: &[String]) -> ToolHint {
    let msg = message.trim();

    // ── Tier 1: 关键词精确匹配 ────────────────────────────────────────────────
    for (keywords, tools) in KEYWORD_RULES {
        for kw in *keywords {
            if msg.contains(kw) {
                return ToolHint::keyword(tools.to_vec());
            }
        }
    }

    // ── Tier 2: 启发式评分 ────────────────────────────────────────────────────
    let mut scores: HashMap<&str, f32> = HashMap::new();
    let mut no_tool_signal = false;

    for feat in heuristic_features() {
        if (feat.check)(msg) {
            if feat.tool_scores.is_empty() {
                no_tool_signal = true;
            } else {
                for (tool, score) in feat.tool_scores {
                    *scores.entry(tool).or_insert(0.0) += score;
                }
            }
        }
    }

    // 如果命中"纯聊天"信号且无其他工具得分 → 不注入工具
    if no_tool_signal && scores.is_empty() {
        return ToolHint {
            tools:      vec![],
            confidence: 0.9,
            source:     HintSource::Heuristic,
        };
    }

    // 找出得分 > 0.6 的工具
    let mut high_score_tools: Vec<(&str, f32)> = scores
        .iter()
        .filter(|(_, &s)| s > 0.6)
        .map(|(&t, &s)| (t, s))
        .collect();
    high_score_tools.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    if !high_score_tools.is_empty() {
        let confidence = high_score_tools[0].1.min(1.0);
        let tools: Vec<String> = high_score_tools.iter().map(|(t, _)| t.to_string()).collect();
        return ToolHint {
            tools,
            confidence,
            source: HintSource::Heuristic,
        };
    }

    // ── Tier 2.5: 上下文连续性 ────────────────────────────────────────────────
    // 如果上轮用了某个工具且本轮消息是短回复，可能在追问同一话题
    if !recent_tools.is_empty() && msg.chars().count() < 30 {
        let context_tools = recent_tools.to_vec();
        return ToolHint {
            tools:      context_tools,
            confidence: 0.5,
            source:     HintSource::Context,
        };
    }

    // ── Tier 3: Fallback — 全量工具交给 LLM 自决 ─────────────────────────────
    ToolHint::all()
}

// ─── 测试 ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_search() {
        let hint = route("帮我查一下 Rust 最新的稳定版本", &[]);
        assert!(hint.tools.contains(&"web_search".to_string()));
        assert!(matches!(hint.source, HintSource::Keyword));
    }

    #[test]
    fn test_heuristic_file() {
        let hint = route("这个 main.rs 文件有问题", &[]);
        assert!(hint.tools.contains(&"read_file".to_string()));
    }

    #[test]
    fn test_heuristic_question_with_time() {
        let hint = route("最近有什么值得关注的技术新动态？", &[]);
        assert!(hint.tools.contains(&"web_search".to_string()));
    }

    #[test]
    fn test_short_chat_no_tools() {
        let hint = route("嗯嗯", &[]);
        assert!(hint.tools.is_empty() || hint.confidence < 0.7);
    }

    #[test]
    fn test_git_keyword() {
        let hint = route("帮我看看当前 git 状态", &[]);
        assert!(hint.tools.contains(&"git_status".to_string()));
    }
}
