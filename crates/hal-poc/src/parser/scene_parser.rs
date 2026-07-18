//! 场景级解析：tag 解析、assign 解析、整个 .tscn 顶层流程。
//!
//! 移植自：
//! - `core/variant/variant_parser.cpp` 的 `parse_tag` / `_parse_tag` / `parse_tag_assign_eof`
//! - `scene/resources/resource_format_text.cpp` 的 `ResourceLoaderText::open` / `load`

use crate::parser::lexer::{get_token, TokenType};
use crate::parser::stream::Stream;
use crate::parser::value::{parse_value, ParseError};
use crate::types::{
    ExtResource, SceneConnection, SceneData, SceneHeader, SceneNode, SubResource, Variant,
};

/// 解析整个 .tscn 文本，返回 SceneData。这是对外主入口。
pub fn parse_scene(input: &str) -> Result<SceneData, ParseError> {
    let mut stream = Stream::new(input);
    let mut loader = SceneLoader::default();
    loader.run(&mut stream)?;
    Ok(loader.finish())
}

/// 场景装载器：对应 Godot `ResourceLoaderText`。
#[derive(Default)]
struct SceneLoader {
    header: Option<SceneHeader>,
    ext_resources: Vec<ExtResource>,
    sub_resources: Vec<SubResource>,
    nodes: Vec<SceneNode>,
    connections: Vec<SceneConnection>,
    editable_instances: Vec<String>,
    /// 当前正在构建的 node / sub_resource（在 assign 之间累积 props）
    current_node: Option<SceneNode>,
    current_sub: Option<SubResource>,
    current_target: CurrentTarget,
}

#[derive(Default, PartialEq, Clone, Copy)]
enum CurrentTarget {
    #[default]
    None,
    Node,
    SubResource,
}

impl SceneLoader {
    fn run(&mut self, stream: &mut Stream) -> Result<(), ParseError> {
        loop {
            // 读到一个 tag 或一个 assign
            match self.parse_tag_or_assign(stream)? {
                Item::Tag(tag) => {
                    self.flush_current();
                    self.handle_tag(tag)?;
                }
                Item::Assign { key, value } => {
                    self.handle_assign(key, value)?;
                }
                Item::Eof => break,
            }
        }
        self.flush_current();
        Ok(())
    }

    fn handle_tag(&mut self, tag: Tag) -> Result<(), ParseError> {
        match tag.name.as_str() {
            "gd_scene" | "gd_resource" => {
                self.header = Some(SceneHeader {
                    format: tag.get_int("format").unwrap_or(3) as u32,
                    uid: tag.get_string("uid"),
                    load_steps: tag.get_int("load_steps").map(|i| i as u32),
                });
                self.current_target = CurrentTarget::None;
            }
            "ext_resource" => {
                self.ext_resources.push(ExtResource {
                    r#type: tag.get_string("type").unwrap_or_default(),
                    path: tag.get_string("path").unwrap_or_default(),
                    id: tag.get_string("id").unwrap_or_default(),
                    uid: tag.get_string("uid"),
                });
                self.current_target = CurrentTarget::None;
            }
            "sub_resource" => {
                self.current_sub = Some(SubResource {
                    r#type: tag.get_string("type").unwrap_or_default(),
                    id: tag.get_string("id").unwrap_or_default(),
                    props: Vec::new(),
                });
                self.current_target = CurrentTarget::SubResource;
            }
            "node" => {
                self.current_node = Some(SceneNode {
                    name: tag.get_string("name").unwrap_or_default(),
                    r#type: tag.get_string("type"),
                    parent: tag.get_string("parent"),
                    index: tag.get_int("index").map(|i| i as i32),
                    instance: tag.get_resource_ref("instance"),
                    instance_placeholder: tag.get_string("instance_placeholder"),
                    owner: tag.get_string("owner"),
                    unique_id: tag.get_int("unique_id"),
                    groups: tag.get_string_list("groups"),
                    deferred_node_paths: tag.get_string_list("node_paths"),
                    props: Vec::new(),
                });
                self.current_target = CurrentTarget::Node;
            }
            "connection" => {
                self.connections.push(SceneConnection {
                    signal: tag.get_string("signal").unwrap_or_default(),
                    from: tag.get_string("from").unwrap_or_default(),
                    to: tag.get_string("to").unwrap_or_default(),
                    method: tag.get_string("method").unwrap_or_default(),
                    flags: tag.get_int("flags").map(|i| i as i32),
                    binds: tag.get_array("binds"),
                    unbinds: tag.get_int("unbinds").map(|i| i as i32),
                });
                self.current_target = CurrentTarget::None;
            }
            "editable" => {
                if let Some(path) = tag.get_string("path") {
                    self.editable_instances.push(path);
                }
                self.current_target = CurrentTarget::None;
            }
            _ => {
                // 未知 tag，忽略但停止当前累积
                self.current_target = CurrentTarget::None;
            }
        }
        Ok(())
    }

