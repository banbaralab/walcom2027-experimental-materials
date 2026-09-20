mod bn;
mod solver;

use bn::{parse_network, BooleanExpr, Network};
use solver::{find_cyber_with_config, BddVariableOrder, CyberConfiguration, State};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::time::Instant;

pub struct Logger {
    start: Instant,
}

impl Logger {
    fn new() -> Self {
        Self { start: Instant::now() }
    }

    pub fn log_fmt(&self, args: std::fmt::Arguments) {
        println!("{:8.3} {}", self.start.elapsed().as_secs_f64(), args);
    }
}

#[macro_export]
macro_rules! log {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log_fmt(format_args!($($arg)*))
    };
}

fn usage(program: &str) -> ! {
    eprintln!("Usage: {program} -cyber <attractor.csv> [options] <network.bnet>");
    eprintln!("Options:");
    eprintln!("  --cyber-config <name>  one of:");
    for config in CyberConfiguration::ALL {
        eprintln!("      {}", config.slug());
    }
    eprintln!("  --cyber-order <order>  dependency (default), lexical, input,");
    eprintln!("                          minfill[-forward], or feedback[-reverse]");
    std::process::exit(2);
}

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let mut attractor_path = None;
    let mut network_path = None;
    let mut order = BddVariableOrder::default();
    let mut config = CyberConfiguration::default();
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "-cyber" => {
                index += 1;
                attractor_path = args.get(index).cloned();
            }
            "--cyber-config" => {
                index += 1;
                config = CyberConfiguration::parse(
                    args.get(index).unwrap_or_else(|| usage(&args[0])),
                )
                .unwrap_or_else(|error| fail(&error));
            }
            "--cyber-order" => {
                index += 1;
                order = args
                    .get(index)
                    .unwrap_or_else(|| usage(&args[0]))
                    .parse::<BddVariableOrder>()
                    .unwrap_or_else(|error| fail(&error));
            }
            option if option.starts_with('-') => fail(&format!("unknown option '{option}'")),
            path if network_path.is_none() => network_path = Some(path.to_string()),
            _ => usage(&args[0]),
        }
        index += 1;
    }

    let attractor_path = attractor_path.unwrap_or_else(|| usage(&args[0]));
    let network_path = network_path.unwrap_or_else(|| usage(&args[0]));
    let network_text = fs::read_to_string(&network_path)
        .unwrap_or_else(|error| fail(&format!("cannot read {network_path}: {error}")));
    let network = parse_network(&network_text)
        .unwrap_or_else(|error| fail(&format!("cannot parse {network_path}: {error}")));
    let attractor = load_attractor(&attractor_path, &network);
    let logger = Logger::new();

    let result = find_cyber_with_config(&network, &logger, &attractor, order, config)
        .unwrap_or_else(|error| fail(&format!("basin computation failed: {error}")));

    log!(logger, "");
    log!(logger, "=== Final Summary ===");
    log!(logger, "Total basin states: {}", result.basin_size);
    log!(logger, "BDD iterations: {}", result.iterations);
    log!(logger, "BDD basin nodes: {}", result.basin_nodes);
    log!(logger, "BDD peak tracked nodes: {}", result.peak_nodes);
    log!(logger, "BDD retained variables: {}", result.retained_variables);
    log!(logger, "BDD pruned variables: {}", result.pruned_variables);
    if let (Some(before), Some(after)) = (
        result.sink_scc_variables_before,
        result.sink_scc_variables_after,
    ) {
        log!(logger, "Sink-SCC variables before: {}", before);
        log!(logger, "Sink-SCC variables after: {}", after);
    }
    log!(logger, "BDD active rules: {}", result.active_rules);
    log!(logger, "BDD skipped rules: {}", result.skipped_rules);
    log!(logger, "BDD variable order: {}", result.variable_order);
}

fn load_attractor(path: &str, network: &Network) -> HashSet<State> {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|error| fail(&format!("cannot read {path}: {error}")));
    let names = variable_names(network);
    let positions = names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut states = HashSet::new();
    let mut state = vec![false; names.len()];
    let mut assigned = HashSet::new();

    for line in content.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line == "---" {
            finish_state(path, &names, &mut states, &mut state, &mut assigned);
            continue;
        }
        let (name, value) = line
            .split_once(',')
            .unwrap_or_else(|| fail(&format!("invalid attractor row in {path}: {line}")));
        let name = name.trim();
        let position = *positions
            .get(name)
            .unwrap_or_else(|| fail(&format!("unknown variable '{name}' in {path}")));
        state[position] = match value.trim() {
            "0" => false,
            "1" => true,
            other => fail(&format!("invalid value '{other}' for '{name}' in {path}")),
        };
        if !assigned.insert(position) {
            fail(&format!("duplicate variable '{name}' in one state in {path}"));
        }
    }
    finish_state(path, &names, &mut states, &mut state, &mut assigned);
    if states.is_empty() {
        fail(&format!("no attractor states found in {path}"));
    }
    states
}

fn finish_state(
    path: &str,
    names: &[String],
    states: &mut HashSet<State>,
    state: &mut State,
    assigned: &mut HashSet<usize>,
) {
    if assigned.is_empty() {
        return;
    }
    if assigned.len() != names.len() {
        fail(&format!(
            "incomplete state in {path}: expected {} variables, found {}",
            names.len(), assigned.len()
        ));
    }
    states.insert(state.clone());
    state.fill(false);
    assigned.clear();
}

fn variable_names(network: &Network) -> Vec<String> {
    fn collect(expression: &BooleanExpr, names: &mut HashSet<String>) {
        match expression {
            BooleanExpr::Var(name) => { names.insert(name.clone()); }
            BooleanExpr::Not(inner) => collect(inner, names),
            BooleanExpr::And(left, right) | BooleanExpr::Or(left, right) => {
                collect(left, names);
                collect(right, names);
            }
        }
    }

    let mut names = HashSet::new();
    for rule in &network.rules {
        names.insert(rule.target.clone());
        collect(&rule.expr, &mut names);
    }
    let mut names = names.into_iter().collect::<Vec<_>>();
    names.sort();
    names
}

fn fail(message: &str) -> ! {
    eprintln!("Error: {message}");
    std::process::exit(1);
}
