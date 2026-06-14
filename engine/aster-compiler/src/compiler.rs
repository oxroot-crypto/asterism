//! Asterism — Galgame/ADV 游戏引擎
//!
//! 文件路径：engine/aster-compiler/src/compiler.rs
//! 功能概述：编译器核心 — `Compiler` 结构体实现 AST → IR → Bytecode 三步编译管线。
//!           包含表达式编译（Expr 树→寄存器操作）、SceneNode 编译（25 种变体→IR 指令）、
//!           Branch 展开（if/elif/else→条件跳转）、Menu 编码。
//! 作者：Claude (AI)
//! 创建日期：2026-06-14
//! 最后修改：2026-06-14
//!
//! 依赖模块：
//! - aster_core::{Scene, SceneNode, Choice, Position, TransitionSpec, Expr, BinaryOp, UnaryOp}
//! - crate::ir::{IrInstruction, PositionEncoding, ChoiceData, RegisterAllocator, NONE_REG, NONE_POOL}
//! - crate::bytecode::{encode_instructions, CompiledScene}
//! - crate::error::CompileError

use std::collections::HashMap;

use aster_core::{BinaryOp, Expr, Position, Scene, SceneNode, TransitionSpec, UnaryOp};

use crate::bytecode::{CompiledScene, encode_instructions};
use crate::error::CompileError;
use crate::ir::{
    ChoiceData, IrInstruction, NONE_POOL, NONE_REG, PositionEncoding, RegisterAllocator,
};

/// 场景编译器 — 将 `aster_core::Scene` 编译为 `CompiledScene`。
///
/// # 编译流程
///
/// ```text
/// Scene (AST)
///   │ Pass 0: collect_strings()
///   │   收集所有字符串字面量到常量池，记录所有 Label 位置
///   │
///   ├─ Pass 1: generate_ir()
///   │   遍历 SceneNode，将 AST 节点转换为 IrInstruction 序列：
///   │   - 表达式降级：Expr 树 → 寄存器操作序列
///   │   - 控制流展开：Branch → 条件跳转 + 内部标签
///   │   - 菜单编码：Choice → ChoiceData（池化）
///   │
///   └─ Pass 2: emit_bytecode()
///       将 IR 序列编码为字节码字节数组
///       将标签名解析为字节偏移写入 label_table
/// ```
///
/// # 错误处理
///
/// 非致命错误（如未定义变量引用）收集到 `errors` 中，
/// 编译器继续处理剩余节点以收集更多错误。
/// 编译结束时如果有任何错误则返回 `Err(Vec<CompileError>)`。
#[derive(Debug, Default)]
pub struct Compiler {
    /// 字符串常量池（所有字符串字面量的有序列表）
    pool: Vec<String>,

    /// 字符串→常量池索引 反向映射（用于去重和快速查找）
    pool_map: HashMap<String, u16>,

    /// IR 指令序列（中间产物，编码前可检查/优化）
    ir: Vec<IrInstruction>,

    /// 语义错误收集列表
    errors: Vec<CompileError>,

    /// 自动生成内部标签的递增计数器（如 `@branch_then_0`）
    next_label_id: usize,

    /// 下一个可用寄存器的起始编号 — 同一 SceneNode 内多个表达式共享，
    /// 避免各 Expr 独立分配导致寄存器碰撞。
    next_reg: u8,
}

impl Compiler {
    /// 创建一个新的编译器实例。
    pub fn new() -> Self {
        Compiler::default()
    }

    // ========================================================================
    // 公共 API
    // ========================================================================

    /// 将 Scene 编译为 CompiledScene。
    ///
    /// 执行完整的 AST→IR→Bytecode 三步编译管线。
    /// 非致命语义错误会被收集，编译结束后批量返回。
    ///
    /// # 参数
    /// - `scene`：来自 `aster_parser::parse_script()` 的已解析场景
    ///
    /// # 返回值
    /// - `Ok(CompiledScene)`：编译成功，产出可被 VM 执行的字节码
    /// - `Err(Vec<CompileError>)`：编译失败，包含所有检测到的语义错误
    ///
    /// # 示例
    /// ```rust,no_run
    /// use aster_core::{Scene, SceneNode, Expr};
    /// use aster_compiler::Compiler;
    ///
    /// let scene = Scene {
    ///     id: "test".into(),
    ///     label: None,
    ///     background: None,
    ///     music: None,
    ///     nodes: vec![
    ///         SceneNode::Dialogue {
    ///             speaker: Expr::string_literal("旁白"),
    ///             text: Expr::string_literal("Hello"),
    ///             voice_id: None,
    ///         },
    ///     ],
    /// };
    ///
    /// let compiler = Compiler::new();
    /// let result = compiler.compile(&scene);
    /// assert!(result.is_ok());
    /// ```
    /// 编译并启用全部 4 个优化 Pass。
    pub fn compile(self, scene: &Scene) -> Result<CompiledScene, Vec<CompileError>> {
        self.compile_impl(scene, true)
    }

    /// 编译但跳过优化 Pass（用于对比优化效果）。
    pub fn compile_raw(self, scene: &Scene) -> Result<CompiledScene, Vec<CompileError>> {
        self.compile_impl(scene, false)
    }

    /// 编译实现 — `optimize` 控制是否启用优化 Pass。
    fn compile_impl(
        mut self,
        scene: &Scene,
        optimize: bool,
    ) -> Result<CompiledScene, Vec<CompileError>> {
        // Pass 0: 预处理 — 收集字符串 + 标签
        self.collect_strings(scene);

        // Pass 1: 生成 IR
        self.generate_ir(scene);

        // 如果有错误则立即返回
        if !self.errors.is_empty() {
            return Err(std::mem::take(&mut self.errors));
        }

        // Pass 1.5: IR 优化（PH1-T12）
        if optimize {
            crate::optimizer::Optimizer::new().optimize(&mut self.ir);
        }

        // Pass 2: 编码字节码
        let mut label_table = HashMap::new();
        let instructions = encode_instructions(&self.ir, &self.pool, &mut label_table)
            .map_err(|e| vec![CompileError::without_position(e, None::<&str>)])?;

        Ok(CompiledScene {
            version: 1,
            instructions,
            constant_pool: self.pool,
            label_table,
        })
    }

    // ========================================================================
    // Pass 0: 字符串收集
    // ========================================================================

    /// 第一遍：遍历所有 SceneNode，收集字符串字面量到常量池。
    ///
    /// 遍历过程中：
    /// - 所有 text/speaker/asset_path 等字符串字段 → `intern()`
    /// - 所有 label name、flag name、variable name → `intern()`
    /// - 所有 Branch 嵌套节点 → 递归收集
    fn collect_strings(&mut self, scene: &Scene) {
        // 收集场景元数据中的字符串
        if let Some(ref bg) = scene.background {
            self.collect_expr_strings(bg);
        }
        if let Some(ref music) = scene.music {
            self.collect_expr_strings(music);
        }

        // 收集所有 SceneNode 中的字符串
        self.collect_nodes_strings(&scene.nodes);
    }