    fn handle_assign(&mut self, key: String, value: Variant) -> Result<(), ParseError> {
        match self.current_target {
            CurrentTarget::Node => {
                if let Some(n) = &mut self.current_node {
                    n.props.push((key, value));
                }
            }
            CurrentTarget::SubResource => {
                if let Some(s) = &mut self.current_sub {
                    s.props.push((key, value));
                }
            }
            CurrentTarget::None => {
                // 顶层属性（如 gd_resource 文件的全局属性），暂忽略
            }
        }
        Ok(())
    }

    fn flush_current(&mut self) {
        if let Some(node) = self.current_node.take() {
            self.nodes.push(node);
        }
        if let Some(sub) = self.current_sub.take() {
            self.sub_resources.push(sub);
        }
        self.current_target = CurrentTarget::None;
    }

    fn finish(self) -> SceneData {
        SceneData {
            header: self.header.unwrap_or(SceneHeader {
                format: 3,
                uid: None,
                load_steps: None,
            }),
            ext_resources: self.ext_resources,
            sub_resources: self.sub_resources,
            nodes: self.nodes,
            connections: self.connections,
            editable_instances: self.editable_instances,
        }
    }

    /// 主循环：读一个 tag（`[...]`）或一个 assign（`key = value`），或 EOF。
    /// 移植自 Godot `parse_tag_assign_eof`。
    fn parse_tag_or_assign(&self, stream: &mut Stream) -> Result<Item, ParseError> {
        let mut what = String::new();

        loop {
            let c = stream.get_char();
            if c == '\0' && stream.is_eof() {
                return Ok(Item::Eof);
            }
            if c == ';' {
                // 行注释
                loop {
                    let ch = stream.get_char();
                    if ch == '\0' && stream.is_eof() {
                        return Ok(Item::Eof);
                    }
                    if ch == '\n' {
                        break;
                    }
                }
                continue;
            }
            if c == '[' && what.is_empty() {
                // 这是一个 tag
                stream.saved = '[';
                let tag = parse_tag(stream)?;
                return Ok(Item::Tag(tag));
            }
            if c == '#' && what.is_empty() {
                // 行注释（Godot .tscn 用 # 做注释）
                loop {
                    let ch = stream.get_char();
                    if ch == '\0' && stream.is_eof() {
                        return Ok(Item::Eof);
                    }
                    if ch == '\n' {
                        break;
                    }
                }
                continue;
            }
            if c == '"' {
                // 引号字符串（key 里罕见，但 parse_tag_assign_eof 支持）
                stream.saved = '"';
                let tk = get_token(stream)?;
                if let Variant::String(s) = tk.value {
                    what = s;
                }
                continue;
            }
            if c == '=' && !what.is_empty() {
                // 找到 assign
                let token = get_token(stream)?;
                let value = parse_value(token, stream)?;
                return Ok(Item::Assign {
                    key: what,
                    value,
                });
            }
            if c > ' ' {
                // 累积 key 字符（含 / @ . - 等特殊字符）
                what.push(c);
            }
            // c <= ' '（空白/换行）：忽略
        }
    }
}

enum Item {
    Tag(Tag),
    Assign { key: String, value: Variant },
    Eof,
}

