//! Biodivine BDDを使うbasin実装に共通する、表現変換と変数順序。

use super::bdd_order::BddVariableOrder;
use super::State;
use crate::bn::{BooleanExpr, Network};
use biodivine_lib_bdd::{
    Bdd, BddPartialValuation, BddVariable, BddVariableSet,
};
use std::collections::{HashMap, HashSet};

const DNF_CHUNK_SIZE: usize = 4096;

pub(super) fn lexical_variable_order(network: &Network) -> Vec<String> {
    let mut names = HashSet::new();
    for rule in &network.rules {
        names.insert(rule.target.clone());
        collect_variable_names(&rule.expr, &mut names);
    }
    let mut result = names.into_iter().collect::<Vec<_>>();
    result.sort();
    result
}

fn input_variable_order(network: &Network) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for rule in &network.rules {
        push_unique(&rule.target, &mut seen, &mut result);
        collect_variable_names_ordered(&rule.expr, &mut seen, &mut result);
    }
    result
}

pub(super) fn variable_order(network: &Network, order: BddVariableOrder) -> Vec<String> {
    match order {
        BddVariableOrder::Lexical => lexical_variable_order(network),
        BddVariableOrder::Input => input_variable_order(network),
        BddVariableOrder::Dependency => dependency_variable_order(network),
        BddVariableOrder::MinFill => min_fill_variable_order(network, true),
        BddVariableOrder::MinFillForward => min_fill_variable_order(network, false),
        BddVariableOrder::Feedback => feedback_variable_order(network, false),
        BddVariableOrder::FeedbackReverse => feedback_variable_order(network, true),
    }
}

fn feedback_variable_order(network: &Network, reverse: bool) -> Vec<String> {
    let input_order = input_variable_order(network);
    let rank = input_order
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), index))
        .collect::<HashMap<_, _>>();
    let mut outgoing = input_order
        .iter()
        .map(|name| (name.clone(), HashSet::<String>::new()))
        .collect::<HashMap<_, _>>();
    let mut incoming = outgoing.clone();
    for rule in &network.rules {
        let mut regulators = HashSet::new();
        collect_variable_names(&rule.expr, &mut regulators);
        for regulator in regulators {
            outgoing
                .entry(regulator.clone())
                .or_default()
                .insert(rule.target.clone());
            incoming
                .entry(rule.target.clone())
                .or_default()
                .insert(regulator);
        }
    }

    let mut remaining = input_order.iter().cloned().collect::<HashSet<_>>();
    let mut feedback = Vec::new();
    loop {
        let mut cyclic = remaining.clone();
        loop {
            let peel = cyclic
                .iter()
                .filter(|name| {
                    outgoing[*name].is_disjoint(&cyclic) || incoming[*name].is_disjoint(&cyclic)
                })
                .cloned()
                .collect::<Vec<_>>();
            if peel.is_empty() {
                break;
            }
            for name in peel {
                cyclic.remove(&name);
            }
        }
        if cyclic.is_empty() {
            break;
        }
        let selected = cyclic
            .iter()
            .max_by_key(|name| {
                let out_degree = outgoing[*name].intersection(&cyclic).count();
                let in_degree = incoming[*name].intersection(&cyclic).count();
                (
                    out_degree * in_degree + out_degree + in_degree,
                    usize::MAX - rank[*name],
                )
            })
            .expect("non-empty cyclic core")
            .clone();
        remaining.remove(&selected);
        feedback.push(selected);
    }

    let mut dag = Vec::with_capacity(remaining.len());
    while !remaining.is_empty() {
        let selected = remaining
            .iter()
            .filter(|name| incoming[*name].is_disjoint(&remaining))
            .min_by_key(|name| rank[*name])
            .or_else(|| remaining.iter().min_by_key(|name| rank[*name]))
            .expect("non-empty DAG remainder")
            .clone();
        remaining.remove(&selected);
        dag.push(selected);
    }
    feedback.extend(dag);
    if reverse {
        feedback.reverse();
    }
    feedback
}

