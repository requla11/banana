use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Struct,
    Class,
    Interface,
    Import,
    Variable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub line: usize,
    pub references: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    pub nodes: HashSet<PathBuf>,
    pub edges: HashMap<PathBuf, HashSet<PathBuf>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_dependency(&mut self, from: PathBuf, to: PathBuf) {
        self.nodes.insert(from.clone());
        self.nodes.insert(to.clone());
        self.edges.entry(from).or_default().insert(to);
    }

    pub fn get_dependencies(&self, file: &Path) -> Vec<PathBuf> {
        self.edges
            .get(file)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }
}

pub struct PolyglotAstEngine;

impl PolyglotAstEngine {
    pub fn extract_symbols(source_code: &str, file_ext: &str) -> Vec<SemanticSymbol> {
        let mut symbols = Vec::new();

        match file_ext {
            "rs" => {
                let fn_re = Regex::new(r"(?m)^\s*(?:pub\s+)?fn\s+([a-zA-Z0-9_]+)").unwrap();
                for cap in fn_re.captures_iter(source_code) {
                    if let Some(m) = cap.get(1) {
                        symbols.push(SemanticSymbol {
                            name: m.as_str().to_string(),
                            kind: SymbolKind::Function,
                            line: 1,
                            references: Vec::new(),
                        });
                    }
                }
                let struct_re = Regex::new(r"(?m)^\s*(?:pub\s+)?struct\s+([a-zA-Z0-9_]+)").unwrap();
                for cap in struct_re.captures_iter(source_code) {
                    if let Some(m) = cap.get(1) {
                        symbols.push(SemanticSymbol {
                            name: m.as_str().to_string(),
                            kind: SymbolKind::Struct,
                            line: 1,
                            references: Vec::new(),
                        });
                    }
                }
            }
            "py" => {
                let def_re = Regex::new(r"(?m)^\s*def\s+([a-zA-Z0-9_]+)").unwrap();
                for cap in def_re.captures_iter(source_code) {
                    if let Some(m) = cap.get(1) {
                        symbols.push(SemanticSymbol {
                            name: m.as_str().to_string(),
                            kind: SymbolKind::Function,
                            line: 1,
                            references: Vec::new(),
                        });
                    }
                }
                let class_re = Regex::new(r"(?m)^\s*class\s+([a-zA-Z0-9_]+)").unwrap();
                for cap in class_re.captures_iter(source_code) {
                    if let Some(m) = cap.get(1) {
                        symbols.push(SemanticSymbol {
                            name: m.as_str().to_string(),
                            kind: SymbolKind::Class,
                            line: 1,
                            references: Vec::new(),
                        });
                    }
                }
            }
            "ts" | "js" => {
                let fn_re =
                    Regex::new(r"(?m)^\s*(?:export\s+)?function\s+([a-zA-Z0-9_]+)").unwrap();
                for cap in fn_re.captures_iter(source_code) {
                    if let Some(m) = cap.get(1) {
                        symbols.push(SemanticSymbol {
                            name: m.as_str().to_string(),
                            kind: SymbolKind::Function,
                            line: 1,
                            references: Vec::new(),
                        });
                    }
                }
                let iface_re =
                    Regex::new(r"(?m)^\s*(?:export\s+)?interface\s+([a-zA-Z0-9_]+)").unwrap();
                for cap in iface_re.captures_iter(source_code) {
                    if let Some(m) = cap.get(1) {
                        symbols.push(SemanticSymbol {
                            name: m.as_str().to_string(),
                            kind: SymbolKind::Interface,
                            line: 1,
                            references: Vec::new(),
                        });
                    }
                }
            }
            _ => {}
        }

        symbols
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polyglot_ast_extraction() {
        let rust_code = "pub fn execute_task() {}\npub struct TaskConfig {}";
        let symbols = PolyglotAstEngine::extract_symbols(rust_code, "rs");
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "execute_task");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
        assert_eq!(symbols[1].name, "TaskConfig");
        assert_eq!(symbols[1].kind, SymbolKind::Struct);

        let py_code = "class Worker:\n    def run(self):\n        pass";
        let py_symbols = PolyglotAstEngine::extract_symbols(py_code, "py");
        assert_eq!(py_symbols.len(), 2);
        assert_eq!(py_symbols[0].name, "run");
        assert_eq!(py_symbols[1].name, "Worker");
    }
}
