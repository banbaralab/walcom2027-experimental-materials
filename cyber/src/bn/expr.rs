#[derive(Debug, Clone, PartialEq)]
pub enum BooleanExpr {
    Var(String),
    Not(Box<BooleanExpr>),
    And(Box<BooleanExpr>, Box<BooleanExpr>),
    Or(Box<BooleanExpr>, Box<BooleanExpr>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_var_creation() {
        let expr = BooleanExpr::Var("geneA".to_string());
        assert_eq!(expr, BooleanExpr::Var("geneA".to_string()));
    }

    #[test]
    fn test_not_creation() {
        let expr = BooleanExpr::Not(Box::new(BooleanExpr::Var("geneA".to_string())));
        match expr {
            BooleanExpr::Not(inner) => {
                assert_eq!(*inner, BooleanExpr::Var("geneA".to_string()));
            }
            _ => panic!("Expected Not"),
        }
    }

    #[test]
    fn test_and_creation() {
        let expr = BooleanExpr::And(
            Box::new(BooleanExpr::Var("geneA".to_string())),
            Box::new(BooleanExpr::Var("geneB".to_string())),
        );
        match expr {
            BooleanExpr::And(left, right) => {
                assert_eq!(*left, BooleanExpr::Var("geneA".to_string()));
                assert_eq!(*right, BooleanExpr::Var("geneB".to_string()));
            }
            _ => panic!("Expected And"),
        }
    }
}
