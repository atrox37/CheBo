// 鈹€鈹€鈹€ tool_registry.rs 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
// Tool Registry锛氱粺涓€娉ㄥ唽琛?
#![allow(dead_code)]
//
// 宸ュ叿娉ㄥ唽 鈫?妯″瀷鍙 鈫?鎵ц锛堝弬鑰?OpenHuman all_tools_with_runtime锛?
//
// 宸ュ叿鍒楄〃锛堟寜绫诲埆锛夛細
//   鏂囦欢宸ュ叿:  read_file, write_file, list_dir, search_files, replace_in_file
//   绯荤粺宸ュ叿:  safe_shell, git_status, open_file, get_system_info, process_list, set_reminder
//   缃戠粶宸ュ叿:  web_search, web_fetch
//   璁板繂宸ュ叿:  memory_recall, note_take
//   鍓创鏉?   clipboard_read
//   濯掍綋:     take_screenshot
//
// 姣忎釜宸ュ叿閮藉疄鐜?Tool trait锛孴ool Registry 缁熶竴鎸佹湁銆?
// 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use sqlx::SqlitePool;

use crate::intent_router;
use crate::llm::LlmConfig;
use crate::memory_vector;
use crate::sandbox::SandboxPolicy;
use crate::tool_trait::{Tool, ToolCallResult, ToolCategory, ToolParam, ToolPermission};

// 鈹€鈹€鈹€ Registry 涓讳綋 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// 鑾峰彇鎵€鏈夊伐鍏风殑 spec 鍒楄〃锛堜緵鍓嶇鏄剧ず锛?
    pub fn all_specs(&self) -> Vec<crate::tool_trait::ToolSpec> {
        let mut specs: Vec<_> = self.tools.values().map(|t| t.spec()).collect();
        specs.sort_by(|a, b| a.name.cmp(&b.name));
        specs
    }

    /// 鐢熸垚娉ㄥ叆 system prompt 鐨勫伐鍏疯鏄庢枃鏈紙鍏ㄩ噺锛?
    pub fn tools_prompt_block(&self) -> String {
        self.build_prompt_block(self.tools.values().map(Arc::clone).collect())
    }

    /// 鏍规嵁鐢ㄦ埛娑堟伅璇箟锛屾櫤鑳介€夋嫨闇€瑕佹敞鍏ョ殑宸ュ叿锛堜笁绾ф剰鍥捐矾鐢憋級
    ///
    /// - 濡傛灉璺敱鍣ㄦ湁鏄庣‘寤鸿 鈫?鍙敞鍏ョ浉鍏冲伐鍏凤紙鍑忓皯 token 娑堣€楋級
    /// - 濡傛灉娌℃湁鏄庣‘寤鸿锛團allback锛夆啋 娉ㄥ叆鍏ㄩ噺宸ュ叿锛岃 LLM 鑷喅
    pub fn tools_prompt_block_for(&self, message: &str, recent_tools: &[String]) -> String {
        let hint = intent_router::route(message, recent_tools);

        if hint.use_all_tools() {
            // Fallback锛氬叏閲忓伐鍏?
            return self.tools_prompt_block();
        }

        // 杩囨护鍑哄懡涓殑宸ュ叿锛堝悎骞?hint + 濮嬬粓鍙鐨勫熀纭€宸ュ叿锛?
        let always_visible = ["memory_recall"]; // 璁板繂宸ュ叿鎬绘槸鍙
        let relevant: Vec<Arc<dyn Tool>> = self.tools
            .iter()
            .filter(|(name, _)| {
                hint.tools.iter().any(|h| h == name.as_str())
                    || always_visible.contains(&name.as_str())
            })
            .map(|(_, t)| Arc::clone(t))
            .collect();

        if relevant.is_empty() {
            return String::new(); // 绾亰澶╋紝涓嶆敞鍏ヤ换浣曞伐鍏?
        }

        self.build_prompt_block(relevant)
    }

    fn build_prompt_block(&self, tools: Vec<Arc<dyn Tool + 'static>>) -> String {
        let mut lines: Vec<String> = tools
            .iter()
            .map(|t| t.spec().one_line())
            .collect();
        lines.sort();

        format!(
            "## 鍙敤宸ュ叿\n\
             褰撲綘闇€瑕佹煡璇俊鎭垨鎵ц鎿嶄綔鏃讹紝鍙互璋冪敤浠ヤ笅宸ュ叿銆俓n\
             璋冪敤鏍煎紡锛歕n\
             <tool_call>\n\
             {{\"name\":\"宸ュ叿鍚峔",\"arguments\":{{\"鍙傛暟鍚峔":\"鍙傛暟鍊糪"}}}}\n\
             </tool_call>\n\
             宸ュ叿鎵ц瀹屾垚鍚庣粨鏋滀細鍑虹幇鍦ㄥ璇濅腑锛屼綘鍙互鍩轰簬缁撴灉缁х画鍥炵瓟銆俓n\n\
             {}\n",
            lines.join("\n")
        )
    }

    /// 鐢熸垚 OpenAI native function calling 鐨?tools 鏁扮粍
    pub fn to_openai_tools(&self) -> Vec<Value> {
        self.tools.values().map(|t| t.spec().to_openai_function()).collect()
    }
}

// 鈹€鈹€鈹€ 鍐呯疆宸ュ叿鍒濆鍖栧叆鍙?鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

