//! 固定点アトラクタに対する厳密な構造前処理。
//!
//! BDDバックエンドやbasinの版番号には依存せず、簡約後のネットワーク、
//! 依存グラフと厳密に除去できる変数を計算するための部品を提供する。

use super::bdd_utils::expression_to_bdd;
use super::State;
use crate::bn::{BooleanExpr, Network};
use biodivine_lib_bdd::{op_function, Bdd, BddVariableSetBuilder};
use std::collections::{HashMap, HashSet};

pub(super) fn collect_names(expression: &BooleanExpr, names: &mut HashSet<String>) {
    match expression {
        BooleanExpr::Var(name) => {
            names.insert(name.clone());
        }
        BooleanExpr::Not(inner) => collect_names(inner, names),
        BooleanExpr::And(left, right) | BooleanExpr::Or(left, right) => {
            collect_names(left, names);
            collect_names(right, names);
        }
    }
}

enum PartiallyEvaluatedExpr {
    Constant(bool),
    Dynamic(BooleanExpr),
}

pub(super) fn substitute_fixed_inputs(
    expression: &BooleanExpr,
    fixed: &HashMap<String, bool>,
    witness: &str,
) -> BooleanExpr {
    fn simplify(expression: &BooleanExpr, fixed: &HashMap<String, bool>) -> PartiallyEvaluatedExpr {
        match expression {
            BooleanExpr::Var(name) => fixed.get(name).copied().map_or_else(
                || PartiallyEvaluatedExpr::Dynamic(expression.clone()),
                PartiallyEvaluatedExpr::Constant,
            ),
            BooleanExpr::Not(inner) => match simplify(inner, fixed) {
                PartiallyEvaluatedExpr::Constant(value) => PartiallyEvaluatedExpr::Constant(!value),
                PartiallyEvaluatedExpr::Dynamic(inner) => {
                    PartiallyEvaluatedExpr::Dynamic(BooleanExpr::Not(Box::new(inner)))
                }
            },
            BooleanExpr::And(left, right) => {
                match (simplify(left, fixed), simplify(right, fixed)) {
                    (PartiallyEvaluatedExpr::Constant(false), _)
                    | (_, PartiallyEvaluatedExpr::Constant(false)) => {
                        PartiallyEvaluatedExpr::Constant(false)
                    }
                    (PartiallyEvaluatedExpr::Constant(true), expression)
                    | (expression, PartiallyEvaluatedExpr::Constant(true)) => expression,
                    (
                        PartiallyEvaluatedExpr::Dynamic(left),
                        PartiallyEvaluatedExpr::Dynamic(right),
                    ) => PartiallyEvaluatedExpr::Dynamic(BooleanExpr::And(
                        Box::new(left),
                        Box::new(right),
                    )),
                }
            }
            BooleanExpr::Or(left, right) => match (simplify(left, fixed), simplify(right, fixed)) {
                (PartiallyEvaluatedExpr::Constant(true), _)
                | (_, PartiallyEvaluatedExpr::Constant(true)) => {
                    PartiallyEvaluatedExpr::Constant(true)
                }
                (PartiallyEvaluatedExpr::Constant(false), expression)
                | (expression, PartiallyEvaluatedExpr::Constant(false)) => expression,
                (PartiallyEvaluatedExpr::Dynamic(left), PartiallyEvaluatedExpr::Dynamic(right)) => {
                    PartiallyEvaluatedExpr::Dynamic(BooleanExpr::Or(
                        Box::new(left),
                        Box::new(right),
                    ))
                }
            },
        }
    }

    match simplify(expression, fixed) {
        PartiallyEvaluatedExpr::Dynamic(expression) => expression,
        PartiallyEvaluatedExpr::Constant(value) => {
            let variable = BooleanExpr::Var(witness.to_string());
            let negated = BooleanExpr::Not(Box::new(variable.clone()));
            if value {
                BooleanExpr::Or(Box::new(variable), Box::new(negated))
            } else {
                BooleanExpr::And(Box::new(variable), Box::new(negated))
            }
        }
    }
}

/// 論理式に名前が現れるかだけで依存辺を作る、ablation用の基準実装。
/// 意味的supportと違い、相殺されて実際には影響しない変数も依存に残る。
pub(super) fn syntactic_dependencies(
    network: &Network,
    names: &[String],
) -> HashMap<String, HashSet<String>> {
    let mut dependencies = names
        .iter()
        .map(|name| (name.clone(), HashSet::new()))
        .collect::<HashMap<_, _>>();
    for rule in &network.rules {
        collect_names(
            &rule.expr,
            dependencies.entry(rule.target.clone()).or_default(),
        );
    }
    dependencies
}

