#![cfg(feature = "lp")]

use egg::{EGraph, Id, Language, LpCostFunction, LpExtractor, SymbolLang};
use good_lp::constraint::ConstraintReference;
use good_lp::solvers::WithTimeLimit;
use good_lp::solvers::coin_cbc::{CoinCbcProblem, CoinCbcSolution};
use good_lp::{Constraint, ResolutionError, Solution, SolverModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

#[derive(Default)]
struct SolverStats {
    solve_seconds: f64,
    constraints: usize,
    status: String,
}

struct TimedModel {
    inner: CoinCbcProblem,
    stats: Rc<RefCell<SolverStats>>,
}

impl SolverModel for TimedModel {
    type Solution = CoinCbcSolution;
    type Error = ResolutionError;

    fn solve(self) -> Result<Self::Solution, Self::Error> {
        let start = Instant::now();
        let result = self.inner.solve();
        let mut stats = self.stats.borrow_mut();
        stats.solve_seconds = start.elapsed().as_secs_f64();
        stats.status = match &result {
            Ok(solution) => format!("{:?}", solution.status()),
            Err(error) => format!("Error:{error}"),
        };
        result
    }

    fn add_constraint(&mut self, constraint: Constraint) -> ConstraintReference {
        self.stats.borrow_mut().constraints += 1;
        self.inner.add_constraint(constraint)
    }

    fn name() -> &'static str {
        "timed CBC"
    }
}

impl WithTimeLimit for TimedModel {
    fn with_time_limit<T: Into<f64>>(self, seconds: T) -> Self {
        Self {
            inner: self.inner.with_time_limit(seconds),
            stats: self.stats,
        }
    }
}

struct Cost;

impl LpCostFunction<SymbolLang, ()> for Cost {
    fn node_cost(
        &mut self,
        _egraph: &EGraph<SymbolLang, ()>,
        _eclass: Id,
        node: &SymbolLang,
    ) -> f64 {
        node_cost(node)
    }
}

fn node_cost(node: &SymbolLang) -> f64 {
    let name = node.op.as_str();
    if name == "base" {
        1.0
    } else if name.starts_with("dag_exit") {
        10_000.0
    } else if let Some(cost) = name.strip_prefix("ring_exit_") {
        cost.parse().unwrap()
    } else if name.starts_with("skip") {
        1.2
    } else {
        1.0
    }
}

fn dag(
    depth: usize,
    alternatives: bool,
    duplicate: bool,
) -> (EGraph<SymbolLang, ()>, Vec<Id>, f64) {
    let mut egraph = EGraph::default();
    let base = egraph.add(SymbolLang::leaf("base"));
    let mut ids = vec![base];
    for index in 1..=depth {
        let child = ids[index - 1];
        let children = if duplicate {
            vec![child, child]
        } else {
            vec![child]
        };
        let node = egraph.add(SymbolLang::new(format!("step{index}"), children));
        if alternatives {
            let exit = egraph.add(SymbolLang::leaf(format!("dag_exit{index}")));
            egraph.union(node, exit);
            if index > 1 {
                let skip = egraph.add(SymbolLang::new(
                    format!("skip{index}"),
                    vec![ids[index - 2], base],
                ));
                egraph.union(node, skip);
            }
        }
        ids.push(node);
    }
    egraph.rebuild();
    let expected = if alternatives {
        1.0 + (depth / 2) as f64 * 1.2 + (depth % 2) as f64
    } else {
        (depth + 1) as f64
    };
    (egraph, vec![ids[depth]], expected)
}

fn ring(size: usize) -> (EGraph<SymbolLang, ()>, Vec<Id>, f64) {
    let mut egraph = EGraph::default();
    let ids: Vec<Id> = (0..size)
        .map(|index| {
            let cost = if index == size / 2 { 1 } else { 1000 + index };
            egraph.add(SymbolLang::leaf(format!("ring_exit_{cost}")))
        })
        .collect();
    for index in 0..size {
        let next = egraph.add(SymbolLang::new(
            format!("link{index}"),
            vec![ids[(index + 1) % size]],
        ));
        egraph.union(ids[index], next);
    }
    egraph.rebuild();
    (egraph, vec![ids[0]], (size / 2 + 1) as f64)
}

#[test]
fn benchmark_lp_extraction() {
    let case = std::env::var("PERF_CASE").unwrap_or_else(|_| "duplicate100".into());
    let repeats: usize = std::env::var("PERF_REPEATS")
        .unwrap_or_else(|_| "5".into())
        .parse()
        .unwrap();
    let size: usize = case
        .trim_start_matches(|c: char| !c.is_ascii_digit())
        .parse()
        .unwrap();
    let (egraph, roots, expected) = if case.starts_with("ring") {
        ring(size)
    } else {
        dag(size, case.starts_with("shared"), !case.starts_with("chain"))
    };
    let classes = egraph.number_of_classes();
    let enodes: usize = egraph.classes().map(|class| class.nodes.len()).sum();
    let edges: usize = egraph
        .classes()
        .flat_map(|class| &class.nodes)
        .map(|node| node.children().len())
        .sum();
    let variant = std::env::var("PERF_VARIANT").unwrap_or_else(|_| "unknown".into());
    println!(
        "PERF_HEADER,variant,case,iteration,classes,enodes,edges,constraints,total_seconds,solve_seconds,cost,expected,nodes,status"
    );
    for iteration in 0..=repeats {
        let stats = Rc::new(RefCell::new(SolverStats::default()));
        let solver_stats = stats.clone();
        let solver = move |problem| {
            let mut inner = good_lp::coin_cbc(problem);
            inner.set_log_level(0);
            TimedModel {
                inner,
                stats: solver_stats.clone(),
            }
        };
        let start = Instant::now();
        let mut extractor = LpExtractor::new(&egraph, Cost);
        let (expr, extracted_roots) = extractor.solve_multiple_with_timeout(&roots, solver, 5.0);
        let total_seconds = start.elapsed().as_secs_f64();
        assert!(expr.is_dag());
        assert_eq!(extracted_roots.len(), roots.len());
        let cost: f64 = expr.as_ref().iter().map(node_cost).sum();
        let stats = stats.borrow();
        println!(
            "PERF_ROW,{variant},{case},{iteration},{classes},{enodes},{edges},{},{total_seconds:.9},{:.9},{cost:.6},{expected:.6},{},{status}",
            stats.constraints,
            stats.solve_seconds,
            expr.len(),
            status = stats.status,
        );
        if stats.status != "Optimal" {
            // A cutoff measures incumbent quality, not time to an optimum.
            break;
        }
        if variant != "baseline" {
            assert!((cost - expected).abs() < 1e-6, "incorrect exact optimum");
        }
    }
}