/// 娉ㄥ唽鎵€鏈夊唴缃伐鍏凤紙鍦?AppState 鍒濆鍖栨椂璋冪敤锛?
pub fn build_registry(pool: SqlitePool, data_dir: PathBuf, sandbox: Arc<SandboxPolicy>, llm_cfg: Arc<LlmConfig>) -> ToolRegistry {
    let mut reg = ToolRegistry::new();

    // 鏂囦欢宸ュ叿
    reg.register(Arc::new(ReadFileTool { sandbox: sandbox.clone() }));
    reg.register(Arc::new(WriteFileTool { sandbox: sandbox.clone() }));
    reg.register(Arc::new(ReplaceInFileTool { sandbox: sandbox.clone() }));
    reg.register(Arc::new(ListDirTool));
    reg.register(Arc::new(SearchFilesTool));

    // 绯荤粺宸ュ叿
    reg.register(Arc::new(SafeShellTool { sandbox: sandbox.clone() }));
    reg.register(Arc::new(GitStatusTool));
    reg.register(Arc::new(OpenFileTool));
    reg.register(Arc::new(GetSystemInfoTool));
    reg.register(Arc::new(ProcessListTool));
    reg.register(Arc::new(SetReminderTool { pool: pool.clone() }));

    // 缃戠粶宸ュ叿
    reg.register(Arc::new(WebSearchTool));
    reg.register(Arc::new(WebFetchTool));

    // 璁板繂宸ュ叿锛堣涔夊悜閲忔绱?+ 鍏抽敭璇嶅洖閫€锛?
    reg.register(Arc::new(MemoryRecallTool { pool: pool.clone(), llm_cfg: llm_cfg.clone() }));
    reg.register(Arc::new(NoteTakeTool { pool: pool.clone(), llm_cfg }));

    // 鍓创鏉垮伐鍏?
    reg.register(Arc::new(ClipboardReadTool));

    // 鎴浘宸ュ叿锛圠1 鏉冮檺锛岄渶鐢ㄦ埛鍚屾剰锛?
    reg.register(Arc::new(ScreenshotTool));

    let _ = data_dir;
    reg
}

// 鈹€鈹€鈹€ 鍏蜂綋宸ュ叿瀹炵幇 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

// 鈹€鈹€ ReadFileTool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct ReadFileTool {
    pub sandbox: Arc<SandboxPolicy>,
}

#[async_trait::async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }
    fn description(&self) -> &str { "璇诲彇鏈湴鏂囦欢鍐呭锛堝墠 200 琛岋級锛岄€傚悎鏌ョ湅浠ｇ爜銆侀厤缃枃浠剁瓑" }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L0 }
    fn category(&self) -> ToolCategory { ToolCategory::File }
    fn params(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "path".into(),
            ty: "string".into(),
            description: "鏂囦欢缁濆璺緞鎴栫浉瀵硅矾寰?.into(),
            required: true,
        enum_values: None,
        default: None,
        }]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return ToolCallResult::err(id, self.name(), self.permission_level(), "缂哄皯 path 鍙傛暟"),
        };

        let path = PathBuf::from(&path_str);

        // 閫熺巼妫€鏌?
        if let Err(msg) = self.sandbox.check_rate_limit(self.name()) {
            return ToolCallResult::err(id, self.name(), self.permission_level(), &msg);
        }
        // 璺緞娌欑洅妫€鏌?
        if let Err(msg) = self.sandbox.check_file_access(self.name(), &path) {
            return ToolCallResult::err(id, self.name(), self.permission_level(), &msg);
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().take(200).collect();
                let output = lines.join("\n");
                let suffix = if content.lines().count() > 200 {
                    format!("\n鈥︼紙浠呮樉绀哄墠 200 琛岋紝鍏?{} 琛岋級", content.lines().count())
                } else {
                    String::new()
                };
                ToolCallResult::ok(id, self.name(), self.permission_level(), format!("{output}{suffix}"))
            }
            Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
        }
    }
}

// 鈹€鈹€ WriteFileTool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct WriteFileTool {
    pub sandbox: Arc<SandboxPolicy>,
}

#[async_trait::async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str { "write_file" }
    fn description(&self) -> &str { "鍐欏叆鍐呭鍒版湰鍦版枃浠讹紙浼氳鐩栧凡鏈夊唴瀹癸級锛岄€傚悎淇濆瓨浠ｇ爜銆佺瑪璁般€佹姤鍛婄瓑" }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L2 }
    fn category(&self) -> ToolCategory { ToolCategory::File }
    fn enabled_by_default(&self) -> bool { false } // 榛樿鍏抽棴锛岀敤鎴烽渶鎵嬪姩寮€鍚?
    fn params(&self) -> Vec<ToolParam> {
        vec![
            ToolParam {
                name: "path".into(),
                ty: "string".into(),
                description: "鏂囦欢缁濆璺緞".into(),
                required: true,
            enum_values: None,
            default: None,
            },
            ToolParam {
                name: "content".into(),
                ty: "string".into(),
                description: "瑕佸啓鍏ョ殑鏂囦欢鍐呭".into(),
                required: true,
            enum_values: None,
            default: None,
            },
        ]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return ToolCallResult::err(id, self.name(), self.permission_level(), "缂哄皯 path 鍙傛暟"),
        };
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) => c.to_string(),
            None => return ToolCallResult::err(id, self.name(), self.permission_level(), "缂哄皯 content 鍙傛暟"),
        };

        let path = PathBuf::from(&path_str);

        // 閫熺巼妫€鏌?
        if let Err(msg) = self.sandbox.check_rate_limit(self.name()) {
            return ToolCallResult::err(id, self.name(), self.permission_level(), &msg);
        }
        // 璺緞娌欑洅妫€鏌?
        if let Err(msg) = self.sandbox.check_file_access(self.name(), &path) {
            return ToolCallResult::err(id, self.name(), self.permission_level(), &msg);
        }

        // 纭繚鐖剁洰褰曞瓨鍦?
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return ToolCallResult::err(id, self.name(), self.permission_level(), &format!("鍒涘缓鐩綍澶辫触: {e}"));
            }
        }

        match std::fs::write(&path, &content) {
            Ok(_) => {
                let size = content.len();
                ToolCallResult::ok(id, self.name(), self.permission_level(),
                    format!("鏂囦欢宸插啓鍏? {}锛坽} 瀛楄妭锛?, path_str, size))
            }
            Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
        }
    }
}