/// 一个 tag，如 `[node name="X" type="Y"]`。对应 Godot `VariantParser::Tag`。
#[derive(Debug, Clone)]
struct Tag {
    name: String,
    fields: Vec<(String, Variant)>,
}

impl Tag {
    fn get_string(&self, key: &str) -> Option<String> {
        self.fields.iter().find(|(k, _)| k == key).and_then(|(_, v)| {
            if let Variant::String(s) | Variant::StringName(s) = v {
                Some(s.clone())
            } else {
                None
            }
        })
    }

    fn get_int(&self, key: &str) -> Option<i64> {
        self.fields.iter().find(|(k, _)| k == key).and_then(|(_, v)| match v {
            Variant::Int(i) => Some(*i),
            Variant::Float(f) => Some(*f as i64),
            _ => None,
        })
    }

    fn get_resource_ref(&self, key: &str) -> Option<String> {
        self.fields.iter().find(|(k, _)| k == key).and_then(|(_, v)| match v {
            Variant::ExtResource(id) | Variant::SubResource(id) => Some(id.clone()),
            _ => None,
        })
    }

    fn get_string_list(&self, key: &str) -> Vec<String> {
        self.fields
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| match v {
                Variant::Array(items) | Variant::TypedArray { items, .. } => {
                    let strs: Vec<String> = items
                        .iter()
                        .filter_map(|item| match item {
                            Variant::String(s) | Variant::StringName(s) => Some(s.clone()),
                            _ => None,
                        })
                        .collect();
                    Some(strs)
                }
                _ => None,
            })
            .unwrap_or_default()
    }

    fn get_array(&self, key: &str) -> Vec<Variant> {
        self.fields
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| match v {
                Variant::Array(items) | Variant::TypedArray { items, .. } => Some(items.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }
}

/// 解析一个 tag（已知道流里有 `[`）。对应 Godot `_parse_tag`（非 simple 模式）。
fn parse_tag(stream: &mut Stream) -> Result<Tag, ParseError> {
    let open = get_token(stream)?;
    if open.ty != TokenType::BracketOpen {
        return Err(ParseError {
            line: stream.line,
            message: "Expected '['".into(),
        });
    }
    let mut name = String::new();
    let mut fields = Vec::new();

    // tag 名（标识符）
    let name_token = get_token(stream)?;
    if name_token.ty != TokenType::Identifier {
        return Err(ParseError {
            line: stream.line,
            message: "Expected identifier (tag name)".into(),
        });
    }
    name.push_str(&name_token.raw);

    // 解析后续：可能是 `]` 结束，或 `.identifier` / `:identifier`（tag 后缀如 platform），
    // 或 `key = value` 字段。
    loop {
        let token = get_token(stream)?;
        if token.ty == TokenType::BracketClose {
            break;
        }
        if token.ty == TokenType::Period {
            name.push('.');
            let next = get_token(stream)?;
            if next.ty != TokenType::Identifier {
                return Err(ParseError {
                    line: stream.line,
                    message: "Expected identifier after '.'".into(),
                });
            }
            name.push_str(&next.raw);
            continue;
        }
        if token.ty == TokenType::Colon {
            name.push(':');
            let next = get_token(stream)?;
            if next.ty != TokenType::Identifier {
                return Err(ParseError {
                    line: stream.line,
                    message: "Expected identifier after ':'".into(),
                });
            }
            name.push_str(&next.raw);
            continue;
        }
        // 字段：identifier = value
        if token.ty != TokenType::Identifier {
            return Err(ParseError {
                line: stream.line,
                message: format!("Unexpected token in tag: {:?}", token.ty),
            });
        }
        let key = token.raw;
        let eq = get_token(stream)?;
        if eq.ty != TokenType::Equal {
            return Err(ParseError {
                line: stream.line,
                message: "Expected '=' after identifier".into(),
            });
        }
        let value_token = get_token(stream)?;
        let value = parse_value(value_token, stream)?;
        fields.push((key, value));
    }

    Ok(Tag { name, fields })
}