/// Variables without rules, or with identity rules only, never change. Their
/// 更新規則がない変数と恒等更新だけの変数を検出する。これらの初期値は
/// 固定点Aの値と一致する必要があるため、代入後はBDD変数から除外できる。
pub(super) fn immutable_variables(
    network: &Network,
    names: &[String],
) -> Result<HashSet<String>, String> {
    let mut builder = BddVariableSetBuilder::new();
    for name in names {
        builder.make_variable(name);
    }
    let variables = builder.build();
    let mut mutable = HashSet::new();
    for rule in &network.rules {
        let variable = variables
            .var_by_name(&rule.target)
            .ok_or_else(|| format!("Missing BDD variable '{}'.", rule.target))?;
        if expression_to_bdd(&rule.expr, &variables)? != variables.mk_var(variable) {
            mutable.insert(rule.target.clone());
        }
    }
    Ok(names
        .iter()
        .filter(|name| !mutable.contains(*name))
        .cloned()
        .collect())
}

/// Repeatedly substitute immutable variables at their target values.  A rule
/// can become an identity only after another immutable input is fixed, so a
/// single global identity pass misses useful (and exact) cascades.
pub(super) fn propagate_immutable_variables(
    network: &Network,
    names: &[String],
    fixed_point: &State,
) -> Result<(HashMap<String, bool>, Network), String> {
    let index = names
        .iter()
        .enumerate()
        .map(|(position, name)| (name.clone(), position))
        .collect::<HashMap<_, _>>();
    let mut fixed = HashMap::new();
    loop {
        let reduced = Network {
            rules: network
                .rules
                .iter()
                .filter(|rule| !fixed.contains_key(&rule.target))
                .map(|rule| crate::bn::Rule {
                    target: rule.target.clone(),
                    expr: substitute_fixed_inputs(&rule.expr, &fixed, &rule.target),
                })
                .collect(),
        };
        let immutable = immutable_variables(&reduced, names)?;
        let mut changed = false;
        for name in immutable {
            if !fixed.contains_key(&name) {
                fixed.insert(name.clone(), fixed_point[index[&name]]);
                changed = true;
            }
        }
        if !changed {
            return Ok((fixed, reduced));
        }
    }
}

pub(super) fn strongly_connected_components(
    retained: &HashSet<String>,
    dependencies: &HashMap<String, HashSet<String>>,
) -> Vec<HashSet<String>> {
    fn finish_order(
        node: &str,
        adjacent: &HashMap<String, HashSet<String>>,
        seen: &mut HashSet<String>,
        order: &mut Vec<String>,
    ) {
        if !seen.insert(node.to_string()) {
            return;
        }
        if let Some(next) = adjacent.get(node) {
            for successor in next {
                finish_order(successor, adjacent, seen, order);
            }
        }
        order.push(node.to_string());
    }

    fn collect_component(
        node: &str,
        reverse: &HashMap<String, HashSet<String>>,
        seen: &mut HashSet<String>,
        component: &mut HashSet<String>,
    ) {
        if !seen.insert(node.to_string()) {
            return;
        }
        component.insert(node.to_string());
        if let Some(next) = reverse.get(node) {
            for predecessor in next {
                collect_component(predecessor, reverse, seen, component);
            }
        }
    }

    let mut adjacent = retained
        .iter()
        .map(|name| (name.clone(), HashSet::new()))
        .collect::<HashMap<String, HashSet<String>>>();
    let mut reverse = adjacent.clone();
    for (target, regulators) in dependencies {
        if !retained.contains(target) {
            continue;
        }
        for regulator in regulators.intersection(retained) {
            adjacent
                .get_mut(regulator)
                .expect("retained regulator")
                .insert(target.clone());
            reverse
                .get_mut(target)
                .expect("retained target")
                .insert(regulator.clone());
        }
    }

    let mut order = Vec::with_capacity(retained.len());
    let mut seen = HashSet::new();
    for name in retained {
        finish_order(name, &adjacent, &mut seen, &mut order);
    }
    seen.clear();
    let mut result = Vec::new();
    for name in order.into_iter().rev() {
        if seen.contains(&name) {
            continue;
        }
        let mut component = HashSet::new();
        collect_component(&name, &reverse, &mut seen, &mut component);
        result.push(component);
    }
    result
}