// 鈹€鈹€ ListDirTool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct ListDirTool;

#[async_trait::async_trait]
impl Tool for ListDirTool {
    fn name(&self) -> &str { "list_dir" }
    fn description(&self) -> &str { "鍒楀嚭鐩綍鍐呭锛屾煡鐪嬫枃浠剁粨鏋? }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L0 }
    fn category(&self) -> ToolCategory { ToolCategory::File }
    fn params(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "dir".into(),
            ty: "string".into(),
            description: "鐩綍璺緞锛圽".\" 琛ㄧず褰撳墠鐩綍锛?.into(),
            required: true,
        enum_values: None,
        default: None,
        }]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let dir = match args.get("dir").and_then(|v| v.as_str()) {
            Some(d) => d.to_string(),
            None => ".".to_string(),
        };

        match std::fs::read_dir(&dir) {
            Ok(entries) => {
                let mut lines: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            format!("馃搧 {}/", name)
                        } else {
                            format!("馃搫 {}", name)
                        }
                    })
                    .collect();
                lines.sort();
                ToolCallResult::ok(id, self.name(), self.permission_level(), lines.join("\n"))
            }
            Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
        }
    }
}

// 鈹€鈹€ SearchFilesTool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct SearchFilesTool;

#[async_trait::async_trait]
impl Tool for SearchFilesTool {
    fn name(&self) -> &str { "search_files" }
    fn description(&self) -> &str { "鎸夋枃浠跺悕妯″紡鎼滅储鏂囦欢锛堟敮鎸侀€氶厤绗?* 鍜??锛夛紝閫傚悎鏌ユ壘椤圭洰涓殑鏂囦欢" }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L0 }
    fn category(&self) -> ToolCategory { ToolCategory::File }
    fn params(&self) -> Vec<ToolParam> {
        vec![
            ToolParam {
                name: "pattern".into(),
                ty: "string".into(),
                description: "鏂囦欢鍚嶆悳绱㈡ā寮忥紝濡?\"*.rs\"銆乗"*test*\"銆乗"config*\"".into(),
                required: true,
            enum_values: None,
            default: None,
            },
            ToolParam {
                name: "root".into(),
                ty: "string".into(),
                description: "鎼滅储鏍圭洰褰曪紙\".\" 琛ㄧず褰撳墠鐩綍锛?.into(),
                required: false,
            enum_values: None,
            default: None,
            },
            ToolParam {
                name: "max_results".into(),
                ty: "number".into(),
                description: "鏈€澶ц繑鍥炵粨鏋滄暟锛堥粯璁?30锛?.into(),
                required: false,
            enum_values: None,
            default: None,
            },
        ]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return ToolCallResult::err(id, self.name(), self.permission_level(), "缂哄皯 pattern 鍙傛暟"),
        };
        let root = args.get("root").and_then(|v| v.as_str()).unwrap_or(".").to_string();
        let max_results = args.get("max_results").and_then(|v| v.as_u64()).unwrap_or(30) as usize;

        let root_path = PathBuf::from(&root);
        if !root_path.is_dir() {
            return ToolCallResult::err(id, self.name(), self.permission_level(), &format!("鐩綍涓嶅瓨鍦? {root}"));
        }

        let mut results = Vec::new();
        let walker = walkdir::WalkDir::new(&root_path)
            .max_depth(10)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file());

        for entry in walker {
            if results.len() >= max_results {
                break;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if wildmatch::WildMatch::new(&pattern).matches(&name) {
                let rel = entry.path().strip_prefix(&root_path)
                    .unwrap_or(entry.path())
                    .to_string_lossy()
                    .to_string();
                results.push(rel);
            }
        }

        if results.is_empty() {
            ToolCallResult::ok(id, self.name(), self.permission_level(),
                format!("鏈壘鍒板尮閰?\"{pattern}\" 鐨勬枃浠讹紙鎼滅储鐩綍: {root}锛?))
        } else {
            ToolCallResult::ok(id, self.name(), self.permission_level(),
                format!("鎵惧埌 {} 涓尮閰?\"{pattern}\" 鐨勬枃浠讹細\n{}", results.len(), results.join("\n")))
        }
    }
}

// 鈹€鈹€ SafeShellTool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct SafeShellTool {
    pub sandbox: Arc<SandboxPolicy>,
}

#[async_trait::async_trait]
impl Tool for SafeShellTool {
    fn name(&self) -> &str { "safe_shell" }
    fn description(&self) -> &str {
        "鎵ц瀹夊叏鐨?shell 鍛戒护锛堜粎鍏佽 git/ls/pwd/cat/echo/date/cargo/pnpm 绛夊彧璇诲懡浠わ級"
    }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L3 }
    fn category(&self) -> ToolCategory { ToolCategory::System }
    fn params(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "cmd".into(),
            ty: "string".into(),
            description: "瑕佹墽琛岀殑鍛戒护".into(),
            required: true,
        enum_values: None,
        default: None,
        }]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let cmd = match args.get("cmd").and_then(|v| v.as_str()) {
            Some(c) => c.to_string(),
            None => return ToolCallResult::err(id, self.name(), self.permission_level(), "缂哄皯 cmd 鍙傛暟"),
        };

        // 閫熺巼妫€鏌?
        if let Err(msg) = self.sandbox.check_rate_limit(self.name()) {
            return ToolCallResult::err(id, self.name(), self.permission_level(), &msg);
        }
        // 鍛戒护瀹夊叏妫€鏌ワ紙sandbox 鍚璁℃棩蹇楋級
        if let Err(msg) = self.sandbox.check_command(&cmd) {
            return ToolCallResult::err(id, self.name(), self.permission_level(), &msg);
        }

        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            return ToolCallResult::err(id, self.name(), self.permission_level(), "鍛戒护涓虹┖");
        }

        match std::process::Command::new(parts[0]).args(&parts[1..]).output() {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let combined = if stderr.is_empty() {
                    stdout
                } else {
                    format!("{stdout}\n[stderr]\n{stderr}")
                };
                ToolCallResult::ok(id, self.name(), self.permission_level(), combined)
            }
            Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
        }
    }
}

