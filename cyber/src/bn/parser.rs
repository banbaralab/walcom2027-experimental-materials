use super::expr::BooleanExpr;

#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub target: String,
    pub expr: BooleanExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Network {
    pub rules: Vec<Rule>,
}

pub fn parse_network(input: &str) -> Result<Network, String> {
    let mut lines = input.lines().peekable();

    // Skip empty lines and comments before header
    while let Some(&line) = lines.peek() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            lines.next();
        } else {
            break;
        }
    }

    // Parse header
    let header = lines.next().ok_or("Empty input")?;
    let header = header.trim();
    if header != "targets,factors" && header != "targets, factors" {
        return Err(format!("Invalid header: expected 'targets,factors', got '{}'", header));
    }

    let mut rules = Vec::new();

    for line in lines {
        let line = line.trim();

        // Skip empty lines
        if line.is_empty() {
            continue;
        }

        // Skip comments
        if line.starts_with('#') {
            continue;
        }

        // Parse rule: target,expression
        let comma_pos = line.find(',').ok_or(format!("Invalid rule format: {}", line))?;
        let target = line[..comma_pos].trim().to_string();
        let expr_str = line[comma_pos + 1..].trim();
        let expr = parse_expr(expr_str)?;

        rules.push(Rule { target, expr });
    }

    Ok(Network { rules })
}

pub fn parse_expr(input: &str) -> Result<BooleanExpr, String> {
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Err("Empty expression".to_string());
    }
    parse_expr_iterative(&tokens)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Gene(String),
    Not,
    And,
    Or,
    LParen,
    RParen,
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' => {
                chars.next();
            }
            '!' => {
                chars.next();
                tokens.push(Token::Not);
            }
            '&' => {
                chars.next();
                tokens.push(Token::And);
            }
            '|' => {
                chars.next();
                tokens.push(Token::Or);
            }
            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }
            _ if c.is_alphanumeric() || c == '_' => {
                let mut name = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        name.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Gene(name));
            }
            _ => {
                return Err(format!("Unexpected character: {}", c));
            }
        }
    }

    Ok(tokens)
}

