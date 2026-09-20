use std::fmt;
use std::str::FromStr;

/// BDD変数を上から並べる方法。各basin実装が共有する設定値であり、
/// 特定のアルゴリズムやBDDバックエンドには依存しない。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BddVariableOrder {
    Lexical,
    Input,
    Dependency,
    MinFill,
    MinFillForward,
    Feedback,
    FeedbackReverse,
}

impl Default for BddVariableOrder {
    fn default() -> Self {
        Self::Dependency
    }
}

impl fmt::Display for BddVariableOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lexical => write!(f, "lexical"),
            Self::Input => write!(f, "input"),
            Self::Dependency => write!(f, "dependency"),
            Self::MinFill => write!(f, "minfill"),
            Self::MinFillForward => write!(f, "minfill-forward"),
            Self::Feedback => write!(f, "feedback"),
            Self::FeedbackReverse => write!(f, "feedback-reverse"),
        }
    }
}

impl FromStr for BddVariableOrder {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "lexical" => Ok(Self::Lexical),
            "input" => Ok(Self::Input),
            "dependency" => Ok(Self::Dependency),
            "minfill" => Ok(Self::MinFill),
            "minfill-forward" => Ok(Self::MinFillForward),
            "feedback" => Ok(Self::Feedback),
            "feedback-reverse" => Ok(Self::FeedbackReverse),
            _ => Err(format!(
                "unknown BDD variable order '{}'; expected lexical, input, dependency, minfill, minfill-forward, feedback, or feedback-reverse",
                value
            )),
        }
    }
}