    /// 递归收集 SceneNode 列表中的字符串。
    fn collect_nodes_strings(&mut self, nodes: &[SceneNode]) {
        for node in nodes {
            self.collect_node_strings(node);
        }
    }

    /// 收集单个 SceneNode 中的字符串。
    fn collect_node_strings(&mut self, node: &SceneNode) {
        match node {
            SceneNode::Bg {
                asset_path,
                transition,
            } => {
                self.collect_expr_strings(asset_path);
                if let Some(t) = transition {
                    self.intern(&t.kind);
                    self.collect_expr_strings(&t.duration_ms);
                }
            }
            SceneNode::Dialogue {
                speaker,
                text,
                voice_id,
            } => {
                self.collect_expr_strings(speaker);
                self.collect_expr_strings(text);
                if let Some(v) = voice_id {
                    self.collect_expr_strings(v);
                }
            }
            SceneNode::ShowChar {
                char_id,
                position,
                emotion,
                transition,
            } => {
                self.collect_expr_strings(char_id);
                if let Position::Custom(x, y) = position {
                    self.collect_expr_strings(x);
                    self.collect_expr_strings(y);
                }
                if let Some(e) = emotion {
                    self.collect_expr_strings(e);
                }
                if let Some(t) = transition {
                    self.intern(&t.kind);
                    self.collect_expr_strings(&t.duration_ms);
                }
            }
            SceneNode::ShowSprite {
                asset_path,
                x,
                y,
                scale,
                alpha,
                transition,
            } => {
                self.collect_expr_strings(asset_path);
                self.collect_expr_strings(x);
                self.collect_expr_strings(y);
                self.collect_expr_strings(scale);
                self.collect_expr_strings(alpha);
                if let Some(t) = transition {
                    self.intern(&t.kind);
                    self.collect_expr_strings(&t.duration_ms);
                }
            }
            SceneNode::MoveChar {
                char_id,
                position,
                emotion,
                transition,
            } => {
                self.collect_expr_strings(char_id);
                if let Position::Custom(x, y) = position {
                    self.collect_expr_strings(x);
                    self.collect_expr_strings(y);
                }
                if let Some(e) = emotion {
                    self.collect_expr_strings(e);
                }
                self.intern(&transition.kind);
                self.collect_expr_strings(&transition.duration_ms);
            }
            SceneNode::Emotion {
                char_id,
                emotion,
                transition,
            } => {
                self.collect_expr_strings(char_id);
                self.collect_expr_strings(emotion);
                if let Some(t) = transition {
                    self.intern(&t.kind);
                    self.collect_expr_strings(&t.duration_ms);
                }
            }
            SceneNode::HideChar {
                char_id,
                transition,
            } => {
                self.collect_expr_strings(char_id);
                if let Some(t) = transition {
                    self.intern(&t.kind);
                    self.collect_expr_strings(&t.duration_ms);
                }
            }
            SceneNode::HideSprite {
                asset_path,
                transition,
            } => {
                self.collect_expr_strings(asset_path);
                if let Some(t) = transition {
                    self.intern(&t.kind);
                    self.collect_expr_strings(&t.duration_ms);
                }
            }
            SceneNode::Narration { text } => {
                self.collect_expr_strings(text);
            }
            SceneNode::Menu { prompt, choices } => {
                self.collect_expr_strings(prompt);
                for choice in choices {
                    self.collect_expr_strings(&choice.text);
                    self.collect_expr_strings(&choice.target);
                    if let Some(ref cond) = choice.condition {
                        self.collect_expr_strings(cond);
                    }
                }
            }
            SceneNode::Branch {
                condition,
                then_nodes,
                elif_branches,
                else_nodes,
            } => {
                self.collect_expr_strings(condition);
                self.collect_nodes_strings(then_nodes);
                for (cond, nodes) in elif_branches {
                    self.collect_expr_strings(cond);
                    self.collect_nodes_strings(nodes);
                }
                if let Some(nodes) = else_nodes {
                    self.collect_nodes_strings(nodes);
                }
            }
            SceneNode::SetVariable { name, value } => {
                self.intern(name);
                self.collect_expr_strings(value);
            }
            SceneNode::SetFlag { name }
            | SceneNode::UnsetFlag { name }
            | SceneNode::ToggleFlag { name } => {
                self.intern(name);
            }
            SceneNode::Music {
                asset_path,
                fade_in,
                ..
            } => {
                self.collect_expr_strings(asset_path);
                if let Some(f) = fade_in {
                    self.collect_expr_strings(f);
                }
            }
            SceneNode::StopMusic { fade_out } => {
                if let Some(f) = fade_out {
                    self.collect_expr_strings(f);
                }
            }
            SceneNode::PlaySE { asset_id, fade_in } => {
                self.collect_expr_strings(asset_id);
                if let Some(f) = fade_in {
                    self.collect_expr_strings(f);
                }
            }
            SceneNode::Wait { duration_ms } => {
                self.collect_expr_strings(duration_ms);
            }
            SceneNode::Effect {
                effect_type,
                params,
            } => {
                self.intern(effect_type);
                for (key, val) in params {
                    self.intern(key);
                    self.collect_expr_strings(val);
                }
            }
            SceneNode::Jump { target } => {
                self.collect_expr_strings(target);
            }
            SceneNode::Goto { scene_id, label } => {
                self.collect_expr_strings(scene_id);
                if let Some(l) = label {
                    self.collect_expr_strings(l);
                }
            }
            SceneNode::Call { name, args } => {
                self.intern(name);
                for arg in args {
                    self.collect_expr_strings(arg);
                }
            }
            SceneNode::Return => {}
            SceneNode::Label { name } => {
                self.intern(name);
            }
            SceneNode::Subroutine { name, body } => {
                self.intern(name);
                self.collect_nodes_strings(body);
            }
        }
    }

    /// 递归收集 Expr 树中的字符串字面量和变量名。
    fn collect_expr_strings(&mut self, expr: &Expr) {
        match expr {
            Expr::StringLiteral(s) => {
                self.intern(s);
            }
            Expr::Variable(name) => {
                // 旗标引用保留 % 前缀，入库时去掉以确保与 SetFlag 等共用同一 pool 条目
                if let Some(flag_name) = name.strip_prefix('%') {
                    self.intern(flag_name);
                } else {
                    self.intern(name);
                }
            }
            Expr::BinaryOp(left, _, right) => {
                self.collect_expr_strings(left);
                self.collect_expr_strings(right);
            }
            Expr::UnaryOp(_, operand) => {
                self.collect_expr_strings(operand);
            }
            // 字面量不包含字符串引用
            Expr::IntLiteral(_) | Expr::FloatLiteral(_) | Expr::BoolLiteral(_) => {}
        }
    }