/// Build a primal graph in which all variables used by one transition kernel
/// form a clique, then greedily eliminate the variable that introduces the
/// fewest fill edges.  Reversing the elimination sequence gives the usual BDD
/// top-to-bottom order; the forward variant is retained for empirical checks.
fn min_fill_variable_order(network: &Network, reverse: bool) -> Vec<String> {
    let input_order = input_variable_order(network);
    let input_rank = input_order
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), index))
        .collect::<HashMap<_, _>>();
    let mut graph = input_order
        .iter()
        .map(|name| (name.clone(), HashSet::<String>::new()))
        .collect::<HashMap<_, _>>();
    for rule in &network.rules {
        let mut support = HashSet::from([rule.target.clone()]);
        collect_variable_names(&rule.expr, &mut support);
        let support = support.into_iter().collect::<Vec<_>>();
        for left in 0..support.len() {
            for right in left + 1..support.len() {
                graph
                    .entry(support[left].clone())
                    .or_default()
                    .insert(support[right].clone());
                graph
                    .entry(support[right].clone())
                    .or_default()
                    .insert(support[left].clone());
            }
        }
    }

    let mut remaining = input_order.iter().cloned().collect::<HashSet<_>>();
    let mut result = Vec::with_capacity(remaining.len());
    while !remaining.is_empty() {
        let selected = remaining
            .iter()
            .min_by_key(|name| {
                let neighbours = graph[*name]
                    .intersection(&remaining)
                    .filter(|other| *other != *name)
                    .collect::<Vec<_>>();
                let mut fill = 0;
                for left in 0..neighbours.len() {
                    for right in left + 1..neighbours.len() {
                        if !graph[neighbours[left]].contains(neighbours[right]) {
                            fill += 1;
                        }
                    }
                }
                (
                    fill,
                    neighbours.len(),
                    input_rank.get(*name).copied().unwrap_or(usize::MAX),
                )
            })
            .expect("non-empty remaining variables")
            .clone();
        let neighbours = graph[&selected]
            .intersection(&remaining)
            .filter(|name| *name != &selected)
            .cloned()
            .collect::<Vec<_>>();
        for left in 0..neighbours.len() {
            for right in left + 1..neighbours.len() {
                graph
                    .get_mut(&neighbours[left])
                    .expect("known neighbour")
                    .insert(neighbours[right].clone());
                graph
                    .get_mut(&neighbours[right])
                    .expect("known neighbour")
                    .insert(neighbours[left].clone());
            }
        }
        remaining.remove(&selected);
        result.push(selected);
    }
    if reverse {
        result.reverse();
    }
    result
}

fn dependency_variable_order(network: &Network) -> Vec<String> {
    let input_order = input_variable_order(network);
    let input_rank = input_order
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), index))
        .collect::<HashMap<_, _>>();
    let mut graph = input_order
        .iter()
        .map(|name| (name.clone(), HashSet::new()))
        .collect::<HashMap<String, HashSet<String>>>();

    for rule in &network.rules {
        let mut regulators = HashSet::new();
        collect_variable_names(&rule.expr, &mut regulators);
        for regulator in regulators {
            if regulator != rule.target {
                graph
                    .entry(rule.target.clone())
                    .or_default()
                    .insert(regulator.clone());
                graph
                    .entry(regulator)
                    .or_default()
                    .insert(rule.target.clone());
            }
        }
    }

    let mut starts = input_order.clone();
    starts.sort_by_key(|name| {
        (
            graph.get(name).map_or(0, HashSet::len),
            input_rank.get(name).copied().unwrap_or(usize::MAX),
        )
    });
    let mut visited = HashSet::new();
    let mut result = Vec::with_capacity(input_order.len());
    for start in starts {
        dependency_dfs(&start, &graph, &input_rank, &mut visited, &mut result);
    }
    result
}