// Iterative expression parser using shunting-yard algorithm
fn parse_expr_iterative(tokens: &[Token]) -> Result<BooleanExpr, String> {
    let mut output: Vec<BooleanExpr> = Vec::new();
    let mut operators: Vec<Op> = Vec::new();

    #[derive(Debug, Clone, PartialEq)]
    enum Op {
        Not,
        And,
        Or,
        LParen,
    }

    fn precedence(op: &Op) -> i32 {
        match op {
            Op::Or => 1,
            Op::And => 2,
            Op::Not => 3,
            Op::LParen => 0,
        }
    }

    fn apply_op(op: Op, output: &mut Vec<BooleanExpr>) -> Result<(), String> {
        match op {
            Op::Not => {
                let expr = output.pop().ok_or("Missing operand for NOT")?;
                output.push(BooleanExpr::Not(Box::new(expr)));
            }
            Op::And => {
                let right = output.pop().ok_or("Missing right operand for AND")?;
                let left = output.pop().ok_or("Missing left operand for AND")?;
                output.push(BooleanExpr::And(Box::new(left), Box::new(right)));
            }
            Op::Or => {
                let right = output.pop().ok_or("Missing right operand for OR")?;
                let left = output.pop().ok_or("Missing left operand for OR")?;
                output.push(BooleanExpr::Or(Box::new(left), Box::new(right)));
            }
            Op::LParen => {
                return Err("Mismatched parentheses".to_string());
            }
        }
        Ok(())
    }

    let mut i = 0;
    let mut expect_operand = true;

    while i < tokens.len() {
        match &tokens[i] {
            Token::Gene(name) => {
                if !expect_operand {
                    return Err("Unexpected operand".to_string());
                }
                output.push(BooleanExpr::Var(name.clone()));
                expect_operand = false;
            }
            Token::Not => {
                if !expect_operand {
                    return Err("Unexpected NOT operator".to_string());
                }
                operators.push(Op::Not);
                // expect_operand stays true
            }
            Token::And => {
                if expect_operand {
                    return Err("Unexpected AND operator".to_string());
                }
                while let Some(top) = operators.last() {
                    if *top == Op::LParen || precedence(top) < precedence(&Op::And) {
                        break;
                    }
                    let op = operators.pop().unwrap();
                    apply_op(op, &mut output)?;
                }
                operators.push(Op::And);
                expect_operand = true;
            }
            Token::Or => {
                if expect_operand {
                    return Err("Unexpected OR operator".to_string());
                }
                while let Some(top) = operators.last() {
                    if *top == Op::LParen || precedence(top) < precedence(&Op::Or) {
                        break;
                    }
                    let op = operators.pop().unwrap();
                    apply_op(op, &mut output)?;
                }
                operators.push(Op::Or);
                expect_operand = true;
            }
            Token::LParen => {
                if !expect_operand {
                    return Err("Unexpected opening parenthesis".to_string());
                }
                operators.push(Op::LParen);
                // expect_operand stays true
            }
            Token::RParen => {
                if expect_operand {
                    return Err("Unexpected closing parenthesis".to_string());
                }
                while let Some(top) = operators.last() {
                    if *top == Op::LParen {
                        break;
                    }
                    let op = operators.pop().unwrap();
                    apply_op(op, &mut output)?;
                }
                if operators.last() == Some(&Op::LParen) {
                    operators.pop();
                } else {
                    return Err("Mismatched parentheses".to_string());
                }
                // After closing paren, apply any pending NOT operators
                while let Some(&Op::Not) = operators.last() {
                    let op = operators.pop().unwrap();
                    apply_op(op, &mut output)?;
                }
                // expect_operand stays false
            }
        }
        i += 1;
    }

    // Apply remaining operators
    while let Some(op) = operators.pop() {
        if op == Op::LParen {
            return Err("Mismatched parentheses".to_string());
        }
        apply_op(op, &mut output)?;
    }

    if output.len() != 1 {
        return Err(format!("Invalid expression: {} items on stack", output.len()));
    }

    Ok(output.pop().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // BooleanExpression パーサーのテスト
    // ========================================

    #[test]
    fn test_parse_single_gene() {
        let result = parse_expr("geneA").unwrap();
        assert_eq!(result, BooleanExpr::Var("geneA".to_string()));
    }

    #[test]
    fn test_parse_not() {
        let result = parse_expr("!geneA").unwrap();
        assert_eq!(
            result,
            BooleanExpr::Not(Box::new(BooleanExpr::Var("geneA".to_string())))
        );
    }

    #[test]
    fn test_parse_and() {
        let result = parse_expr("geneA & geneB").unwrap();
        assert_eq!(
            result,
            BooleanExpr::And(
                Box::new(BooleanExpr::Var("geneA".to_string())),
                Box::new(BooleanExpr::Var("geneB".to_string())),
            )
        );
    }

    #[test]
    fn test_parse_or() {
        let result = parse_expr("geneA | geneB").unwrap();
        assert_eq!(
            result,
            BooleanExpr::Or(
                Box::new(BooleanExpr::Var("geneA".to_string())),
                Box::new(BooleanExpr::Var("geneB".to_string())),
            )
        );
    }

    #[test]
    fn test_parse_parentheses() {
        let result = parse_expr("(geneA)").unwrap();
        assert_eq!(result, BooleanExpr::Var("geneA".to_string()));
    }

    #[test]
    fn test_parse_complex_expr() {
        // !geneA & geneB
        let result = parse_expr("!geneA & geneB").unwrap();
        assert_eq!(
            result,
            BooleanExpr::And(
                Box::new(BooleanExpr::Not(Box::new(BooleanExpr::Var("geneA".to_string())))),
                Box::new(BooleanExpr::Var("geneB".to_string())),
            )
        );
    }

    #[test]
    fn test_parse_nested_parentheses() {
        // (geneA | geneB) & geneC
        let result = parse_expr("(geneA | geneB) & geneC").unwrap();
        assert_eq!(
            result,
            BooleanExpr::And(
                Box::new(BooleanExpr::Or(
                    Box::new(BooleanExpr::Var("geneA".to_string())),
                    Box::new(BooleanExpr::Var("geneB".to_string())),
                )),
                Box::new(BooleanExpr::Var("geneC".to_string())),
            )
        );
    }

    #[test]
    fn test_parse_not_with_parentheses() {
        // !(geneA | geneB)
        let result = parse_expr("!(geneA | geneB)").unwrap();
        assert_eq!(
            result,
            BooleanExpr::Not(Box::new(BooleanExpr::Or(
                Box::new(BooleanExpr::Var("geneA".to_string())),
                Box::new(BooleanExpr::Var("geneB".to_string())),
            )))
        );
    }

    #[test]
    fn test_parse_precedence() {
        // geneA | geneB & geneC should be geneA | (geneB & geneC)
        let result = parse_expr("geneA | geneB & geneC").unwrap();
        assert_eq!(
            result,
            BooleanExpr::Or(
                Box::new(BooleanExpr::Var("geneA".to_string())),
                Box::new(BooleanExpr::And(
                    Box::new(BooleanExpr::Var("geneB".to_string())),
                    Box::new(BooleanExpr::Var("geneC".to_string())),
                )),
            )
        );
    }

    // ========================================
    // Network パーサーのテスト
    // ========================================

    #[test]
    fn test_parse_simple_network() {
        let input = "targets,factors\ngeneA,geneB";
        let result = parse_network(input).unwrap();
        assert_eq!(result.rules.len(), 1);
        assert_eq!(result.rules[0].target, "geneA");
        assert_eq!(result.rules[0].expr, BooleanExpr::Var("geneB".to_string()));
    }

    #[test]
    fn test_parse_network_with_comment() {
        let input = "targets,factors\n# This is a comment\ngeneA,geneB";
        let result = parse_network(input).unwrap();
        assert_eq!(result.rules.len(), 1);
    }

    #[test]
    fn test_parse_network_multiple_rules() {
        let input = "targets,factors\ngeneA,geneB\ngeneC,geneA & geneB";
        let result = parse_network(input).unwrap();
        assert_eq!(result.rules.len(), 2);
        assert_eq!(result.rules[0].target, "geneA");
        assert_eq!(result.rules[1].target, "geneC");
    }

    #[test]
    fn test_parse_bbm_sample() {
        // BBM benchmark sample (model_007.bnet)
        let input = r#"targets,factors
v_Coup_fti, (!(v_Fgf8 | v_Sp8) | !(v_Sp8 | v_Fgf8))
v_Emx2, (v_Coup_fti & !((v_Fgf8 | v_Sp8) | v_Pax6))
v_Fgf8, ((v_Fgf8 & v_Sp8) & !v_Emx2)
v_Pax6, (v_Sp8 & !(v_Emx2 | v_Coup_fti))
v_Sp8, (v_Fgf8 & !v_Emx2)"#;
        let result = parse_network(input).unwrap();
        assert_eq!(result.rules.len(), 5);
    }

    #[test]
    fn test_parse_benchmark_files() {
        use std::fs;
        use std::path::Path;

        let benchmark_dir = Path::new("benchmark/BBM");
        if !benchmark_dir.exists() {
            println!("Benchmark directory not found, skipping test");
            return;
        }

        let mut success = 0;
        let mut failed = 0;
        let mut errors: Vec<(String, String)> = Vec::new();

        let entries: Vec<_> = fs::read_dir(benchmark_dir)
            .expect("Failed to read benchmark directory")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "bnet"))
            .collect();

        for entry in &entries {
            let path = entry.path();
            let filename = path.file_name().unwrap().to_string_lossy().to_string();

            match fs::read_to_string(&path) {
                Ok(content) => {
                    match parse_network(&content) {
                        Ok(_) => {
                            success += 1;
                        }
                        Err(e) => {
                            failed += 1;
                            errors.push((filename.clone(), e.clone()));
                        }
                    }
                }
                Err(e) => {
                    failed += 1;
                    errors.push((filename.clone(), e.to_string()));
                }
            }
        }

        println!("\n=== BBM Benchmark Parse Test ===");
        println!("Total: {} files", entries.len());
        println!("Success: {}", success);
        println!("Failed: {}", failed);

        if !errors.is_empty() {
            println!("\nFirst 10 errors:");
            for (filename, error) in &errors[..errors.len().min(10)] {
                println!("  {}: {}", filename, error);
            }
        }

        assert_eq!(failed, 0, "Some files failed to parse: {:?}", errors);
    }
}