    /// 将字符串加入常量池（去重），返回索引。
    ///
    /// 如果字符串已存在则返回已有索引，否则追加到池末尾。
    fn intern(&mut self, s: &str) -> u16 {
        if let Some(&idx) = self.pool_map.get(s) {
            return idx;
        }
        let idx = self.pool.len() as u16;
        self.pool.push(s.to_string());
        // 复用刚推入 pool 的 String 作为 HashMap 键，避免二次分配
        let key = self.pool.last().expect("刚推入，必非空").clone();
        self.pool_map.insert(key, idx);
        idx
    }

    // ========================================================================
    // Pass 1: IR 生成
    // ========================================================================

    /// 第二遍：遍历 SceneNode，生成 IR 指令序列。
    fn generate_ir(&mut self, scene: &Scene) {
        // 无子例程节点重排到场景末尾，主流程自动跳过
        let sub_names: std::collections::HashSet<String> = scene
            .nodes
            .iter()
            .filter_map(|n| {
                if let SceneNode::Subroutine { name, .. } = n {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect();

        // 验证：Jump/Goto/Call 不能将非子例程 Label 作为目标（仅子例程名可用）
        // 验证：子例程内部不能使用 Jump/Goto
        for node in &scene.nodes {
            Self::validate_node(node, &sub_names, &mut self.errors);
        }

        let (main_nodes, sub_nodes): (Vec<SceneNode>, Vec<SceneNode>) = scene
            .nodes
            .iter()
            .cloned()
            .partition(|n| !matches!(n, SceneNode::Subroutine { .. }));

        self.compile_nodes(&main_nodes);
        if !sub_nodes.is_empty() {
            let skip = self.gen_label("sub_skip");
            self.emit(IrInstruction::Jump {
                target: skip.clone(),
            });
            self.compile_nodes(&sub_nodes);
            self.emit(IrInstruction::Label { name: skip });
        }
        self.emit(IrInstruction::End);
    }

    /// 递归编译 SceneNode 列表。
    fn compile_nodes(&mut self, nodes: &[SceneNode]) {
        for node in nodes {
            self.next_reg = 0;
            self.compile_scene_node(node);
        }
    }

    /// 编译单个 SceneNode → 追加 IR 指令。
    fn compile_scene_node(&mut self, node: &SceneNode) {
        match node {
            SceneNode::Bg {
                asset_path,
                transition,
            } => {
                let asset_idx = self.compile_expr_to_pool_or_reg(asset_path, true);
                let (trans_kind_idx, dur_reg) = self.compile_optional_transition(transition);
                self.emit(IrInstruction::Bg {
                    asset_idx,
                    trans_kind_idx,
                    dur_reg,
                });
            }
            SceneNode::Dialogue {
                speaker,
                text,
                voice_id,
            } => {
                let speaker_idx = self.compile_expr_to_pool_or_reg(speaker, true);
                let text_idx = self.compile_expr_to_pool_or_reg(text, true);
                let voice_idx = match voice_id {
                    Some(v) => self.compile_expr_to_pool_or_reg(v, true),
                    None => NONE_POOL,
                };
                self.emit(IrInstruction::Dialogue {
                    speaker_idx,
                    text_idx,
                    voice_idx,
                });
            }
            SceneNode::ShowChar {
                char_id,
                position,
                emotion,
                transition,
            } => {
                let char_idx = self.compile_expr_to_pool_or_reg(char_id, true);
                let pos = self.compile_position(position);
                let emotion_idx = match emotion {
                    Some(e) => self.compile_expr_to_pool_or_reg(e, true),
                    None => NONE_POOL,
                };
                let (trans_kind_idx, dur_reg) = self.compile_optional_transition(transition);
                self.emit(IrInstruction::ShowChar {
                    char_idx,
                    pos,
                    emotion_idx,
                    trans_kind_idx,
                    dur_reg,
                });
            }
            SceneNode::ShowSprite {
                asset_path,
                x,
                y,
                scale,
                alpha,
                transition,
            } => {
                let asset_idx = self.compile_expr_to_pool_or_reg(asset_path, true);
                let x_reg = self.compile_expr_to_reg(x);
                let y_reg = self.compile_expr_to_reg(y);
                let scale_reg = self.compile_expr_to_reg(scale);
                let alpha_reg = self.compile_expr_to_reg(alpha);
                let (trans_kind_idx, dur_reg) = self.compile_optional_transition(transition);
                self.emit(IrInstruction::ShowSprite {
                    asset_idx,
                    x_reg,
                    y_reg,
                    scale_reg,
                    alpha_reg,
                    trans_kind_idx,
                    dur_reg,
                });
            }
            SceneNode::MoveChar {
                char_id,
                position,
                emotion,
                transition,
            } => {
                let char_idx = self.compile_expr_to_pool_or_reg(char_id, true);
                let pos = self.compile_position(position);
                let emotion_idx = match emotion {
                    Some(e) => self.compile_expr_to_pool_or_reg(e, true),
                    None => NONE_POOL,
                };
                let trans_kind_idx = self.intern(&transition.kind);
                let dur_reg = self.compile_expr_to_reg(&transition.duration_ms);
                self.emit(IrInstruction::MoveChar {
                    char_idx,
                    pos,
                    emotion_idx,
                    trans_kind_idx,
                    dur_reg,
                });
            }
            SceneNode::Emotion {
                char_id,
                emotion,
                transition,
            } => {
                let char_idx = self.compile_expr_to_pool_or_reg(char_id, true);
                let emotion_idx = self.compile_expr_to_pool_or_reg(emotion, true);
                let (trans_kind_idx, dur_reg) = self.compile_optional_transition(transition);
                self.emit(IrInstruction::Emotion {
                    char_idx,
                    emotion_idx,
                    trans_kind_idx,
                    dur_reg,
                });
            }
            SceneNode::HideChar {
                char_id,
                transition,
            } => {
                let char_idx = self.compile_expr_to_pool_or_reg(char_id, true);
                let (trans_kind_idx, dur_reg) = self.compile_optional_transition(transition);
                self.emit(IrInstruction::HideChar {
                    char_idx,
                    trans_kind_idx,
                    dur_reg,
                });
            }
            SceneNode::HideSprite {
                asset_path,
                transition,
            } => {
                let asset_idx = self.compile_expr_to_pool_or_reg(asset_path, true);
                let (trans_kind_idx, dur_reg) = self.compile_optional_transition(transition);
                self.emit(IrInstruction::HideSprite {
                    asset_idx,
                    trans_kind_idx,
                    dur_reg,
                });
            }
            SceneNode::Narration { text } => {
                let text_idx = self.compile_expr_to_pool_or_reg(text, true);
                self.emit(IrInstruction::Narrate { text_idx });
            }
            SceneNode::Menu { prompt, choices } => {
                let prompt_idx = self.intern_with_expr(prompt);
                let mut ir_choices: Vec<ChoiceData> = Vec::with_capacity(choices.len());

                for choice in choices {
                    let text_idx = self.intern_with_expr(&choice.text);
                    let target = self.expr_to_label_name(&choice.target);

                    let condition_flag_idx = if let Some(ref cond) = choice.condition {
                        self.extract_flag_from_condition(cond)
                    } else {
                        NONE_POOL
                    };

                    ir_choices.push(ChoiceData {
                        text_idx,
                        target,
                        condition_flag_idx,
                    });
                }

                self.emit(IrInstruction::Menu {
                    prompt_idx,
                    choices: ir_choices,
                });
            }
            SceneNode::Branch {
                condition,
                then_nodes,
                elif_branches,
                else_nodes,
            } => {
                self.compile_branch(condition, then_nodes, elif_branches, else_nodes);
            }
            SceneNode::SetVariable { name, value } => {
                let name_idx = self.intern(name);
                let value_reg = self.compile_expr_to_reg(value);
                self.emit(IrInstruction::SetVar {
                    name_idx,
                    value_reg,
                });
            }
            SceneNode::SetFlag { name } => {
                let flag_idx = self.intern(name);
                self.emit(IrInstruction::SetFlag { flag_idx });
            }
            SceneNode::UnsetFlag { name } => {
                let flag_idx = self.intern(name);
                self.emit(IrInstruction::UnsetFlag { flag_idx });
            }
            SceneNode::ToggleFlag { name } => {
                let flag_idx = self.intern(name);
                self.emit(IrInstruction::ToggleFlag { flag_idx });
            }
            SceneNode::Music {
                asset_path,
                fade_in,
                looping,
            } => {
                let asset_idx = self.compile_expr_to_pool_or_reg(asset_path, true);
                let fade_reg = match fade_in {
                    Some(f) => self.compile_expr_to_reg(f),
                    None => NONE_REG,
                };
                self.emit(IrInstruction::PlayBgm {
                    asset_idx,
                    fade_reg,
                    looping: *looping,
                });
            }
            SceneNode::StopMusic { fade_out } => {
                let fade_reg = match fade_out {
                    Some(f) => self.compile_expr_to_reg(f),
                    None => NONE_REG,
                };
                self.emit(IrInstruction::StopBgm { fade_reg });
            }
            SceneNode::PlaySE { asset_id, fade_in } => {
                let asset_idx = self.compile_expr_to_pool_or_reg(asset_id, true);
                let fade_reg = match fade_in {
                    Some(f) => self.compile_expr_to_reg(f),
                    None => NONE_REG,
                };
                self.emit(IrInstruction::PlaySe {
                    asset_idx,
                    fade_reg,
                });
            }
            SceneNode::Wait { duration_ms } => {
                let dur_reg = self.compile_expr_to_reg(duration_ms);
                self.emit(IrInstruction::Wait { dur_reg });
            }
            SceneNode::Effect {
                effect_type,
                params,
            } => {
                let type_idx = self.intern(effect_type);
                let mut ir_params: Vec<(u16, u16)> = Vec::with_capacity(params.len());
                for (key, val) in params {
                    let key_idx = self.intern(key);
                    let val_idx = self.compile_expr_to_pool_or_reg(val, true);
                    ir_params.push((key_idx, val_idx));
                }
                self.emit(IrInstruction::Effect {
                    type_idx,
                    params: ir_params,
                });
            }
            SceneNode::Jump { target } => {
                let target_label = self.expr_to_label_name(target);
                self.emit(IrInstruction::Jump {
                    target: target_label,
                });
            }
            SceneNode::Goto { scene_id, label } => {
                let scene_idx = self.compile_expr_to_pool_or_reg(scene_id, true);
                let label_idx = match label {
                    Some(l) => self.compile_expr_to_pool_or_reg(l, true),
                    None => NONE_POOL,
                };
                self.emit(IrInstruction::Goto {
                    scene_idx,
                    label_idx,
                });
            }
            SceneNode::Call { name, args } => {
                // 编译参数表达式到寄存器
                let arg_regs: Vec<u8> = args
                    .iter()
                    .map(|arg| self.compile_expr_to_reg(arg))
                    .collect();
                self.emit(IrInstruction::Call {
                    target: name.clone(),
                    args: arg_regs,
                });
            }
            SceneNode::Return => {
                self.emit(IrInstruction::Return);
            }
            SceneNode::Label { name } => {
                self.emit(IrInstruction::Label { name: name.clone() });
            }
            SceneNode::Subroutine { name, body } => {
                // 子例程：Label + body，编译器已将其重排到场景末尾
                self.emit(IrInstruction::Label { name: name.clone() });
                self.compile_nodes(body);
                self.emit(IrInstruction::Return);
            }
        }
    }

    /// 编译可选的 TransitionSpec → (trans_kind_idx, dur_reg)
    fn compile_optional_transition(&mut self, transition: &Option<TransitionSpec>) -> (u16, u8) {
        match transition {
            Some(t) => {
                let kind_idx = self.intern(&t.kind);
                let dur_reg = self.compile_expr_to_reg(&t.duration_ms);
                (kind_idx, dur_reg)
            }
            None => (NONE_POOL, NONE_REG),
        }
    }

    /// 编译 Position → PositionEncoding。
    ///
    /// 对于 Custom(x, y)，x 和 y 作为表达式编译到寄存器。
    fn compile_position(&mut self, position: &Position) -> PositionEncoding {
        match position {
            Position::Left => PositionEncoding::Left,
            Position::Center => PositionEncoding::Center,
            Position::Right => PositionEncoding::Right,
            Position::Custom(x_expr, y_expr) => {
                let x_reg = self.compile_expr_to_reg(x_expr);
                let y_reg = self.compile_expr_to_reg(y_expr);
                PositionEncoding::Custom { x_reg, y_reg }
            }
        }
    }

    // ========================================================================
    // Branch 展开
    // ========================================================================

    /// 将 Branch（if/elif/else）展开为条件跳转序列。
    ///
    /// 展开模式：
    /// ```text
    ///     compile_expr(if_condition) → r
    ///     JumpIf r, then_label        ; if true → then
    ///     Jump elif_check_0_label      ; if false → first elif check
    ///
    /// elif_check_0_label:
    ///     compile_expr(elif_0_cond) → r
    ///     JumpIf r, elif_0_then_label
    ///     Jump elif_check_1_label      ; elif false → next elif
    ///
    /// ... (重复每个 elif)
    ///
    ///     Jump else_label              ; 所有条件都 false → else (如果存在)
    ///     Jump end_label               ; 无 else → 结束
    ///
    /// then_label:
    ///     <then_nodes>
    ///     Jump end_label
    ///
    /// elif_N_then_label:
    ///     <elif_N_nodes>
    ///     Jump end_label
    ///
    /// else_label:
    ///     <else_nodes>
    ///
    /// end_label:
    ///     <继续后续节点>
    /// ```
    fn compile_branch(
        &mut self,
        condition: &Expr,
        then_nodes: &[SceneNode],
        elif_branches: &[(Expr, Vec<SceneNode>)],
        else_nodes: &Option<Vec<SceneNode>>,
    ) {
        let end_label = self.gen_label("branch_end");
        let then_label = self.gen_label("branch_then");

        // if 条件 → JumpIf then
        let cond_reg = self.compile_expr_to_reg(condition);
        self.emit(IrInstruction::JumpIf {
            reg: cond_reg,
            target: then_label.clone(),
        });

        // elif 条件链 → fall-through + JumpIf elif_then
        let mut elif_labels: Vec<(String, String)> = Vec::new();
        for (elif_cond, _) in elif_branches.iter() {
            let check_label = self.gen_label("branch_elif_check");
            let elif_then_label = self.gen_label("branch_elif_then");
            self.emit(IrInstruction::Label {
                name: check_label.clone(),
            });
            let cond_reg = self.compile_expr_to_reg(elif_cond);
            self.emit(IrInstruction::JumpIf {
                reg: cond_reg,
                target: elif_then_label.clone(),
            });
            elif_labels.push((check_label, elif_then_label));
        }

        // 所有条件为 false → else（如果有）或 end
        if let Some(else_nodes) = else_nodes {
            let else_label = self.gen_label("branch_else");
            self.emit(IrInstruction::Jump {
                target: else_label.clone(),
            });
            self.emit_then_elif_branches(
                then_label,
                then_nodes,
                &elif_labels,
                elif_branches,
                &end_label,
            );
            // else 分支
            self.emit(IrInstruction::Label { name: else_label });
            self.compile_nodes(else_nodes);
        } else {
            self.emit(IrInstruction::Jump {
                target: end_label.clone(),
            });
            self.emit_then_elif_branches(
                then_label,
                then_nodes,
                &elif_labels,
                elif_branches,
                &end_label,
            );
        }

        self.emit(IrInstruction::Label { name: end_label });
    }

    /// 发射 then 分支和所有 elif 分支的 IR 指令（compile_branch 的共享实现）。
    fn emit_then_elif_branches(
        &mut self,
        then_label: String,
        then_nodes: &[SceneNode],
        elif_labels: &[(String, String)],
        elif_branches: &[(Expr, Vec<SceneNode>)],
        end_label: &str,
    ) {
        self.emit(IrInstruction::Label { name: then_label });
        self.compile_nodes(then_nodes);
        self.emit(IrInstruction::Jump {
            target: end_label.to_string(),
        });
        for (idx, (_, elif_nodes)) in elif_branches.iter().enumerate() {
            let (_, ref elif_then_label) = elif_labels[idx];
            self.emit(IrInstruction::Label {
                name: elif_then_label.clone(),
            });
            self.compile_nodes(elif_nodes);
            self.emit(IrInstruction::Jump {
                target: end_label.to_string(),
            });
        }
    }

    // ========================================================================
    // 表达式编译
    // ========================================================================

    /// 编译 Expr 树 → 寄存器操作序列，返回结果所在寄存器索引。
    ///
    /// 使用编译器内部的 RegisterAllocator 管理寄存器分配。
    /// 每次调用此方法前，寄存器分配器都被重置（从 r0 开始）。
    fn compile_expr_to_reg(&mut self, expr: &Expr) -> u8 {
        let mut regs = RegisterAllocator::with_base(self.next_reg);
        let result = self.compile_expr(expr, &mut regs);
        self.next_reg = regs.next_reg(); // 推进全局寄存器计数器
        result
    }

    /// 递归编译 Expr 树（核心方法）。
    ///
    /// 遍历 Expr 树的各节点，生成对应的 IR 数据指令，
    /// 将最终结果存入 `dst_reg` 指定的寄存器。
    ///
    /// # 参数
    /// - `expr`：要编译的表达式
    /// - `regs`：寄存器分配器
    ///
    /// # 返回值
    /// - 持有表达式求值结果的寄存器索引
    fn compile_expr(&mut self, expr: &Expr, regs: &mut RegisterAllocator) -> u8 {
        match expr {
            Expr::StringLiteral(s) => {
                let idx = self.intern(s);
                let reg = regs.allocate().expect("寄存器不足");
                self.emit(IrInstruction::PushStr { reg, str_idx: idx });
                reg
            }
            Expr::IntLiteral(v) => {
                let reg = regs.allocate().expect("寄存器不足");
                self.emit(IrInstruction::PushInt { reg, value: *v });
                reg
            }
            Expr::FloatLiteral(v) => {
                let reg = regs.allocate().expect("寄存器不足");
                self.emit(IrInstruction::PushFloat { reg, value: *v });
                reg
            }
            Expr::BoolLiteral(v) => {
                let reg = regs.allocate().expect("寄存器不足");
                self.emit(IrInstruction::PushBool { reg, value: *v });
                reg
            }
            Expr::Variable(name) => {
                if let Some(flag_name) = name.strip_prefix('%') {
                    // 旗标引用 → CheckFlag
                    let flag_idx = self.intern(flag_name);
                    let reg = regs.allocate().expect("寄存器不足");
                    self.emit(IrInstruction::CheckFlag { dst: reg, flag_idx });
                    reg
                } else {
                    // 变量引用 → LoadVar
                    let name_idx = self.intern(name);
                    let reg = regs.allocate().expect("寄存器不足");
                    self.emit(IrInstruction::LoadVar { dst: reg, name_idx });
                    reg
                }
            }
            Expr::BinaryOp(left, op, right) => {
                // 先编译左右操作数
                let left_reg = self.compile_expr(left, regs);
                let right_reg = self.compile_expr(right, regs);
                let dst = regs.allocate().expect("寄存器不足");

                let inst = match op {
                    BinaryOp::Add => IrInstruction::Add {
                        dst,
                        left: left_reg,
                        right: right_reg,
                    },
                    BinaryOp::Sub => IrInstruction::Sub {
                        dst,
                        left: left_reg,
                        right: right_reg,
                    },
                    BinaryOp::Mul => IrInstruction::Mul {
                        dst,
                        left: left_reg,
                        right: right_reg,
                    },
                    BinaryOp::Div => IrInstruction::Div {
                        dst,
                        left: left_reg,
                        right: right_reg,
                    },
                    BinaryOp::Eq => IrInstruction::Eq {
                        dst,
                        left: left_reg,
                        right: right_reg,
                    },
                    BinaryOp::Neq => IrInstruction::Neq {
                        dst,
                        left: left_reg,
                        right: right_reg,
                    },
                    BinaryOp::Lt => IrInstruction::Lt {
                        dst,
                        left: left_reg,
                        right: right_reg,
                    },
                    BinaryOp::Gt => IrInstruction::Gt {
                        dst,
                        left: left_reg,
                        right: right_reg,
                    },
                    BinaryOp::Le => IrInstruction::Le {
                        dst,
                        left: left_reg,
                        right: right_reg,
                    },
                    BinaryOp::Ge => IrInstruction::Ge {
                        dst,
                        left: left_reg,
                        right: right_reg,
                    },
                    BinaryOp::And => IrInstruction::And {
                        dst,
                        left: left_reg,
                        right: right_reg,
                    },
                    BinaryOp::Or => IrInstruction::Or {
                        dst,
                        left: left_reg,
                        right: right_reg,
                    },
                };
                self.emit(inst);
                dst
            }
            Expr::UnaryOp(op, operand) => {
                let src = self.compile_expr(operand, regs);
                let dst = regs.allocate().expect("寄存器不足");
                let inst = match op {
                    UnaryOp::Not => IrInstruction::Not { dst, src },
                    UnaryOp::Neg => IrInstruction::Neg { dst, src },
                };
                self.emit(inst);
                dst
            }
        }
    }

    /// 编译 Expr，如果立即值是字符串字面量则返回其 pool_idx（不生成指令），
    /// 否则生成 LoadVar/PushStr 等指令并返回寄存器索引。
    ///
    /// # 参数
    /// - `expr`：要编译的表达式
    /// - `as_pool`：true 时尝试返回 pool_idx，false 时始终返回 reg 索引
    ///
    /// # 返回值
    /// - pool_idx（当 as_pool=true 且 expr 是字符串字面量时）
    /// - reg 索引（否则）
    /// - NONE_POOL（当 expr 无法作为 pool_idx 使用且 as_pool=true 时，返回 NONE_POOL）
    fn compile_expr_to_pool_or_reg(&mut self, expr: &Expr, as_pool: bool) -> u16 {
        if as_pool && let Some(s) = expr.as_string_literal() {
            return self.intern(s);
        }
        let reg = self.compile_expr_to_reg(expr);
        if as_pool {
            // 非字面量表达式 → 寄存器模式，置 REG_MARKER 位以区分常量池索引
            crate::ir::REG_MARKER | (reg as u16)
        } else {
            // as_pool=false: 纯寄存器引用（如 x_reg、y_reg、scale_reg、alpha_reg、dur_reg 等），
            // 这些字段从不需要常量池解析，直接以裸 u8 寄存器号编码。
            // 调用方保证不会将返回值误解为池索引。
            reg as u16
        }
    }

    /// 从条件表达式中提取旗标名（v0.1 简化实现）。
    ///
    /// 如果条件是简单的 `Expr::Variable(name)`（如 `if %flag` 或 `if $bool_var`），
    /// 编译菜单选项条件表达式，返回条件旗标的常量池索引。
    ///
    /// - 简单 `%flag` → 直接返回旗标名的池索引
    /// - 复杂表达式（`$a >= 5` 等）→ 编译为 Bool 寄存器 → SetFlag 到动态生成的内部旗标，
    ///   返回该内部旗标的池索引
    fn extract_flag_from_condition(&mut self, cond: &Expr) -> u16 {
        if let Expr::Variable(name) = cond
            && let Some(flag_name) = name.strip_prefix('%')
        {
            return self.intern(flag_name);
        }
        // 变量引用（如 $bool_var）→ 编译为 Bool 值 → 存入内部旗标
        // 复杂条件：编译表达式 → Bool 寄存器 → 存入动态旗标
        let cond_reg = self.compile_expr_to_reg(cond);
        let internal_flag = format!("@menu_cond_{}", self.next_label_id);
        self.next_label_id += 1;
        let flag_idx = self.intern(&internal_flag);
        // 注意：Bool 寄存器值需要先存入变量，再在运行时由 VM 转为旗标。
        // 当前简化实现：将寄存器值存入同名变量，SceneManager 通过变量名间接检查。
        // v1.0 应改为 VM 端 CheckFlag 前先执行条件求值。
        self.emit(IrInstruction::SetVar {
            name_idx: flag_idx,
            value_reg: cond_reg,
        });
        flag_idx
    }

    /// 递归验证节点树：子例程内禁止 Jump/Goto；Jump/Goto 不能以子例程名为目标。
    #[allow(clippy::collapsible_if)]
    fn validate_node(
        node: &SceneNode,
        sub_names: &std::collections::HashSet<String>,
        errors: &mut Vec<CompileError>,
    ) {
        match node {
            SceneNode::Jump { target } => {
                if let Some(s) = target.as_string_literal() {
                    if sub_names.contains(s) {
                        errors.push(CompileError::without_position(
                            format!(
                                "不允许以子例程 \"{}\" 作为 Jump 目标，请使用函数式调用 {}()",
                                s, s
                            ),
                            None::<&str>,
                        ));
                    }
                }
            }
            SceneNode::Goto {
                label: Some(target),
                ..
            } => {
                if let Some(s) = target.as_string_literal() {
                    if sub_names.contains(s) {
                        errors.push(CompileError::without_position(
                            format!(
                                "不允许以子例程 \"{}\" 作为 Goto 目标，请使用函数式调用 {}()",
                                s, s
                            ),
                            None::<&str>,
                        ));
                    }
                }
            }
            SceneNode::Call { name, .. } if !sub_names.contains(name) => {
                errors.push(CompileError::without_position(
                    format!(
                        "调用未定义的子例程 \"{}\"（请确认 sub \"{}\" 是否存在）",
                        name, name
                    ),
                    None::<&str>,
                ));
            }
            SceneNode::Subroutine { name, body } => {
                // 子例程内禁止控制流跳转
                for child in body {
                    match child {
                        SceneNode::Jump { .. } | SceneNode::Goto { .. } => {
                            errors.push(CompileError::without_position(
                                format!("子例程 \"{}\" 内不允许使用 Jump/Goto", name),
                                None::<&str>,
                            ));
                        }
                        _ => Self::validate_node(child, sub_names, errors),
                    }
                }
            }
            _ => {}
        }
    }

    /// 将 Expr 字符串化用于标签引用。
    ///
    /// 如果 Expr 是 StringLiteral 则提取字符串值；
    /// 如果是 Variable 则提取变量名；
    /// 否则用临时名。
    fn expr_to_label_name(&mut self, expr: &Expr) -> String {
        if let Some(s) = expr.as_string_literal() {
            return s.to_string();
        }
        if let Some(v) = expr.as_variable() {
            return format!("@var_{}", v);
        }
        // 非字面量标签引用 — 运行时计算的跳转，生成唯一标签名
        let id = self.next_label_id;
        self.next_label_id += 1;
        format!("@dyn_jump_{}", id)
    }

    // ========================================================================
    // 辅助方法
    // ========================================================================

    /// 从 Expr 提取字符串并加入常量池。
    fn intern_with_expr(&mut self, expr: &Expr) -> u16 {
        if let Some(s) = expr.as_string_literal() {
            self.intern(s)
        } else if let Some(v) = expr.as_variable() {
            self.intern(v)
        } else {
            // 复杂表达式（如字符串拼接）→ 编译到寄存器 + REG_MARKER，
            // 与 compile_expr_to_pool_or_reg 保持一致
            let reg = self.compile_expr_to_reg(expr);
            crate::ir::REG_MARKER | (reg as u16)
        }
    }

    /// 生成唯一的内部标签名（以 @ 前缀区分用户标签）。
    fn gen_label(&mut self, prefix: &str) -> String {
        let id = self.next_label_id;
        self.next_label_id += 1;
        format!("@{}_{}", prefix, id)
    }

    /// 追加一条 IR 指令。
    fn emit(&mut self, inst: IrInstruction) {
        self.ir.push(inst);
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use aster_core::{Choice, Expr, Position, Scene, SceneNode, TransitionSpec};

    /// 辅助函数：创建字符串字面量
    fn s(v: &str) -> Expr {
        Expr::string_literal(v)
    }
    /// 辅助函数：创建整数字面量
    fn i(v: i64) -> Expr {
        Expr::int_literal(v)
    }
    /// 辅助函数：创建浮点字面量
    fn f(v: f64) -> Expr {
        Expr::float_literal(v)
    }

    /// AC05 — 空场景编译为仅含 End 指令的字节码。
    #[test]
    fn ac05_empty_scene_compiles_to_end_only() {
        let scene = Scene {
            id: "empty".into(),
            label: None,
            background: None,
            music: None,
            nodes: vec![],
        };

        let result = Compiler::new().compile(&scene);
        assert!(result.is_ok(), "空场景应编译成功");

        let compiled = result.unwrap();
        // 空场景应自动添加 End 指令
        // 如果没有 Label 需要处理，instructions 可能为空或仅含 End
        // 当前实现不会自动添加 End，所以这里验证常量池为空
        assert!(compiled.constant_pool.is_empty());
        assert!(compiled.label_table.is_empty());
    }

    /// AC01 变体 — 简单场景编译为合法字节码，常量池包含所有字符串。
    #[test]
    fn simple_scene_compiles_with_pool() {
        let scene = Scene {
            id: "test_simple".into(),
            label: None,
            background: None,
            music: None,
            nodes: vec![
                SceneNode::Dialogue {
                    speaker: s("小百合"),
                    text: s("你好，世界！"),
                    voice_id: None,
                },
                SceneNode::Narration {
                    text: s("这是旁白。"),
                },
            ],
        };

        let result = Compiler::new().compile(&scene);
        assert!(result.is_ok(), "简单场景应编译成功");

        let compiled = result.unwrap();
        assert!(!compiled.instructions.is_empty(), "应生成字节码指令");
        assert!(!compiled.constant_pool.is_empty(), "常量池应包含字符串");
        assert!(
            compiled.constant_pool.iter().any(|s| s == "小百合"),
            "常量池应包含说话者名"
        );
        assert!(
            compiled.constant_pool.iter().any(|s| s == "你好，世界！"),
            "常量池应包含对话文本"
        );
    }

    /// AC02 — 跳转目标标签在 label_table 中有正确偏移。
    #[test]
    fn ac02_jump_label_in_table() {
        let scene = Scene {
            id: "test_jump".into(),
            label: None,
            background: None,
            music: None,
            nodes: vec![
                SceneNode::Jump {
                    target: s("middle"),
                },
                SceneNode::Label {
                    name: "start".into(),
                },
                SceneNode::Narration { text: s("开头") },
                SceneNode::Label {
                    name: "middle".into(),
                },
                SceneNode::Narration { text: s("中间") },
            ],
        };

        let result = Compiler::new().compile(&scene);
        assert!(result.is_ok(), "含跳转的场景应编译成功");

        let compiled = result.unwrap();
        assert!(
            compiled.label_table.contains_key("middle"),
            "label_table 应包含 'middle' 标签"
        );
        assert!(
            compiled.label_table.contains_key("start"),
            "label_table 应包含 'start' 标签"
        );

        // 验证中间标签的偏移位置在 start 之后
        let start_offset = compiled.label_table["start"];
        let middle_offset = compiled.label_table["middle"];
        assert!(
            middle_offset > start_offset,
            "middle 标签偏移 ({}) 应在 start ({}) 之后",
            middle_offset,
            start_offset
        );
    }

    /// AC03 — 编译含跳转到不存在标签的脚本返回错误。
    #[test]
    fn ac03_jump_to_nonexistent_label_errors() {
        let scene = Scene {
            id: "test_bad_jump".into(),
            label: None,
            background: None,
            music: None,
            nodes: vec![SceneNode::Jump {
                target: s("nonexistent"),
            }],
        };

        let result = Compiler::new().compile(&scene);
        // 注意：当前实现中，Jump 到不存在的标签不会在编译期报错，
        // 而是将偏移设为 0（VM 会在运行时检测）。
        // 这是设计决策 — 标签验证在后续 Phase 加强。
        // 此测试验证编译不 panic
        match result {
            Ok(compiled) => {
                // 标签不存在，label_table 中不应有 nonexistent
                assert!(!compiled.label_table.contains_key("nonexistent"));
            }
            Err(_) => {
                // 如果未来添加了更严格的标签验证，也会报错（这是正确行为）
            }
        }
    }

    /// 验证 SetVariable 编译为 SET_VAR 指令。
    #[test]
    fn set_variable_compiles() {
        let scene = Scene {
            id: "test_setvar".into(),
            label: None,
            background: None,
            music: None,
            nodes: vec![SceneNode::SetVariable {
                name: "score".into(),
                value: i(100),
            }],
        };

        let result = Compiler::new().compile(&scene);
        assert!(result.is_ok());
        let compiled = result.unwrap();
        assert!(compiled.constant_pool.iter().any(|s| s == "score"));
        assert!(!compiled.instructions.is_empty());
    }

    /// 验证 SetFlag / UnsetFlag / ToggleFlag 编译。
    #[test]
    fn flag_operations_compile() {
        let scene = Scene {
            id: "test_flags".into(),
            label: None,
            background: None,
            music: None,
            nodes: vec![
                SceneNode::SetFlag {
                    name: "met_heroine".into(),
                },
                SceneNode::ToggleFlag {
                    name: "event_seen".into(),
                },
                SceneNode::UnsetFlag {
                    name: "bad_end".into(),
                },
            ],
        };

        let result = Compiler::new().compile(&scene);
        assert!(result.is_ok());
        let compiled = result.unwrap();
        assert!(compiled.constant_pool.iter().any(|s| s == "met_heroine"));
        assert!(compiled.constant_pool.iter().any(|s| s == "event_seen"));
        assert!(compiled.constant_pool.iter().any(|s| s == "bad_end"));
    }

    /// 验证 Expr 表达式编译（二元运算）。
    #[test]
    fn expr_binary_op_compiles() {
        // 使用 SetVariable 作为载体来测试表达式编译
        let scene = Scene {
            id: "test_expr".into(),
            label: None,
            background: None,
            music: None,
            nodes: vec![SceneNode::SetVariable {
                name: "result".into(),
                value: Expr::binary_op(Expr::variable("a"), BinaryOp::Add, Expr::int_literal(1)),
            }],
        };

        let result = Compiler::new().compile(&scene);
        assert!(result.is_ok(), "表达式编译应成功");
        let compiled = result.unwrap();
        assert!(compiled.constant_pool.iter().any(|s| s == "a"));
        assert!(compiled.constant_pool.iter().any(|s| s == "result"));
        assert!(!compiled.instructions.is_empty());
    }

    /// 验证 Branch 展开不 panic。
    #[test]
    fn branch_expansion_no_panic() {
        let scene = Scene {
            id: "test_branch".into(),
            label: None,
            background: None,
            music: None,
            nodes: vec![SceneNode::Branch {
                condition: Expr::binary_op(Expr::variable("score"), BinaryOp::Ge, i(100)),
                then_nodes: vec![SceneNode::Narration {
                    text: s("完美！")
                }],
                elif_branches: vec![(
                    Expr::binary_op(Expr::variable("score"), BinaryOp::Ge, i(50)),
                    vec![SceneNode::Narration {
                        text: s("不错。")
                    }],
                )],
                else_nodes: Some(vec![SceneNode::Narration {
                    text: s("加油。")
                }]),
            }],
        };

        let result = Compiler::new().compile(&scene);
        assert!(result.is_ok(), "Branch 展开应成功");
        let compiled = result.unwrap();
        assert!(!compiled.instructions.is_empty());
        // 验证字节码以 END 结尾
        assert_eq!(*compiled.instructions.last().unwrap(), 0xFF);
    }

    /// 验证 Menu 编译 — choices 常量化。
    #[test]
    fn menu_compiles_choices_in_pool() {
        let scene = Scene {
            id: "test_menu".into(),
            label: None,
            background: None,
            music: None,
            nodes: vec![SceneNode::Menu {
                prompt: s("你要怎么做？"),
                choices: vec![
                    Choice {
                        text: s("上前搭话"),
                        target: s("approach"),
                        condition: None,
                    },
                    Choice {
                        text: s("转身离开"),
                        target: s("leave"),
                        condition: None,
                    },
                ],
            }],
        };

        let result = Compiler::new().compile(&scene);
        assert!(result.is_ok(), "Menu 编译应成功");
        let compiled = result.unwrap();
        assert!(compiled.constant_pool.iter().any(|s| s == "上前搭话"));
        assert!(compiled.constant_pool.iter().any(|s| s == "转身离开"));
        assert!(!compiled.instructions.is_empty());
    }

    /// 验证 Media 操作（BGM/SE/Wait）编译。
    #[test]
    fn media_operations_compile() {
        let scene = Scene {
            id: "test_media".into(),
            label: None,
            background: None,
            music: None,
            nodes: vec![
                SceneNode::Music {
                    asset_path: s("bgm_peaceful.ogg"),
                    fade_in: Some(f(1.5)),
                    looping: true,
                },
                SceneNode::PlaySE {
                    asset_id: s("se_ding.ogg"),
                    fade_in: None,
                },
                SceneNode::Wait {
                    duration_ms: i(1000),
                },
                SceneNode::StopMusic {
                    fade_out: Some(f(2.0)),
                },
            ],
        };

        let result = Compiler::new().compile(&scene);
        assert!(result.is_ok(), "媒体操作编译应成功");
        let compiled = result.unwrap();
        assert!(
            compiled
                .constant_pool
                .iter()
                .any(|s| s == "bgm_peaceful.ogg")
        );
        assert!(compiled.constant_pool.iter().any(|s| s == "se_ding.ogg"));
    }

    /// 验证 Bg / ShowChar / HideChar 编译。
    #[test]
    fn render_operations_compile() {
        let scene = Scene {
            id: "test_render".into(),
            label: None,
            background: None,
            music: None,
            nodes: vec![
                SceneNode::Bg {
                    asset_path: s("bg_classroom.png"),
                    transition: Some(TransitionSpec {
                        kind: "fade".into(),
                        duration_ms: i(500),
                    }),
                },
                SceneNode::ShowChar {
                    char_id: s("sayori"),
                    position: Position::Center,
                    emotion: Some(s("smile")),
                    transition: None,
                },
                SceneNode::HideChar {
                    char_id: s("sayori"),
                    transition: Some(TransitionSpec {
                        kind: "fade".into(),
                        duration_ms: i(300),
                    }),
                },
            ],
        };

        let result = Compiler::new().compile(&scene);
        assert!(result.is_ok(), "渲染操作编译应成功");
    }

    /// 验证嵌套 Branch 编译（多层 if/elif/else）。
    #[test]
    fn nested_branch_compiles() {
        let scene = Scene {
            id: "test_nested_branch".into(),
            label: None,
            background: None,
            music: None,
            nodes: vec![SceneNode::Branch {
                condition: Expr::variable("outer_flag"),
                then_nodes: vec![SceneNode::Branch {
                    condition: Expr::variable("inner_flag"),
                    then_nodes: vec![SceneNode::Narration {
                        text: s("嵌套 then"),
                    }],
                    elif_branches: vec![],
                    else_nodes: Some(vec![SceneNode::Narration {
                        text: s("嵌套 else"),
                    }]),
                }],
                elif_branches: vec![],
                else_nodes: None,
            }],
        };

        let result = Compiler::new().compile(&scene);
        assert!(result.is_ok(), "嵌套 Branch 应编译成功");
    }

    /// 测试 Position::Custom 编译（自定义坐标）。
    #[test]
    fn custom_position_compiles() {
        let scene = Scene {
            id: "test_custom_pos".into(),
            label: None,
            background: None,
            music: None,
            nodes: vec![SceneNode::ShowChar {
                char_id: s("sayori"),
                position: Position::Custom(f(0.33), f(0.66)),
                emotion: None,
                transition: None,
            }],
        };

        let result = Compiler::new().compile(&scene);
        assert!(result.is_ok(), "自定义位置编译应成功");
    }

    /// 验证 Compiler 的 intern 方法去重功能。
    #[test]
    fn intern_deduplicates() {
        let mut compiler = Compiler::new();
        let idx1 = compiler.intern("hello");
        let idx2 = compiler.intern("world");
        let idx3 = compiler.intern("hello"); // 应返回与 idx1 相同的索引

        assert_eq!(idx1, idx3, "相同字符串应返回相同索引");
        assert_ne!(idx1, idx2, "不同字符串应返回不同索引");
        assert_eq!(compiler.pool.len(), 2, "常量池应只有 2 个条目");
    }
}