// 鈹€鈹€ GitStatusTool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct GitStatusTool;

#[async_trait::async_trait]
impl Tool for GitStatusTool {
    fn name(&self) -> &str { "git_status" }
    fn description(&self) -> &str { "鏌ョ湅 Git 浠撳簱鐘舵€侊紙褰撳墠鍒嗘敮銆佸彉鏇存枃浠躲€佹渶杩戞彁浜わ級" }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L1 }
    fn category(&self) -> ToolCategory { ToolCategory::Git }
    fn params(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "dir".into(),
            ty: "string".into(),
            description: "浠撳簱璺緞锛圽".\" 琛ㄧず褰撳墠鐩綍锛?.into(),
            required: false,
        enum_values: None,
        default: None,
        }]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let dir = args.get("dir").and_then(|v| v.as_str()).unwrap_or(".").to_string();

        let mut parts = Vec::new();

        // git status
        if let Ok(out) = std::process::Command::new("git")
            .args(["-C", &dir, "status", "--short"])
            .output()
        {
            parts.push(format!("=== 鍙樻洿鏂囦欢 ===\n{}", String::from_utf8_lossy(&out.stdout)));
        }

        // git branch
        if let Ok(out) = std::process::Command::new("git")
            .args(["-C", &dir, "branch", "--show-current"])
            .output()
        {
            parts.push(format!("=== 褰撳墠鍒嗘敮 ===\n{}", String::from_utf8_lossy(&out.stdout).trim()));
        }

        // git log --oneline -5
        if let Ok(out) = std::process::Command::new("git")
            .args(["-C", &dir, "log", "--oneline", "-5"])
            .output()
        {
            parts.push(format!("=== 鏈€杩戞彁浜?===\n{}", String::from_utf8_lossy(&out.stdout)));
        }

        ToolCallResult::ok(id, self.name(), self.permission_level(), parts.join("\n"))
    }
}

// 鈹€鈹€ OpenFileTool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct OpenFileTool;

#[async_trait::async_trait]
impl Tool for OpenFileTool {
    fn name(&self) -> &str { "open_file" }
    fn description(&self) -> &str { "鐢ㄧ郴缁熼粯璁ょ▼搴忔墦寮€鏂囦欢鎴栫洰褰曪紙濡傛墦寮€鏂囦欢澶广€佸浘鐗囥€佹枃妗ｇ瓑锛? }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L1 }
    fn category(&self) -> ToolCategory { ToolCategory::System }
    fn params(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "path".into(),
            ty: "string".into(),
            description: "瑕佹墦寮€鐨勬枃浠舵垨鐩綍璺緞".into(),
            required: true,
        enum_values: None,
        default: None,
        }]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return ToolCallResult::err(id, self.name(), self.permission_level(), "缂哄皯 path 鍙傛暟"),
        };

        let path = PathBuf::from(&path_str);
        if !path.exists() {
            return ToolCallResult::err(id, self.name(), self.permission_level(),
                &format!("璺緞涓嶅瓨鍦? {path_str}"));
        }

        #[cfg(target_os = "windows")]
        let result = std::process::Command::new("explorer")
            .arg(&path_str)
            .spawn();
        #[cfg(target_os = "macos")]
        let result = std::process::Command::new("open")
            .arg(&path_str)
            .spawn();
        #[cfg(target_os = "linux")]
        let result = std::process::Command::new("xdg-open")
            .arg(&path_str)
            .spawn();

        match result {
            Ok(_) => ToolCallResult::ok(id, self.name(), self.permission_level(),
                format!("宸叉墦寮€: {path_str}")),
            Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
        }
    }
}

// 鈹€鈹€ WebSearchTool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct WebSearchTool;

#[async_trait::async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str { "web_search" }
    fn description(&self) -> &str { "閫氳繃 DuckDuckGo 鎼滅储缃戠粶淇℃伅" }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L0 }
    fn category(&self) -> ToolCategory { ToolCategory::Web }
    fn params(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "query".into(),
            ty: "string".into(),
            description: "鎼滅储鍏抽敭璇?.into(),
            required: true,
        enum_values: None,
        default: None,
        }]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q.to_string(),
            None => return ToolCallResult::err(id, self.name(), self.permission_level(), "缂哄皯 query 鍙傛暟"),
        };

        let encoded = urlencoding::encode(&query);
        let url = format!("https://duckduckgo.com/html/?q={encoded}");

        match reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (compatible; Chebo/1.0)")
            .timeout(std::time::Duration::from_secs(10))
            .build()
        {
            Ok(client) => match client.get(&url).send().await {
                Ok(resp) => match resp.text().await {
                    Ok(html) => {
                        let snippets = extract_ddg_snippets(&html, 5);
                        if snippets.is_empty() {
                            ToolCallResult::ok(
                                id, self.name(), self.permission_level(),
                                format!("鏈壘鍒?\"{query}\" 鐨勬悳绱㈢粨鏋?),
                            )
                        } else {
                            ToolCallResult::ok(
                                id, self.name(), self.permission_level(),
                                format!("鎼滅储 \"{query}\" 鐨勭粨鏋滐細\n\n{}", snippets.join("\n\n")),
                            )
                        }
                    }
                    Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
                },
                Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
            },
            Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
        }
    }
}

