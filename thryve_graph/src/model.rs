//! What the window shows, worked out once from the file: each call site with its totals, and the
//! edges in and out of it.

use crate::format::{Edge, Graph};

/// How a call site is drawn and described.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// No other call site's task sent it: it was sent from outside any task (from `main`, say),
    /// or it's a plugin's start task.
    Start,
    /// Tasks from other call sites sent it.
    Task,
    /// The node graph mode added when it stopped recording, with the reason.
    Failure,
}

/// One call site, with what the window needs to show it.
#[derive(Debug, Clone)]
pub struct Site {
    /// The task's name.
    pub name: String,
    /// The name for lists: with its line added when another call site has the same name, as
    /// in `TaskD (line 83)`.
    pub label: String,
    /// The source file, as the export wrote it.
    pub file: String,
    /// The file's last part, for the card: `pipeline.rs` for `src\pipeline.rs`.
    pub file_name: String,
    /// The line that made the task.
    pub line: u32,
    /// The hash Thryve found the node by.
    pub fingerprint: String,
    /// Why graph mode stopped, on the failure node.
    pub failure: Option<String>,
    /// Start, task or failure.
    pub role: Role,
    /// Tasks this line made while a task from another call site was running.
    pub made_by_others: u64,
    /// Tasks this line made while a task from this same line was running: recursion.
    pub made_by_itself: u64,
    /// Tasks other call sites' lines made while this call site's task was running.
    pub sent_to_others: u64,
    /// The edges that end here, busiest first.
    pub incoming: Vec<usize>,
    /// The edges that start here, busiest first.
    pub outgoing: Vec<usize>,
}

impl Site {
    /// Every task the graph saw this line make.
    pub fn made(&self) -> u64 {
        self.made_by_others + self.made_by_itself
    }
}

/// The call sites and edges of one export.
#[derive(Debug, Clone, Default)]
pub struct Model {
    /// The call sites, in the file's order.
    pub sites: Vec<Site>,
    /// The edges, in the file's order.
    pub edges: Vec<Edge>,
    /// The highest edge count, for scaling the arrows.
    pub max_count: u64,
    /// All the edge counts added up: every task the graph saw sent from inside a task.
    pub total_sent: u64,
}

impl Model {
    /// Works out the totals and the lists of edges for each call site.
    pub fn new(graph: Graph) -> Self {
        let mut sites: Vec<Site> = graph
            .nodes
            .into_iter()
            .map(|node| {
                let file = node.callsite.file_path;
                let file_name = file
                    .rsplit(['/', '\\'])
                    .find(|part| !part.is_empty())
                    .unwrap_or_default()
                    .to_string();
                let role = if node.failure.is_some() {
                    Role::Failure
                } else {
                    Role::Start
                };

                Site {
                    label: node.callsite.callsite_name.clone(),
                    name: node.callsite.callsite_name,
                    file,
                    file_name,
                    line: node.callsite.line_number,
                    fingerprint: node.fingerprint,
                    failure: node.failure,
                    role,
                    made_by_others: 0,
                    made_by_itself: 0,
                    sent_to_others: 0,
                    incoming: Vec::new(),
                    outgoing: Vec::new(),
                }
            })
            .collect();

        // Each edge adds to both ends; an edge from another call site makes its target a task.
        for (index, edge) in graph.edges.iter().enumerate() {
            if edge.from == edge.to {
                sites[edge.to].made_by_itself += edge.count;
            } else {
                sites[edge.to].made_by_others += edge.count;
                sites[edge.from].sent_to_others += edge.count;
                if sites[edge.to].role == Role::Start {
                    sites[edge.to].role = Role::Task;
                }
            }
            sites[edge.from].outgoing.push(index);
            sites[edge.to].incoming.push(index);
        }

        // A name shared by several call sites gets each one's line, so lists can tell them apart.
        let mut uses = std::collections::HashMap::<&str, usize>::new();
        for site in &sites {
            *uses.entry(site.name.as_str()).or_default() += 1;
        }
        let shared: Vec<bool> = sites
            .iter()
            .map(|site| uses[site.name.as_str()] > 1)
            .collect();
        for (site, shared) in sites.iter_mut().zip(shared) {
            if shared {
                site.label = format!("{} (line {})", site.name, site.line);
            }
        }

        let edges = graph.edges;
        for site in &mut sites {
            site.incoming
                .sort_by_key(|&l| std::cmp::Reverse(edges[l].count));
            site.outgoing
                .sort_by_key(|&l| std::cmp::Reverse(edges[l].count));
        }

        Self {
            max_count: edges.iter().map(|edge| edge.count).max().unwrap_or(0),
            total_sent: edges.iter().map(|edge| edge.count).sum(),
            sites,
            edges,
        }
    }

    /// The call sites no other call site sent, in the file's order.
    pub fn starts(&self) -> Vec<usize> {
        (0..self.sites.len())
            .filter(|&site| self.sites[site].role == Role::Start)
            .collect()
    }

