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
    Module,
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

    pub fn detect_cycles(&self) -> Vec<Vec<PathBuf>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut on_stack = HashSet::new();
        let mut current_path = Vec::new();

        for node in &self.nodes {
            if !visited.contains(node) {
                self.dfs_cycle(
                    node,
                    &mut visited,
                    &mut on_stack,
                    &mut current_path,
                    &mut cycles,
                );
            }
        }
        cycles
    }

    fn dfs_cycle(
        &self,
        current: &PathBuf,
        visited: &mut HashSet<PathBuf>,
        on_stack: &mut HashSet<PathBuf>,
        current_path: &mut Vec<PathBuf>,
        cycles: &mut Vec<Vec<PathBuf>>,
    ) {
        visited.insert(current.clone());
        on_stack.insert(current.clone());
        current_path.push(current.clone());

        if let Some(neighbors) = self.edges.get(current) {
            for next in neighbors {
                if !visited.contains(next) {
                    self.dfs_cycle(next, visited, on_stack, current_path, cycles);
                } else if on_stack.contains(next) {
                    if let Some(pos) = current_path.iter().position(|p| p == next) {
                        let mut cycle = current_path[pos..].to_vec();
                        cycle.push(next.clone());
                        cycles.push(cycle);
                    }
                }
            }
        }

        current_path.pop();
        on_stack.remove(current);
    }
}

pub struct PolyglotAstEngine;

impl PolyglotAstEngine {
    pub fn extract_symbols(source_code: &str, file_ext: &str) -> Vec<SemanticSymbol> {
        let mut symbols = Vec::new();

        match file_ext {
            "rs" => {
                let fn_re =
                    Regex::new(r"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)").unwrap();
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
            "go" => {
                let fn_re =
                    Regex::new(r"(?m)^\s*func\s+(?:\([^)]+\)\s+)?([a-zA-Z0-9_]+)\s*\(").unwrap();
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
                let type_re = Regex::new(r"(?m)^\s*type\s+([a-zA-Z0-9_]+)\s+struct").unwrap();
                for cap in type_re.captures_iter(source_code) {
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
                    Regex::new(r"(?m)^\s*(?:export\s+)?(?:async\s+)?function\s+([a-zA-Z0-9_]+)")
                        .unwrap();
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
                let class_re =
                    Regex::new(r"(?m)^\s*(?:export\s+)?class\s+([a-zA-Z0-9_]+)").unwrap();
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
            "zig" => {
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
                let struct_re =
                    Regex::new(r"(?m)^\s*(?:pub\s+)?const\s+([a-zA-Z0-9_]+)\s*=\s*struct").unwrap();
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
            "java" => {
                let class_re =
                    Regex::new(r"(?m)^\s*(?:public\s+)?(?:final\s+)?class\s+([a-zA-Z0-9_]+)")
                        .unwrap();
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
                let iface_re =
                    Regex::new(r"(?m)^\s*(?:public\s+)?interface\s+([a-zA-Z0-9_]+)").unwrap();
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
            "dart" => {
                let class_re =
                    Regex::new(r"(?m)^\s*(?:abstract\s+)?class\s+([a-zA-Z0-9_]+)").unwrap();
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
            "cpp" | "cc" | "cxx" | "h" | "hpp" => {
                let class_re = Regex::new(r"(?m)^\s*(?:class|struct)\s+([a-zA-Z0-9_]+)").unwrap();
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
        let rust_code = "pub async fn execute_task() {}\npub struct TaskConfig {}";
        let symbols = PolyglotAstEngine::extract_symbols(rust_code, "rs");
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "execute_task");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
        assert_eq!(symbols[1].name, "TaskConfig");
        assert_eq!(symbols[1].kind, SymbolKind::Struct);

        let go_code = "func ProcessQueue() {}\ntype Job struct {}";
        let go_symbols = PolyglotAstEngine::extract_symbols(go_code, "go");
        assert_eq!(go_symbols.len(), 2);
        assert_eq!(go_symbols[0].name, "ProcessQueue");
        assert_eq!(go_symbols[1].name, "Job");

        let zig_code = "pub fn init() {}\npub const Server = struct {};";
        let zig_symbols = PolyglotAstEngine::extract_symbols(zig_code, "zig");
        assert_eq!(zig_symbols.len(), 2);
        assert_eq!(zig_symbols[0].name, "init");
        assert_eq!(zig_symbols[1].name, "Server");
    }

    #[test]
    fn test_dependency_graph_cycle_detection() {
        let mut graph = DependencyGraph::new();
        let a = PathBuf::from("a.rs");
        let b = PathBuf::from("b.rs");
        let c = PathBuf::from("c.rs");

        graph.add_dependency(a.clone(), b.clone());
        graph.add_dependency(b.clone(), c.clone());
        graph.add_dependency(c.clone(), a.clone());

        let cycles = graph.detect_cycles();
        assert!(!cycles.is_empty());
    }
}
