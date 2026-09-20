//! CyBER: Biodivine BDDだけでcycle-free basinを求める実装。
//!
//! 前処理と固定点エンジンをこのファイルに置き、版番号を持たない
//! 共通部品だけを利用する。basin7/basin11など他方式の実装には依存しない。

use super::State;
use super::basin_reduction::{
    is_fixed_point, propagate_immutable_variables, prune_convergent_sink_components,
    syntactic_dependencies,
};
use super::bdd_order::BddVariableOrder;
use super::bdd_utils::{
    expression_to_bdd, lexical_variable_order, states_to_bdd, validate_states, variable_order,
};
use crate::bn::Network;
use crate::{Logger, log};
use biodivine_lib_bdd::{Bdd, BddVariable, BddVariableSet, BddVariableSetBuilder, op_function};
use num_bigint::BigUint;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

pub struct CyberResult {
    pub basin_size: String,
    pub iterations: usize,
    pub basin_nodes: usize,
    pub peak_nodes: usize,
    pub active_rules: usize,
    pub skipped_rules: usize,
    pub variable_order: BddVariableOrder,
    pub retained_variables: usize,
    pub pruned_variables: usize,
    pub sink_scc_variables_before: Option<usize>,
    pub sink_scc_variables_after: Option<usize>,
}

/// 論文とcactus plotで比較するCyBERの6構成。
///
/// 任意のfeature組合せは公開せず、実装名と実験名を一対一に保つ。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CyberConfiguration {
    Full,
    ForwardBaseline,
    ComplementGfp,
    ComplementGfpPlusImmutable,
    ComplementGfpPlusSinkScc,
    ComplementGfpPlusDependencyOrder,
}

impl Default for CyberConfiguration {
    fn default() -> Self {
        Self::Full
    }
}

impl CyberConfiguration {
    pub const ALL: [Self; 6] = [
        Self::Full,
        Self::ForwardBaseline,
        Self::ComplementGfp,
        Self::ComplementGfpPlusImmutable,
        Self::ComplementGfpPlusSinkScc,
        Self::ComplementGfpPlusDependencyOrder,
    ];

    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "full" => Ok(Self::Full),
            "forward-baseline" => Ok(Self::ForwardBaseline),
            "complement-gfp" => Ok(Self::ComplementGfp),
            "complement-gfp-plus-immutable" => Ok(Self::ComplementGfpPlusImmutable),
            "complement-gfp-plus-sink-scc" => Ok(Self::ComplementGfpPlusSinkScc),
            "complement-gfp-plus-dependency-order" => Ok(Self::ComplementGfpPlusDependencyOrder),
            _ => Err(format!(
                "unknown CyBER configuration '{name}'; expected {}",
                Self::ALL.map(Self::slug).join(",")
            )),
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::ForwardBaseline => "forward-baseline",
            Self::ComplementGfp => "complement-gfp",
            Self::ComplementGfpPlusImmutable => "complement-gfp-plus-immutable",
            Self::ComplementGfpPlusSinkScc => "complement-gfp-plus-sink-scc",
            Self::ComplementGfpPlusDependencyOrder => "complement-gfp-plus-dependency-order",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Full => "CyBER Full",
            Self::ForwardBaseline => "Forward Baseline",
            Self::ComplementGfp => "Complement GFP",
            Self::ComplementGfpPlusImmutable => "Complement GFP+Immutable",
            Self::ComplementGfpPlusSinkScc => "Complement GFP+Sink-SCC",
            Self::ComplementGfpPlusDependencyOrder => "Complement GFP+Dependency Order",
        }
    }

    fn immutable(self) -> bool {
        matches!(self, Self::Full | Self::ComplementGfpPlusImmutable)
    }

    fn sink_scc(self) -> bool {
        matches!(self, Self::Full | Self::ComplementGfpPlusSinkScc)
    }

    fn dependency_order(self) -> bool {
        matches!(self, Self::Full | Self::ComplementGfpPlusDependencyOrder)
    }

    fn complement_gfp(self) -> bool {
        !matches!(self, Self::ForwardBaseline)
    }

    fn enabled_techniques(self) -> &'static str {
        match self {
            Self::Full => "complement-gfp,immutable,sink-scc,dependency-order",
            Self::ForwardBaseline => "none",
            Self::ComplementGfp => "complement-gfp",
            Self::ComplementGfpPlusImmutable => "complement-gfp,immutable",
            Self::ComplementGfpPlusSinkScc => "complement-gfp,sink-scc",
            Self::ComplementGfpPlusDependencyOrder => "complement-gfp,dependency-order",
        }
    }
}