    /// The node graph mode added when it stopped, if it did.
    pub fn failure(&self) -> Option<usize> {
        self.sites
            .iter()
            .position(|site| site.role == Role::Failure)
    }

    /// The `n` edges with the highest counts, highest first.
    pub fn busiest_edges(&self, n: usize) -> Vec<usize> {
        let mut edges: Vec<usize> = (0..self.edges.len()).collect();
        edges.sort_by_key(|&l| std::cmp::Reverse(self.edges[l].count));
        edges.truncate(n);
        edges
    }

    /// Whether a call site's name or file contains `query`, ignoring letter case. An empty query
    /// matches nothing.
    pub fn matches(&self, site: usize, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return false;
        }

        let site = &self.sites[site];
        site.name.to_lowercase().contains(&query)
            || site.file.to_lowercase().contains(&query)
            || format!("{}:{}", site.file_name, site.line)
                .to_lowercase()
                .contains(&query)
    }

    /// `count` written for people: `1,234,567`.
    pub fn count_text(count: u64) -> String {
        let digits = count.to_string();
        let mut out = String::with_capacity(digits.len() + digits.len() / 3);
        for (i, digit) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                out.push(',');
            }
            out.push(digit);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{CallSite, Node};

    fn node(name: &str, file: &str, line: u32) -> Node {
        Node {
            callsite: CallSite {
                file_path: file.to_string(),
                line_number: line,
                callsite_name: name.to_string(),
            },
            fingerprint: format!("fp-{name}-{line}"),
            failure: None,
        }
    }

    fn edge(from: usize, to: usize, count: u64) -> Edge {
        Edge { from, to, count }
    }

    /// Start sends Hash once; Hash sends LoadImg 5 times; LoadImg sends itself 110 times.
    fn pipeline() -> Model {
        Model::new(Graph {
            nodes: vec![
                node("Start", "src\\main.rs", 10),
                node("Hash", "crates/app/src/hash.rs", 20),
                node("LoadImg", "src/load.rs", 30),
            ],
            edges: vec![edge(0, 1, 1), edge(1, 2, 5), edge(2, 2, 110)],
        })
    }

    #[test]
    fn totals_are_split_between_others_and_recursion() {
        let model = pipeline();
        let load_img = &model.sites[2];

        assert_eq!(load_img.made_by_others, 5);
        assert_eq!(load_img.made_by_itself, 110);
        assert_eq!(load_img.made(), 115);
        assert_eq!(model.sites[1].sent_to_others, 5);
        assert_eq!(model.total_sent, 116);
        assert_eq!(model.max_count, 110);
    }

    #[test]
    fn a_site_only_its_own_task_sends_is_still_a_start() {
        let model = Model::new(Graph {
            nodes: vec![node("Walk", "src/walk.rs", 1)],
            edges: vec![edge(0, 0, 3)],
        });

        assert_eq!(model.sites[0].role, Role::Start);
        assert_eq!(model.starts(), vec![0]);
    }

    #[test]
    fn roles_follow_the_edges_and_the_failure() {
        let mut graph = Graph {
            nodes: vec![node("Start", "a.rs", 1), node("Hash", "b.rs", 2)],
            edges: vec![edge(0, 1, 1)],
        };
        graph.nodes.push(Node {
            failure: Some("an edge pointed to a missing node".to_string()),
            ..node("Graph mode stopped", "", 0)
        });
        let model = Model::new(graph);

        assert_eq!(model.sites[0].role, Role::Start);
        assert_eq!(model.sites[1].role, Role::Task);
        assert_eq!(model.failure(), Some(2));
    }

    #[test]
    fn file_names_come_from_either_kind_of_slash() {
        let model = pipeline();

        assert_eq!(model.sites[0].file_name, "main.rs");
        assert_eq!(model.sites[1].file_name, "hash.rs");
    }

    #[test]
    fn busiest_edges_come_first_and_search_ignores_case() {
        let model = pipeline();

        assert_eq!(model.busiest_edges(2), vec![2, 1]);
        assert!(model.matches(2, "loadimg"));
        assert!(model.matches(1, "HASH.RS:20"));
        assert!(!model.matches(0, "   "));
    }

    #[test]
    fn a_name_shared_by_two_call_sites_gets_each_one_s_line() {
        let model = Model::new(Graph {
            nodes: vec![
                node("TaskC", "src/c.rs", 67),
                node("TaskD", "src/c.rs", 83),
                node("TaskD", "src/c.rs", 89),
            ],
            edges: vec![edge(0, 1, 5), edge(0, 2, 5)],
        });

        assert_eq!(model.sites[0].label, "TaskC");
        assert_eq!(model.sites[1].label, "TaskD (line 83)");
        assert_eq!(model.sites[2].label, "TaskD (line 89)");
    }

    #[test]
    fn counts_are_grouped_by_thousands() {
        assert_eq!(Model::count_text(7), "7");
        assert_eq!(Model::count_text(1234), "1,234");
        assert_eq!(Model::count_text(1234567), "1,234,567");
    }
}