fn extract_ddg_snippets(html: &str, max: usize) -> Vec<String> {
    let mut results = Vec::new();
    let mut pos = 0;
    while results.len() < max {
        if let Some(start) = html[pos..].find("class=\"result__snippet\"") {
            let abs = pos + start;
            if let Some(tag_end) = html[abs..].find('>') {
                let content_start = abs + tag_end + 1;
                if let Some(close) = html[content_start..].find("</a>") {
                    let raw = &html[content_start..content_start + close];
                    let amp = ['&', 'a', 'm', 'p', ';'].iter().collect::<String>();
                    let lt = ['&', 'l', 't', ';'].iter().collect::<String>();
                    let gt = ['&', 'g', 't', ';'].iter().collect::<String>();
                    let quot = ['&', 'q', 'u', 'o', 't', ';'].iter().collect::<String>();
                    let cleaned = raw
                        .replace("<b>", "")
                        .replace("</b>", "")
                        .replace(&amp, "&")
                        .replace(&lt, "<")
                        .replace(&gt, ">")
                        .replace(&quot, "\"")
                        .replace("&#x27;", "'");
                    let cleaned = cleaned.trim().to_string();
                    if !cleaned.is_empty() {
                        results.push(cleaned);
                    }
                    pos = content_start + close + 4;
                    continue;
                }
            }
        }
        break;
    }
    results
}

// 鈹€鈹€ WebFetchTool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct WebFetchTool;

#[async_trait::async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str { "web_fetch" }
    fn description(&self) -> &str { "鑾峰彇缃戦〉鍐呭锛圚TML 绾枃鏈級锛岄€傚悎闃呰鏂囩珷銆佹枃妗ｃ€丄PI 杩斿洖绛? }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L0 }
    fn category(&self) -> ToolCategory { ToolCategory::Web }
    fn params(&self) -> Vec<ToolParam> {
        vec![
            ToolParam {
                name: "url".into(),
                ty: "string".into(),
                description: "瑕佽幏鍙栫殑缃戦〉 URL".into(),
                required: true,
            enum_values: None,
            default: None,
            },
            ToolParam {
                name: "max_chars".into(),
                ty: "number".into(),
                description: "鏈€澶ц繑鍥炲瓧绗︽暟锛堥粯璁?3000锛?.into(),
                required: false,
            enum_values: None,
            default: None,
            },
        ]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let url = match args.get("url").and_then(|v| v.as_str()) {
            Some(u) => u.to_string(),
            None => return ToolCallResult::err(id, self.name(), self.permission_level(), "缂哄皯 url 鍙傛暟"),
        };
        let max_chars = args.get("max_chars").and_then(|v| v.as_u64()).unwrap_or(3000) as usize;

        match reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (compatible; Chebo/1.0)")
            .timeout(std::time::Duration::from_secs(15))
            .build()
        {
            Ok(client) => match client.get(&url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    match resp.text().await {
                        Ok(html) => {
                            // 绠€鍗曟彁鍙栫函鏂囨湰锛堝幓闄?HTML 鏍囩锛?
                            let text = html2text::from_read(html.as_bytes(), max_chars);
                            let text = text.trim().to_string();
                            let text = if text.len() > max_chars {
                                format!("{}鈥︼紙宸叉埅鏂紝鍘熼暱 {} 瀛楃锛?, &text[..max_chars], text.len())
                            } else {
                                text
                            };
                            ToolCallResult::ok(id, self.name(), self.permission_level(),
                                format!("HTTP {status}\n\n{text}"))
                        }
                        Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
                    }
                }
                Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
            },
            Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
        }
    }
}

// 鈹€鈹€ MemoryRecallTool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct MemoryRecallTool {
    pub pool:     SqlitePool,
    pub llm_cfg:  Arc<LlmConfig>,
}

#[async_trait::async_trait]
impl Tool for MemoryRecallTool {
    fn name(&self) -> &str { "memory_recall" }
    fn description(&self) -> &str {
        "浠?Chebo 鐨勯暱鏈熻蹇嗕腑妫€绱俊鎭紙瀵硅瘽鎽樿銆佺敤鎴蜂範鎯€佸叧绯昏蹇嗙瓑锛?
    }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L0 }
    fn category(&self) -> ToolCategory { ToolCategory::Memory }
    fn params(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "query".into(),
            ty: "string".into(),
            description: "瑕佹绱㈢殑鍏抽敭璇嶆垨闂".into(),
            required: true,
        enum_values: None,
        default: None,
        }]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q.to_string(),
            None => return ToolCallResult::err(id, self.name(), self.permission_level(), "缂哄皯 query 鍙傛暟"),
        };

        let mut results: Vec<String> = Vec::new();
        let mut used_semantic = false;

        // P0: 浼樺厛璇箟鍚戦噺妫€绱?
        match memory_vector::recall_semantic(&self.pool, &self.llm_cfg, &query, 8).await {
            Ok(hits) if !hits.is_empty() => {
                used_semantic = true;
                results = hits
                    .iter()
                    .map(|h| format!("{} (鐩稿叧搴?{:.0}%)", h.content, h.score * 100.0))
                    .collect();
            }
            Ok(_) => {}
            Err(e) => {
                log::debug!("memory_recall semantic fallback: {e}");
            }
        }

        // 鍚戦噺涓嶅彲鐢ㄦ垨鏃犲懡涓椂鍥為€€鍏抽敭璇嶆绱?
        if results.is_empty() {
            results = memory_vector::recall_keyword(&self.pool, &query).await;
        }

        if results.is_empty() {
            ToolCallResult::ok(
                id, self.name(), self.permission_level(),
                format!("娌℃湁鎵惧埌鍏充簬 \"{}\" 鐨勮蹇?, query),
            )
        } else {
            let header = if used_semantic {
                format!("鍏充簬 \"{}\" 鐨勮涔夌浉鍏宠蹇嗭細", query)
            } else {
                format!("鍏充簬 \"{}\" 鐨勮蹇嗭細", query)
            };
            ToolCallResult::ok(
                id, self.name(), self.permission_level(),
                format!("{header}\n\n{}", results.join("\n")),
            )
        }
    }
}