/// Decide whether a small output SCC terminates for every fixed valuation of
/// its external regulators and reaches the local target when those regulators
/// have their target values. This also rules out unfair internal oscillation.
pub(super) fn component_is_globally_convergent(
    network: &Network,
    component: &HashSet<String>,
    fixed_point: &State,
    names: &[String],
    dependencies: &HashMap<String, HashSet<String>>,
) -> Result<bool, String> {
    const EXPLICIT_INPUT_LIMIT: usize = 20;
    const SYMBOLIC_INPUT_LIMIT: usize = 24;
    if component.is_empty() {
        return Ok(false);
    }
    let full_index = names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), index))
        .collect::<HashMap<_, _>>();
    let mut local_names = component.iter().cloned().collect::<Vec<_>>();
    local_names.sort();
    let mut external_names = local_names
        .iter()
        .flat_map(|target| dependencies.get(target).into_iter().flatten())
        .filter(|regulator| !component.contains(*regulator))
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    external_names.sort();
    if local_names.len() + external_names.len() > EXPLICIT_INPUT_LIMIT {
        if local_names.len() + external_names.len() > SYMBOLIC_INPUT_LIMIT {
            return Ok(false);
        }
        return component_is_symbolically_terminating(network, component, fixed_point, names);
    }
    let rules = local_names
        .iter()
        .map(|target| {
            network
                .rules
                .iter()
                .filter(|rule| &rule.target == target)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let target_state = local_names
        .iter()
        .enumerate()
        .fold(0usize, |state, (bit, name)| {
            state | (usize::from(fixed_point[full_index[name]]) << bit)
        });
    let target_external = external_names
        .iter()
        .enumerate()
        .fold(0usize, |state, (bit, name)| {
            state | (usize::from(fixed_point[full_index[name]]) << bit)
        });
    let state_count = 1usize << local_names.len();
    for external_state in 0..(1usize << external_names.len()) {
        let mut enabled = vec![0usize; state_count];
        for (local_state, enabled_mask) in enabled.iter_mut().enumerate() {
            let mut full_state = fixed_point.clone();
            for (bit, name) in external_names.iter().enumerate() {
                full_state[full_index[name]] = external_state & (1usize << bit) != 0;
            }
            for (bit, name) in local_names.iter().enumerate() {
                full_state[full_index[name]] = local_state & (1usize << bit) != 0;
            }
            for (bit, target_rules) in rules.iter().enumerate() {
                let current = local_state & (1usize << bit) != 0;
                if target_rules
                    .iter()
                    .any(|rule| evaluate(&rule.expr, &full_state, &full_index) != current)
                {
                    *enabled_mask |= 1usize << bit;
                }
            }
        }

        // With target-valued inputs, acyclicity plus a unique terminal state
        // is exactly universal convergence to the local target.
        if external_state == target_external
            && enabled
                .iter()
                .enumerate()
                .any(|(state, mask)| *mask == 0 && state != target_state)
        {
            return Ok(false);
        }

        // For every valuation of the upstream inputs, exclude internal
        // cycles. Otherwise an unfair path could postpone an enabled upstream
        // transition forever by circulating inside this output SCC.
        let mut indegree = vec![0u32; state_count];
        for (state, mask) in enabled.iter().copied().enumerate() {
            for bit in 0..local_names.len() {
                if mask & (1usize << bit) != 0 {
                    indegree[state ^ (1usize << bit)] += 1;
                }
            }
        }
        let mut queue = indegree
            .iter()
            .enumerate()
            .filter_map(|(state, degree)| (*degree == 0).then_some(state))
            .collect::<std::collections::VecDeque<_>>();
        let mut removed = 0usize;
        while let Some(state) = queue.pop_front() {
            removed += 1;
            let mask = enabled[state];
            for bit in 0..local_names.len() {
                if mask & (1usize << bit) == 0 {
                    continue;
                }
                let successor = state ^ (1usize << bit);
                indegree[successor] -= 1;
                if indegree[successor] == 0 {
                    queue.push_back(successor);
                }
            }
        }
        if removed != state_count {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Symbolically prove the same termination condition used by the explicit
/// checker. `EG(true)` is evaluated using only transitions internal to the
/// component; external regulators are never flipped and are therefore checked
/// for all valuations simultaneously. Resource limits make failure to prove
/// the property conservative.
pub(super) fn component_is_symbolically_terminating(
    network: &Network,
    component: &HashSet<String>,
    fixed_point: &State,
    names: &[String],
) -> Result<bool, String> {
    const NODE_LIMIT: usize = 250_000;
    const ITERATION_LIMIT: usize = 256;

    let full_index = names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), index))
        .collect::<HashMap<_, _>>();
    let mut builder = BddVariableSetBuilder::new();
    for name in names {
        builder.make_variable(name);
    }
    let variables = builder.build();
    let mut kernels = Vec::new();
    for target in component {
        let variable = variables
            .var_by_name(target)
            .ok_or_else(|| format!("Missing BDD variable '{}'.", target))?;
        let target_bdd = variables.mk_var(variable);
        let mut enabled = variables.mk_false();
        for rule in network.rules.iter().filter(|rule| &rule.target == target) {
            let rule_enabled = target_bdd.xor(&expression_to_bdd(&rule.expr, &variables)?);
            let Some(next_enabled) =
                Bdd::binary_op_with_limit(NODE_LIMIT, &enabled, &rule_enabled, op_function::or)
            else {
                return Ok(false);
            };
            enabled = next_enabled;
        }
        if !enabled.is_false() {
            kernels.push((variable, enabled));
        }
    }

    let mut infinite = variables.mk_true();
    let mut stable = false;
    for _ in 0..ITERATION_LIMIT {
        let mut predecessor = variables.mk_false();
        for (variable, enabled) in &kernels {
            let Some(term) = Bdd::fused_binary_flip_op_with_limit(
                NODE_LIMIT,
                (enabled, None),
                (&infinite, Some(*variable)),
                None,
                op_function::and,
            ) else {
                return Ok(false);
            };
            let Some(next_predecessor) =
                Bdd::binary_op_with_limit(NODE_LIMIT, &predecessor, &term, op_function::or)
            else {
                return Ok(false);
            };
            predecessor = next_predecessor;
        }
        if predecessor == infinite {
            stable = true;
            break;
        }
        infinite = predecessor;
    }
    if !stable || !infinite.is_false() {
        return Ok(false);
    }

    let mut enabled_any = variables.mk_false();
    for (_, enabled) in &kernels {
        let Some(next_enabled) =
            Bdd::binary_op_with_limit(NODE_LIMIT, &enabled_any, enabled, op_function::or)
        else {
            return Ok(false);
        };
        enabled_any = next_enabled;
    }
    let mut target_inputs_deadlock = enabled_any.not();
    for name in names.iter().filter(|name| !component.contains(*name)) {
        let variable = variables
            .var_by_name(name)
            .ok_or_else(|| format!("Missing BDD variable '{}'.", name))?;
        target_inputs_deadlock =
            target_inputs_deadlock.var_restrict(variable, fixed_point[full_index[name]]);
    }
    let mut target_cube = variables.mk_true();
    for name in component {
        let variable = variables
            .var_by_name(name)
            .ok_or_else(|| format!("Missing BDD variable '{}'.", name))?;
        target_cube =
            target_cube.and(&variables.mk_literal(variable, fixed_point[full_index[name]]));
    }
    Ok(target_inputs_deadlock.and(&target_cube.not()).is_false())
}

/// Repeatedly remove sink SCCs that are guaranteed to settle after their
/// upstream regulators settle. Such an SCC contributes every one of its local
/// states to the global basin and therefore only multiplies its cardinality.
pub(super) fn prune_convergent_sink_components(
    network: &Network,
    fixed_point: &State,
    names: &[String],
    dependencies: &HashMap<String, HashSet<String>>,
    mut retained: HashSet<String>,
) -> Result<HashSet<String>, String> {
    loop {
        let mut removable = HashSet::new();
        for component in strongly_connected_components(&retained, dependencies) {
            let is_sink = component.iter().all(|name| {
                dependencies.iter().all(|(target, regulators)| {
                    !regulators.contains(name)
                        || !retained.contains(target)
                        || component.contains(target)
                })
            });
            if is_sink
                && component_is_globally_convergent(
                    network,
                    &component,
                    fixed_point,
                    names,
                    dependencies,
                )?
            {
                removable.extend(component);
            }
        }
        if removable.is_empty() {
            return Ok(retained);
        }
        retained.retain(|name| !removable.contains(name));
    }
}

pub(super) fn evaluate(
    expression: &BooleanExpr,
    state: &State,
    index: &HashMap<String, usize>,
) -> bool {
    match expression {
        BooleanExpr::Var(name) => state[index[name]],
        BooleanExpr::Not(inner) => !evaluate(inner, state, index),
        BooleanExpr::And(left, right) => {
            evaluate(left, state, index) && evaluate(right, state, index)
        }
        BooleanExpr::Or(left, right) => {
            evaluate(left, state, index) || evaluate(right, state, index)
        }
    }
}

pub(super) fn is_fixed_point(network: &Network, state: &State, names: &[String]) -> bool {
    let index = names
        .iter()
        .enumerate()
        .map(|(position, name)| (name.clone(), position))
        .collect::<HashMap<_, _>>();
    network.rules.iter().all(|rule| {
        index
            .get(&rule.target)
            .is_some_and(|target| evaluate(&rule.expr, state, &index) == state[*target])
    })
}