pub fn find_cyber_with_config(
    network: &Network,
    logger: &Logger,
    attractor: &HashSet<State>,
    requested_order: BddVariableOrder,
    config: CyberConfiguration,
) -> Result<CyberResult, String> {
    // 手順1: ルール集合BNとアトラクタAを読み、状態の次元を検査する。
    let full_names = lexical_variable_order(network);
    validate_states(attractor, full_names.len(), "attractor")?;
    if attractor.is_empty() {
        return Err("Cannot compute a basin for an empty attractor.".to_string());
    }

    // 手順2: Aが固定点なら、更新規則なし・恒等更新だけの変数を検出する。
    // 手順3: それらをAの値へ固定して規則へ代入し、新しい恒等変数が
    // 生じなくなるまで定数伝播を繰り返す。
    let fixed_point_target = attractor
        .iter()
        .next()
        .is_some_and(|state| attractor.len() == 1 && is_fixed_point(network, state, &full_names));
    let (fixed_inputs, reduced_network) = if fixed_point_target && config.immutable() {
        propagate_immutable_variables(
            network,
            &full_names,
            attractor.iter().next().expect("verified singleton"),
        )?
    } else {
        // 周期アトラクタには固定点専用の構造簡約を適用しない。
        (HashMap::new(), network.clone())
    };

    // 手順4: 簡約した規則の構文から依存グラフを作る。
    let dependencies = if fixed_point_target {
        syntactic_dependencies(&reduced_network, &full_names)
    } else {
        HashMap::new()
    };

    // 手順5: 全入力評価で収束を証明できたsink SCCを除去する。
    let mut retained = full_names.iter().cloned().collect::<HashSet<_>>();
    let sink_scc_variables_before = full_names.len().saturating_sub(fixed_inputs.len());
    let mut sink_scc_counts = None;
    if fixed_point_target {
        let fixed_point = attractor.iter().next().expect("verified singleton");
        if config.sink_scc() {
            retained = prune_convergent_sink_components(
                &reduced_network,
                fixed_point,
                &full_names,
                &dependencies,
                retained,
            )?;
            let after = retained
                .iter()
                .filter(|name| !fixed_inputs.contains_key(*name))
                .count();
            sink_scc_counts = Some((sink_scc_variables_before, after));
        }
    }
    retained.retain(|name| !fixed_inputs.contains_key(name));

    let pruned_variables = full_names.len().saturating_sub(retained.len());
    // 恒等変数はAと同じ初期値でなければ到達不能なので自由度を持たない。
    // それ以外の収束保証済み変数だけが2^nの係数を与える。
    let free_pruned_variables = pruned_variables.saturating_sub(fixed_inputs.len());
    let full_index = full_names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), index))
        .collect::<HashMap<_, _>>();

    let order = if config.dependency_order() {
        requested_order
    } else {
        BddVariableOrder::Lexical
    };

    log!(logger, "CyBER configuration: {}", config.label());
    log!(
        logger,
        "CyBER enabled techniques: {}",
        config.enabled_techniques()
    );
    log!(logger, "BDD retained variables: {}", retained.len());
    log!(
        logger,
        "BDD pruned convergent-output variables: {}",
        pruned_variables
    );
    if retained.is_empty() {
        return Ok(CyberResult {
            basin_size: (BigUint::from(1u8) << free_pruned_variables).to_string(),
            iterations: 0,
            basin_nodes: 1,
            peak_nodes: 1,
            active_rules: 0,
            skipped_rules: network.rules.len(),
            variable_order: order,
            retained_variables: 0,
            pruned_variables,
            sink_scc_variables_before: sink_scc_counts.map(|counts| counts.0),
            sink_scc_variables_after: sink_scc_counts.map(|counts| counts.1),
        });
    }

    // 除去されなかった変数の規則を一つのBDD問題として解く。
    // sink-SCCは外部を調整しないため、その変数は残存規則に現れない。
    let residual_network = Network {
        rules: reduced_network
            .rules
            .iter()
            .filter(|rule| retained.contains(&rule.target))
            .cloned()
            .collect(),
    };
    let residual_names = lexical_variable_order(&residual_network);
    let residual_attractor = attractor
        .iter()
        .map(|state| {
            residual_names
                .iter()
                .map(|name| state[full_index[name]])
                .collect::<State>()
        })
        .collect::<HashSet<_>>();
    let inner = find_cyber_core(
        &residual_network,
        logger,
        &residual_attractor,
        order,
        config,
    )?;
    let residual_size = inner
        .basin_size
        .parse::<BigUint>()
        .map_err(|_| "CyBER returned an invalid cardinality".to_string())?;

    // sink-SCCで除去した変数は任意の初期値を取れるため2^nを掛け戻す。
    Ok(CyberResult {
        basin_size: (residual_size << free_pruned_variables).to_string(),
        iterations: inner.iterations,
        basin_nodes: inner.basin_nodes,
        peak_nodes: inner.peak_nodes,
        active_rules: inner.active_rules,
        skipped_rules: network.rules.len().saturating_sub(inner.active_rules),
        variable_order: order,
        retained_variables: retained.len(),
        pruned_variables,
        sink_scc_variables_before: sink_scc_counts.map(|counts| counts.0),
        sink_scc_variables_after: sink_scc_counts.map(|counts| counts.1),
    })
}