// 鈹€鈹€ ClipboardReadTool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct ClipboardReadTool;

#[async_trait::async_trait]
impl Tool for ClipboardReadTool {
    fn name(&self) -> &str { "clipboard_read" }
    fn description(&self) -> &str { "璇诲彇鐢ㄦ埛褰撳墠鍓创鏉垮唴瀹? }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L1 }
    fn category(&self) -> ToolCategory { ToolCategory::Clipboard }
    fn params(&self) -> Vec<ToolParam> { vec![] }

    async fn execute(&self, id: &str, _args: Value) -> ToolCallResult {
        match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
            Ok(text) if !text.is_empty() => {
                ToolCallResult::ok(id, self.name(), self.permission_level(), text)
            }
            Ok(_) => ToolCallResult::ok(
                id, self.name(), self.permission_level(), "锛堝壀璐存澘涓虹┖锛?.to_string(),
            ),
            Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
        }
    }
}

// 鈹€鈹€鈹€ 宸ュ叿鍑芥暟 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

fn is_forbidden_path(path: &PathBuf) -> bool {
    let p = path.to_string_lossy().to_lowercase();
    let forbidden = ["/etc/", "/sys/", "/proc/", "c:\\windows\\system32\\"];
    forbidden.iter().any(|f| p.starts_with(f))
}

fn is_dangerous_cmd(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    let dangerous = ["rm ", "del ", "rmdir", "rd ", "sudo", "format", "mkfs",
                     ":(){", "dd if", "> /dev", "shutdown", "reboot", "halt"];
    dangerous.iter().any(|d| lower.contains(d))
}

// 鈹€鈹€ ReplaceInFileTool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct ReplaceInFileTool {
    pub sandbox: Arc<SandboxPolicy>,
}