fn dependency_dfs(
    name: &str,
    graph: &HashMap<String, HashSet<String>>,
    input_rank: &HashMap<String, usize>,
    visited: &mut HashSet<String>,
    result: &mut Vec<String>,
) {
    if !visited.insert(name.to_string()) {
        return;
    }
    result.push(name.to_string());
    let mut neighbours = graph
        .get(name)
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    neighbours.sort_by_key(|next| {
        (
            graph.get(next).map_or(0, HashSet::len),
            input_rank.get(next).copied().unwrap_or(usize::MAX),
        )
    });
    for next in neighbours {
        dependency_dfs(&next, graph, input_rank, visited, result);
    }
}

fn push_unique(name: &str, seen: &mut HashSet<String>, result: &mut Vec<String>) {
    if seen.insert(name.to_string()) {
        result.push(name.to_string());
    }
}

fn collect_variable_names(expr: &BooleanExpr, names: &mut HashSet<String>) {
    match expr {
        BooleanExpr::Var(name) => {
            names.insert(name.clone());
        }
        BooleanExpr::Not(inner) => collect_variable_names(inner, names),
        BooleanExpr::And(left, right) | BooleanExpr::Or(left, right) => {
            collect_variable_names(left, names);
            collect_variable_names(right, names);
        }
    }
}

fn collect_variable_names_ordered(
    expr: &BooleanExpr,
    seen: &mut HashSet<String>,
    result: &mut Vec<String>,
) {
    match expr {
        BooleanExpr::Var(name) => push_unique(name, seen, result),
        BooleanExpr::Not(inner) => collect_variable_names_ordered(inner, seen, result),
        BooleanExpr::And(left, right) | BooleanExpr::Or(left, right) => {
            collect_variable_names_ordered(left, seen, result);
            collect_variable_names_ordered(right, seen, result);
        }
    }
}

pub(super) fn expression_to_bdd(
    expr: &BooleanExpr,
    variables: &BddVariableSet,
) -> Result<Bdd, String> {
    match expr {
        BooleanExpr::Var(name) => variables
            .var_by_name(name)
            .map(|variable| variables.mk_var(variable))
            .ok_or_else(|| format!("Unknown variable '{}' in Boolean expression.", name)),
        BooleanExpr::Not(inner) => Ok(expression_to_bdd(inner, variables)?.not()),
        BooleanExpr::And(left, right) => {
            Ok(expression_to_bdd(left, variables)?.and(&expression_to_bdd(right, variables)?))
        }
        BooleanExpr::Or(left, right) => {
            Ok(expression_to_bdd(left, variables)?.or(&expression_to_bdd(right, variables)?))
        }
    }
}

pub(super) fn validate_states(
    states: &HashSet<State>,
    expected_len: usize,
    label: &str,
) -> Result<(), String> {
    if let Some(state) = states.iter().find(|state| state.len() != expected_len) {
        return Err(format!(
            "Invalid {} state length: expected {}, got {}.",
            label,
            expected_len,
            state.len()
        ));
    }
    Ok(())
}

pub(super) fn states_to_bdd(
    states: &HashSet<State>,
    variables: &BddVariableSet,
    state_variables: &[BddVariable],
) -> Bdd {
    let mut chunks = Vec::new();
    let mut valuations = Vec::with_capacity(DNF_CHUNK_SIZE.min(states.len()));
    for state in states {
        valuations.push(BddPartialValuation::from_values_iter(
            state_variables.iter().copied().zip(state.iter().copied()),
        ));
        if valuations.len() == DNF_CHUNK_SIZE {
            chunks.push(variables.mk_dnf(&valuations));
            valuations.clear();
        }
    }
    if !valuations.is_empty() {
        chunks.push(variables.mk_dnf(&valuations));
    }
    if chunks.is_empty() {
        return variables.mk_false();
    }
    while chunks.len() > 1 {
        let mut merged = Vec::with_capacity((chunks.len() + 1) / 2);
        let mut iter = chunks.into_iter();
        while let Some(left) = iter.next() {
            if let Some(right) = iter.next() {
                merged.push(left.or(&right));
            } else {
                merged.push(left);
            }
        }
        chunks = merged;
    }
    chunks.pop().unwrap()
}