struct CyberCoreResult {
    pub basin_size: String,
    pub iterations: usize,
    pub basin_nodes: usize,
    pub peak_nodes: usize,
    pub active_rules: usize,
}

struct TransitionKernel {
    target: BddVariable,
    enabled: Bdd,
}

fn find_cyber_core(
    network: &Network,
    logger: &Logger,
    attractor: &HashSet<State>,
    order: BddVariableOrder,
    config: CyberConfiguration,
) -> Result<CyberCoreResult, String> {
    // BDD変数と更新規則を構築する。
    let build_start = Instant::now();
    let state_names = lexical_variable_order(network);
    validate_states(attractor, state_names.len(), "attractor")?;
    if attractor.is_empty() {
        return Err("Cannot compute a basin for an empty attractor.".to_string());
    }

    let bdd_order = variable_order(network, order);
    let mut builder = BddVariableSetBuilder::new();
    for name in &bdd_order {
        builder.make_variable(name);
    }
    let variables = builder.build();
    let state_variables = state_names
        .iter()
        .map(|name| {
            variables
                .var_by_name(name)
                .ok_or_else(|| format!("Missing BDD variable '{name}'."))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let kernels = network
        .rules
        .iter()
        .map(|rule| {
            let target = variables
                .var_by_name(&rule.target)
                .ok_or_else(|| format!("Unknown target variable '{}'.", rule.target))?;
            let enabled = variables
                .mk_var(target)
                .xor(&expression_to_bdd(&rule.expr, &variables)?);
            Ok(TransitionKernel { target, enabled })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let attractor_bdd = states_to_bdd(attractor, &variables, &state_variables);

    log!(logger, "BDD backend: Biodivine BDD 0.5.27");
    log!(
        logger,
        "CyBER fixed point: {}",
        if config.complement_gfp() {
            "Complement GFP"
        } else {
            "Forward Baseline"
        }
    );
    log!(logger, "BDD variables: {}", state_names.len());
    log!(logger, "BDD transition representation: rule-local kernels");
    log!(logger, "BDD active rule functions: {}", kernels.len());
    log!(logger, "BDD variable order: {}", order);
    log!(logger, "BDD image accumulation: binary then union");
    log!(logger, "BDD safety/deadlock check: global");
    log!(
        logger,
        "BDD build time: {:.3}",
        build_start.elapsed().as_secs_f64()
    );

    let fixed_point = if config.complement_gfp() {
        complement_fixed_point(&variables, &kernels, attractor_bdd, logger)
    } else {
        forward_fixed_point(&variables, &kernels, attractor_bdd, logger)
    };
    Ok(CyberCoreResult {
        basin_size: fixed_point.basin.exact_cardinality().to_string(),
        iterations: fixed_point.iterations,
        basin_nodes: fixed_point.basin.size(),
        peak_nodes: fixed_point.peak_nodes,
        active_rules: kernels.len(),
    })
}

struct FixedPointResult {
    basin: Bdd,
    iterations: usize,
    peak_nodes: usize,
}

fn transition_predecessor(
    variables: &BddVariableSet,
    kernels: &[TransitionKernel],
    target_set: &Bdd,
) -> Bdd {
    let mut predecessor = variables.mk_false();
    for kernel in kernels {
        let term = Bdd::fused_binary_flip_op(
            (&kernel.enabled, None),
            (target_set, Some(kernel.target)),
            None,
            op_function::and,
        );
        predecessor = predecessor.or(&term);
    }
    predecessor
}

fn complement_fixed_point(
    variables: &BddVariableSet,
    kernels: &[TransitionKernel],
    attractor: Bdd,
    logger: &Logger,
) -> FixedPointResult {
    // 手順8: Aの補集合を、Aを回避し続けられる候補集合badの初期値にする。
    let outside = attractor.not();
    let mut bad = outside.clone();
    let mut peak_nodes = outside.size() + bad.size();
    let mut iterations = 0;
    let global_deadlock = kernels.iter().fold(variables.mk_true(), |set, kernel| {
        set.and_not(&kernel.enabled)
    });

    // 手順9: bad = (not A) ∩ EX(bad) の最大不動点を反復計算する。
    loop {
        iterations += 1;
        let iteration_start = Instant::now();
        let predecessor = transition_predecessor(variables, kernels, &bad);
        peak_nodes = peak_nodes.max(outside.size() + bad.size() + predecessor.size());

        let next_bad = outside.and(&predecessor.or(&global_deadlock));
        let stable = next_bad == bad;
        bad = next_bad;
        peak_nodes = peak_nodes.max(outside.size() + bad.size());
        log!(
            logger,
            "BDD iteration {}: bad nodes={}, time={:.3}",
            iterations,
            bad.size(),
            iteration_start.elapsed().as_secs_f64()
        );
        if stable {
            break;
        }
    }

    // 手順10: AF(A) = not EG(not A) より、最大不動点badを反転する。
    let basin = bad.not();
    peak_nodes = peak_nodes.max(basin.size() + bad.size());
    FixedPointResult {
        basin,
        iterations,
        peak_nodes,
    }
}

fn forward_fixed_point(
    variables: &BddVariableSet,
    kernels: &[TransitionKernel],
    attractor: Bdd,
    logger: &Logger,
) -> FixedPointResult {
    let mut basin = attractor.clone();
    let mut frontier = attractor;
    let mut iterations = 0;
    let mut peak_nodes = basin.size() + frontier.size();

    loop {
        iterations += 1;
        let iteration_start = Instant::now();
        let reaches = transition_predecessor(variables, kernels, &frontier);
        let mut candidate = reaches.and_not(&basin);

        // basin6と同じ基準経路: 全規則のescape集合を作ってから除外する。
        let mut escapes = variables.mk_false();
        for kernel in kernels {
            let term = Bdd::fused_binary_flip_op(
                (&kernel.enabled, None),
                (&basin, Some(kernel.target)),
                None,
                op_function::and_not,
            );
            escapes = escapes.or(&term);
        }
        candidate = candidate.and_not(&escapes);

        let empty = candidate.is_false();
        if !empty {
            basin = basin.or(&candidate);
        }
        frontier = candidate;
        peak_nodes = peak_nodes.max(basin.size() + frontier.size());
        log!(
            logger,
            "BDD iteration {}: frontier nodes={}, basin nodes={}, time={:.3}",
            iterations,
            frontier.size(),
            basin.size(),
            iteration_start.elapsed().as_secs_f64()
        );
        if empty {
            break;
        }
    }

    FixedPointResult {
        basin,
        iterations,
        peak_nodes,
    }
}

#[cfg(all(test, feature = "internal-regression-tests"))]
mod tests {
    use super::*;
    use crate::bn::{BooleanExpr, Network, Rule, parse_network};
    use crate::solver::{find_basin6, find_basin10};

    fn paper_configurations() -> Vec<(String, CyberConfiguration)> {
        CyberConfiguration::ALL
            .into_iter()
            .map(|config| (config.slug().to_string(), config))
            .collect()
    }

    fn truth_table_expression(table: usize) -> BooleanExpr {
        let names = ["A", "B"];
        let mut terms = Vec::new();
        for valuation in 0..4 {
            if table & (1usize << valuation) == 0 {
                continue;
            }
            let mut literals = names.iter().enumerate().map(|(bit, name)| {
                let variable = BooleanExpr::Var((*name).to_string());
                if valuation & (1usize << bit) == 0 {
                    BooleanExpr::Not(Box::new(variable))
                } else {
                    variable
                }
            });
            let term = BooleanExpr::And(
                Box::new(literals.next().expect("two variables")),
                Box::new(literals.next().expect("two variables")),
            );
            terms.push(term);
        }
        let witness = BooleanExpr::Var("A".to_string());
        terms
            .into_iter()
            .reduce(|left, right| BooleanExpr::Or(Box::new(left), Box::new(right)))
            .unwrap_or_else(|| {
                BooleanExpr::And(
                    Box::new(witness.clone()),
                    Box::new(BooleanExpr::Not(Box::new(witness))),
                )
            })
    }

    #[test]
    fn matches_basin10_on_small_fixed_points_exhaustively() {
        let logger = Logger::new();
        for table_a in 0..16 {
            for table_b in 0..16 {
                let network = Network {
                    rules: vec![
                        Rule {
                            target: "A".to_string(),
                            expr: truth_table_expression(table_a),
                        },
                        Rule {
                            target: "B".to_string(),
                            expr: truth_table_expression(table_b),
                        },
                    ],
                };
                for valuation in 0..4 {
                    let state = vec![valuation & 1 != 0, valuation & 2 != 0];
                    if (table_a & (1usize << valuation) != 0) != state[0]
                        || (table_b & (1usize << valuation) != 0) != state[1]
                    {
                        continue;
                    }
                    let attractor = [state].into_iter().collect();
                    let expected =
                        find_basin10(&network, &logger, &attractor, BddVariableOrder::Dependency)
                            .unwrap();
                    for (label, config) in paper_configurations() {
                        let actual = find_cyber_with_config(
                            &network,
                            &logger,
                            &attractor,
                            BddVariableOrder::Dependency,
                            config,
                        )
                        .unwrap();
                        assert_eq!(
                            expected.basin_size, actual.basin_size,
                            "configuration={label}, tables=({table_a}, {table_b}), valuation={valuation}"
                        );
                    }

                    let basin6 = find_basin6(&network, &logger, &attractor, &attractor).unwrap();
                    let forward_baseline = find_cyber_with_config(
                        &network,
                        &logger,
                        &attractor,
                        BddVariableOrder::Dependency,
                        CyberConfiguration::ForwardBaseline,
                    )
                    .unwrap();
                    assert_eq!(basin6.basin_size, forward_baseline.basin_size);
                }
            }
        }
    }

    #[test]
    fn parses_paper_configuration_names() {
        for config in CyberConfiguration::ALL {
            assert_eq!(CyberConfiguration::parse(config.slug()), Ok(config));
        }
        assert!(CyberConfiguration::parse("not-a-configuration").is_err());
    }

    #[test]
    fn supports_an_explicit_cyclic_attractor() {
        let network = parse_network("targets,factors\nA, !A\nB, B\n").unwrap();
        let attractor = [vec![false, false], vec![true, false]]
            .into_iter()
            .collect();
        let logger = Logger::new();
        let expected =
            find_basin10(&network, &logger, &attractor, BddVariableOrder::Dependency).unwrap();
        let actual = find_cyber_with_config(
            &network,
            &logger,
            &attractor,
            BddVariableOrder::Dependency,
            CyberConfiguration::Full,
        )
        .unwrap();
        assert_eq!(expected.basin_size, actual.basin_size);
    }

    #[test]
    fn self_contained_preprocessing_prunes_an_acyclic_output_cone() {
        let network = parse_network("targets,factors\nA, A\nB, A\nC, B\n").unwrap();
        let attractor = [vec![false, false, false]].into_iter().collect();
        let logger = Logger::new();
        let expected =
            find_basin10(&network, &logger, &attractor, BddVariableOrder::Dependency).unwrap();
        let actual = find_cyber_with_config(
            &network,
            &logger,
            &attractor,
            BddVariableOrder::Dependency,
            CyberConfiguration::Full,
        )
        .unwrap();
        assert_eq!(expected.basin_size, actual.basin_size);
        assert_eq!(actual.retained_variables, 0);
        assert_eq!(actual.pruned_variables, 3);
    }
}
