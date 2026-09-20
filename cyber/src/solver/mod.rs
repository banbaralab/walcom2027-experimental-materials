mod basin_reduction;
mod bdd_order;
mod bdd_utils;
mod cyber;

pub type State = Vec<bool>;

pub use bdd_order::BddVariableOrder;
pub use cyber::{find_cyber_with_config, CyberConfiguration};