#[async_trait::async_trait]
impl Tool for ReplaceInFileTool {
    fn name(&self) -> &str { "replace_in_file" }
    fn description(&self) -> &str { "绮剧‘鏇挎崲鏂囦欢涓殑鍐呭锛堟煡鎵炬浛鎹級锛屾瘮 write_file 鏇村畨鍏紝閫傚悎淇敼浠ｇ爜鐗囨" }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L2 }
    fn category(&self) -> ToolCategory { ToolCategory::File }
    fn enabled_by_default(&self) -> bool { false }
    fn params(&self) -> Vec<ToolParam> {
        vec![
            ToolParam { name: "path".into(), ty: "string".into(), description: "鏂囦欢缁濆璺緞".into(), required: true },
            ToolParam { name: "old".into(), ty: "string".into(), description: "瑕佹浛鎹㈢殑鍘熸枃锛堢簿纭尮閰嶏級".into(), required: true },
            ToolParam { name: "new".into(), ty: "string".into(), description: "鏇挎崲鍚庣殑鏂板唴瀹?.into(), required: true },
        ]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let path = match args.get("path").and_then(|v| v.as_str()) { Some(p) => p.to_string(), None => return ToolCallResult::err(id, self.name(), self.permission_level(), "缂哄皯 path 鍙傛暟") };
        let old = match args.get("old").and_then(|v| v.as_str()) { Some(s) => s.to_string(), None => return ToolCallResult::err(id, self.name(), self.permission_level(), "缂哄皯 old 鍙傛暟") };
        let new = match args.get("new").and_then(|v| v.as_str()) { Some(s) => s.to_string(), None => return ToolCallResult::err(id, self.name(), self.permission_level(), "缂哄皯 new 鍙傛暟") };

        let path_buf = PathBuf::from(&path);
        if let Err(msg) = self.sandbox.check_rate_limit(self.name()) { return ToolCallResult::err(id, self.name(), self.permission_level(), &msg); }
        if let Err(msg) = self.sandbox.check_file_access(self.name(), &path_buf) { return ToolCallResult::err(id, self.name(), self.permission_level(), &msg); }

        match std::fs::read_to_string(&path_buf) {
            Ok(content) => {
                if !content.contains(&old) {
                    return ToolCallResult::err(id, self.name(), self.permission_level(), "鏈壘鍒板尮閰嶇殑鍘熸枃锛屾浛鎹㈠け璐?);
                }
                let new_content = content.replace(&old, &new);
                let count = content.matches(&old).count();
                match std::fs::write(&path_buf, &new_content) {
                    Ok(_) => ToolCallResult::ok(id, self.name(), self.permission_level(),
                        format!("鏂囦欢 {} 鏇挎崲瀹屾垚锛屽叡鏇挎崲 {} 澶?, path, count)),
                    Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
                }
            }
            Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
        }
    }
}

// 鈹€鈹€ GetSystemInfoTool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct GetSystemInfoTool;

#[async_trait::async_trait]
impl Tool for GetSystemInfoTool {
    fn name(&self) -> &str { "get_system_info" }
    fn description(&self) -> &str { "鏌ヨ绯荤粺淇℃伅锛堟搷浣滅郴缁熴€丆PU銆佸唴瀛樸€佺鐩樸€佹椂闂寸瓑锛? }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L0 }
    fn category(&self) -> ToolCategory { ToolCategory::System }
    fn params(&self) -> Vec<ToolParam> { vec![] }

    async fn execute(&self, id: &str, _args: Value) -> ToolCallResult {
        let mut info = Vec::new();
        info.push(format!("鎿嶄綔绯荤粺: {} {}", std::env::consts::OS, std::env::consts::ARCH));
        info.push(format!("褰撳墠鏃堕棿: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")));
        info.push(format!("涓绘満鍚? {}", hostname().unwrap_or_else(|_| "unknown".to_string())));

        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            if let Ok(out) = Command::new("wmic").args(["os", "get", "TotalVisibleMemorySize,FreePhysicalMemory", "/format:csv"]).output() {
                let s = String::from_utf8_lossy(&out.stdout);
                let parts: Vec<&str> = s.trim().split('\n').collect();
                if parts.len() >= 2 {
                    let vals: Vec<&str> = parts[1].split(',').collect();
                    if vals.len() >= 3 {
                        let total_kb: f64 = vals[1].trim().parse().unwrap_or(0.0);
                        let free_kb: f64 = vals[2].trim().parse().unwrap_or(0.0);
                        let used_pct = (total_kb - free_kb) / total_kb * 100.0;
                        info.push(format!("鍐呭瓨: 宸茬敤 {used_pct:.0}% / {:.1} GB", total_kb / 1_048_576.0));
                    }
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let info_lines = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
            for line in info_lines.lines().take(4) { info.push(line.to_string()); }
        }

        ToolCallResult::ok(id, self.name(), self.permission_level(), info.join("\n"))
    }
}

fn hostname() -> Result<String, std::io::Error> {
    Ok(std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "localhost".to_string()))
}

// 鈹€鈹€ ProcessListTool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct ProcessListTool;

#[async_trait::async_trait]
impl Tool for ProcessListTool {
    fn name(&self) -> &str { "process_list" }
    fn description(&self) -> &str { "鏌ョ湅姝ｅ湪杩愯鐨勮繘绋嬪垪琛紙绮剧畝妯″紡锛岄伩鍏嶄俊鎭繃澶氾級" }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L0 }
    fn category(&self) -> ToolCategory { ToolCategory::System }
    fn params(&self) -> Vec<ToolParam> {
        vec![ToolParam { name: "max".into(), ty: "number".into(), description: "鏈€澶ф樉绀鸿繘绋嬫暟锛堥粯璁?20锛?.into(), required: false }]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let max = args.get("max").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
        let output = std::process::Command::new("tasklist")
            .args(["/FO", "CSV", "/NH"])
            .output();
        match output {
            Ok(out) => {
                let s = String::from_utf8_lossy(&out.stdout);
                let lines: Vec<&str> = s.lines().take(max).collect();
                ToolCallResult::ok(id, self.name(), self.permission_level(), format!("杩涚▼鍒楄〃锛堝墠 {} 鏉★級锛歕n{}", lines.len(), lines.join("\n")))
            }
            Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
        }
    }
}

// 鈹€鈹€ SetReminderTool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct SetReminderTool {
    pub pool: SqlitePool,
}

#[async_trait::async_trait]
impl Tool for SetReminderTool {
    fn name(&self) -> &str { "set_reminder" }
    fn description(&self) -> &str { "璁剧疆涓€涓畾鏃舵彁閱掋€傛彁閱掑埌鏈熷悗 Chebo 浼氫富鍔ㄦ彁閱掓偍锛堜粎褰撳墠浼氳瘽鏈夋晥锛? }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L1 }
    fn category(&self) -> ToolCategory { ToolCategory::System }
    fn enabled_by_default(&self) -> bool { false }
    fn params(&self) -> Vec<ToolParam> {
        vec![
            ToolParam { name: "minutes".into(), ty: "number".into(), description: "澶氬皯鍒嗛挓鍚庢彁閱?.into(), required: true },
            ToolParam { name: "message".into(), ty: "string".into(), description: "鎻愰啋鍐呭".into(), required: true },
        ]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let minutes = args.get("minutes").and_then(|v| v.as_u64()).unwrap_or(0);
        let message = args.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if minutes == 0 { return ToolCallResult::err(id, self.name(), self.permission_level(), "minutes 蹇呴』澶т簬 0"); }
        if message.is_empty() { return ToolCallResult::err(id, self.name(), self.permission_level(), "缂哄皯 message 鍙傛暟"); }

        let pool = self.pool.clone();
        let msg = message.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(minutes * 60)).await;
            let _ = sqlx::query(
                "INSERT INTO pet_events (type, payload) VALUES ('reminder', ?)"
            ).bind(&msg).execute(&pool).await;
            log::info!("鎻愰啋鍒版湡: {msg}");
        });

        ToolCallResult::ok(id, self.name(), self.permission_level(),
            format!("宸茶缃彁閱掞紝{minutes} 鍒嗛挓鍚庝細閫氱煡鎮細銆寋message}銆?))
    }
}

// 鈹€鈹€ NoteTakeTool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct NoteTakeTool {
    pub pool:    SqlitePool,
    pub llm_cfg: Arc<LlmConfig>,
}

#[async_trait::async_trait]
impl Tool for NoteTakeTool {
    fn name(&self) -> &str { "note_take" }
    fn description(&self) -> &str { "鍒涘缓/璇诲彇/鏇存柊/鍒犻櫎绗旇銆傜瑪璁板瓨鍌ㄥ湪闀挎湡璁板繂涓紝鍙互闅忔椂鏌ヨ銆? }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L1 }
    fn category(&self) -> ToolCategory { ToolCategory::Memory }
    fn params(&self) -> Vec<ToolParam> {
        vec![
            ToolParam { name: "action".into(), ty: "string".into(), description: "鎿嶄綔锛歝reate/read/update/delete/list".into(), required: true },
            ToolParam { name: "key".into(), ty: "string".into(), description: "绗旇鐨勯敭鍚嶏紙濡?\"todo-list\"銆乗"meeting-notes\"锛?.into(), required: true },
            ToolParam { name: "content".into(), ty: "string".into(), description: "绗旇鍐呭锛坈reate/update 鏃堕渶瑕侊級".into(), required: false },
        ]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let key = args.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();

        if key.is_empty() { return ToolCallResult::err(id, self.name(), self.permission_level(), "缂哄皯 key 鍙傛暟"); }

        match action.as_str() {
            "create" | "update" => {
                if content.is_empty() { return ToolCallResult::err(id, self.name(), self.permission_level(), "create/update 闇€瑕?content 鍙傛暟"); }
                match crate::db::set_user_profile(&self.pool, &format!("note:{}", key), &content).await {
                    Ok(_) => ToolCallResult::ok(id, self.name(), self.permission_level(), format!("绗旇銆寋key}銆嶅凡淇濆瓨")),
                    Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
                }
            }
            "read" => {
                match crate::db::get_config(&self.pool, &format!("note:{}", key)).await {
                    Ok(Some(v)) => ToolCallResult::ok(id, self.name(), self.permission_level(), format!("绗旇銆寋key}銆嶏細\n{v}")),
                    Ok(None) => ToolCallResult::ok(id, self.name(), self.permission_level(), format!("鏈壘鍒扮瑪璁般€寋key}銆?)),
                    Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
                }
            }
            "delete" => {
                match crate::db::delete_user_profile_entry(&self.pool, &format!("note:{}", key)).await {
                    Ok(_) => ToolCallResult::ok(id, self.name(), self.permission_level(), format!("绗旇銆寋key}銆嶅凡鍒犻櫎")),
                    Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
                }
            }
            "list" => {
                use sqlx::Row;
                let rows = sqlx::query("SELECT key FROM user_profile WHERE key LIKE 'note:%' ORDER BY updated_at DESC")
                    .fetch_all(&self.pool).await;
                match rows {
                    Ok(rows) => {
                        let keys: Vec<String> = rows.iter().map(|r| {
                            let k: String = r.get("key");
                            k.trim_start_matches("note:").to_string()
                        }).collect();
                        if keys.is_empty() {
                            ToolCallResult::ok(id, self.name(), self.permission_level(), "鏆傛棤绗旇".to_string())
                        } else {
                            ToolCallResult::ok(id, self.name(), self.permission_level(),
                                format!("鍏辨湁 {} 鏉＄瑪璁帮細\n{}", keys.len(), keys.join("\n")))
                        }
                    }
                    Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
                }
            }
            _ => ToolCallResult::err(id, self.name(), self.permission_level(), &format!("鏈煡鎿嶄綔: {action}锛屽彲鐢細create/read/update/delete/list")),
        }
    }
}

// 鈹€鈹€ ScreenshotTool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

pub struct ScreenshotTool;

#[async_trait::async_trait]
impl Tool for ScreenshotTool {
    fn name(&self) -> &str { "take_screenshot" }
    fn description(&self) -> &str {
        "鎴彇褰撳墠灞忓箷鎴浘锛屼互 base64 PNG 鏍煎紡杩斿洖銆傞€傚悎闇€瑕佹煡鐪嬬敤鎴峰綋鍓嶅睆骞曞唴瀹规椂浣跨敤銆俓
         濡傛灉 LLM 鏀寔瑙嗚鑳藉姏锛屽彲浠ュ垎鏋愬浘鐗囧唴瀹癸紱鍚﹀垯杩斿洖鎴浘鐨勪繚瀛樿矾寰勩€?
    }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L1 }
    fn category(&self) -> ToolCategory { ToolCategory::Media }
    fn params(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "monitor".into(),
            ty: "number".into(),
            description: "鏄剧ず鍣ㄧ储寮曪紙浠?0 寮€濮嬶級锛岄粯璁?0锛堜富灞忓箷锛?.into(),
            required: false,
        enum_values: None,
        default: None,
        }]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        use base64::Engine as _;

        let monitor_idx = args.get("monitor")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        // 鍦ㄩ樆濉炵嚎绋嬩腑鎵ц鎴浘锛堥伩鍏嶉樆濉?tokio 杩愯鏃讹級
        let result = tokio::task::spawn_blocking(move || {
            let screens = screenshots::Screen::all()
                .map_err(|e| format!("鑾峰彇鏄剧ず鍣ㄥ垪琛ㄥけ璐? {e}"))?;

            let screen = screens.get(monitor_idx)
                .or_else(|| screens.first())
                .ok_or_else(|| "鏈壘鍒颁换浣曟樉绀哄櫒".to_string())?;

            let image = screen.capture()
                .map_err(|e| format!("鎴浘澶辫触: {e}"))?;

            // 淇濆瓨鍒颁复鏃剁洰褰?
            let tmp_path = std::env::temp_dir().join(
                format!("chebo_screenshot_{}.png",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs())
            );

            // 杞负 DynamicImage 鍚?save锛堣嚜鍔ㄦ帹鏂?PNG 鏍煎紡锛?
            let dyn_img = screenshots::image::DynamicImage::ImageRgba8(image);
            dyn_img.save(&tmp_path)
                .map_err(|e| format!("淇濆瓨鎴浘澶辫触: {e}"))?;

            let png_bytes = std::fs::read(&tmp_path)
                .map_err(|e| format!("璇诲彇鎴浘鏂囦欢澶辫触: {e}"))?;

            let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
            let size_kb = png_bytes.len() / 1024;

            Ok::<String, String>(format!(
                "鎴浘宸插畬鎴愶紙{}脳{} px锛寋} KB锛塡n淇濆瓨璺緞锛歿}\ndata:image/png;base64,{}",
                screen.display_info.width,
                screen.display_info.height,
                size_kb,
                tmp_path.display(),
                &b64[..b64.len().min(200)],  // 鍙睍绀哄墠200瀛楃锛岄伩鍏嶈秴鍑?MAX_OUTPUT
            ))
        }).await;

        match result {
            Ok(Ok(output)) => ToolCallResult::ok(id, self.name(), self.permission_level(), output),
            Ok(Err(msg))   => ToolCallResult::err(id, self.name(), self.permission_level(), &msg),
            Err(e)         => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
        }
    }
}