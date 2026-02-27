// pedestrian low
/*
\begin{equation}
\mu_{\text{少}}(x) =
    \begin{cases}
        0, & x \le 0, \\
        1, & 0 < x \le 30, \\
        \dfrac{50 - x}{50 - 30}, & 30 < x < 50, \\
        0, & x \ge 50.
    \end{cases}
\end{equation}
*/
fn ped_low(x: f64) -> f64 {
    if x <= 0.0 {
        0.0
    } else if x > 0.0 && x <= 30.0 {
        1.0
    } else if x > 30.0 && x < 50.0 {
        (50.0 - x) / (50.0 - 30.0)
    } else {
        0.0
    }
}

// pedestrian medium
/* \begin{equation}
\mu_{\text{中}}(x) =
    \begin{cases}
        0, & x \le 30, \\
        \dfrac{x - 30}{50 - 30}, & 30 < x \le 50, \\
        1, & 50 < x \le 100, \\
        \dfrac{150 - x}{150 - 100}, & 100 < x < 150, \\
        0, & x \ge 150.
    \end{cases}
\end{equation}
*/
fn ped_mid(x: f64) -> f64 {
    if x <= 30.0 {
        0.0
    } else if x > 30.0 && x <= 50.0 {
        (x - 30.0) / (50.0 - 30.0)
    } else if x > 50.0 && x <= 100.0 {
        1.0
    } else if x > 100.0 && x < 150.0 {
        (150.0 - x) / (150.0 - 100.0)
    } else {
        0.0
    }
}

// pedestrian high
/*
\begin{equation}
\mu_{\text{多}}(x) =
    \begin{cases}
        0, & x \le 100, \\
        \dfrac{x - 100}{150 - 100}, & 100 < x \le 150, \\
        1, & 150 < x \le 300, \\
    \end{cases}
\end{equation}
*/
fn ped_high(x: f64) -> f64 {
    if x <= 100.0 {
        0.0
    } else if x > 100.0 && x <= 150.0 {
        (x - 100.0) / (150.0 - 100.0)
    } else {
        1.0
    }
}

// vehicle low
/*
\begin{equation}
\mu_{\text{少}}(y) =
    \begin{cases}
        0, & y \le 0, \\
        1, & 0 < y \le 3, \\
        \dfrac{5 - y}{5 - 3}, & 3 < y < 5, \\
        0, & y \ge 5.
    \end{cases}
\end{equation}
*/
fn veh_low(y: f64) -> f64 {
    if y <= 0.0 {
        0.0
    } else if y > 0.0 && y <= 3.0 {
        1.0
    } else if y > 3.0 && y < 5.0 {
        (5.0 - y) / (5.0 - 3.0)
    } else {
        0.0
    }
}

// vehicle medium
/*
\begin{equation}
\mu_{\text{中}}(y) =
    \begin{cases}
        0, & y \le 3, \\
        \dfrac{y - 3}{5 - 3}, & 3 < y \le 5, \\
        1, & 5 < y \le 7, \\
        \dfrac{10 - y}{10 - 7}, & 7 < y < 10, \\
        0, & y \ge 10.
    \end{cases}
\end{equation}
*/
fn veh_mid(y: f64) -> f64 {
    if y <= 3.0 {
        0.0
    } else if y > 3.0 && y <= 5.0 {
        (y - 3.0) / (5.0 - 3.0)
    } else if y > 5.0 && y <= 7.0 {
        1.0
    } else if y > 7.0 && y < 10.0 {
        (10.0 - y) / (10.0 - 7.0)
    } else {
        0.0
    }
}

// vehicle high
/*
\begin{equation}
\mu_{\text{多}}(y) =
    \begin{cases}
        0, & y \le 7, \\
        \dfrac{y - 7}{10 - 7}, & 7 < y \le 10, \\
        1, & 10 < y \le 20, \\
        1, & y > 20.
    \end{cases}
\end{equation}
*/
fn veh_high(y: f64) -> f64 {
    if y <= 7.0 {
        0.0
    } else if y > 7.0 && y <= 10.0 {
        (y - 7.0) / (10.0 - 7.0)
    } else {
        1.0
    }
}

enum FuzzyResult {
    Low,
    Medium,
    High,
}
enum FuzzyAction{
    NoExtension,
    ShortExtension,
    MediumExtension,
    LongExtension,
}
fn knowledge_base(ped: FuzzyResult, veh: FuzzyResult) -> FuzzyAction {
    match (veh, ped) {
        (FuzzyResult::Low,    FuzzyResult::Low)    => FuzzyAction::NoExtension,
        (FuzzyResult::Low,    FuzzyResult::Medium) => FuzzyAction::MediumExtension,
        (FuzzyResult::Low,    FuzzyResult::High)   => FuzzyAction::LongExtension,
        (FuzzyResult::Medium, FuzzyResult::Low)    => FuzzyAction::NoExtension,
        (FuzzyResult::Medium, FuzzyResult::Medium) => FuzzyAction::ShortExtension,
        (FuzzyResult::Medium, FuzzyResult::High)   => FuzzyAction::MediumExtension,
        (FuzzyResult::High,   FuzzyResult::Low)    => FuzzyAction::NoExtension,
        (FuzzyResult::High,   FuzzyResult::Medium) => FuzzyAction::NoExtension,
        (FuzzyResult::High,   FuzzyResult::High)   => FuzzyAction::ShortExtension,
    }
}

