// This file contains fuzzy inference logics and operations
// Fuzzy inference method: Mamdani
// Exact pedestrian or vehicle count -> list of membership degrees
use std::collections::HashMap;
use crate::membership_fns::{FuzzyAction, FuzzyLabels};
use crate::config::{T_MIN, T_MAX, STEP};
fn get_element_membership_degree(   // 3.3.1
    exact_value: i32,
    membership_fn_list: Vec<(FuzzyLabels, fn(i32) -> f64)>
) -> Vec<(FuzzyLabels, f64)> {    // <FuzzyResult, degree>
    let mut elem_mb_degrees = Vec::new();
    for (fz_label, func) in membership_fn_list {
        elem_mb_degrees.push((fz_label, func(exact_value)));
    }
    /*return*/elem_mb_degrees
}
// result would be like: [(FuzzyLabels::Low, 0.8), (FuzzyLabels::Medium, 0.2), (FuzzyLabels::High, 0.0)]
// run this twice to get both ped and veh elem membership degrees

// Action membership degree: minimum of all element membership degrees in the rule
fn get_rule_membership_degree(    // 3.3.2
    knowledge_base: fn(FuzzyLabels, FuzzyLabels) -> FuzzyAction,
    ped_element_membership_degrees: Vec<(FuzzyLabels, f64)>,
    veh_element_membership_degrees: Vec<(FuzzyLabels, f64)>,
) -> HashMap<FuzzyAction, Vec<f64>> {    // <FuzzyAction, Vec(degree)>
    // for all FuzzyAction members, init <FuzzyAction, Vec<f64>> with empty vec
    let mut rule_mb_degrees: HashMap<FuzzyAction, Vec<f64>> = FuzzyAction::all().iter().map(|&a| (a, Vec::new())).collect();
    // A simple two-dimension loop here just like we do in array[][]
    for (ped_label, ped_degree) in &ped_element_membership_degrees {
        for (veh_label, veh_degree) in &veh_element_membership_degrees {
            let action = knowledge_base(*ped_label, *veh_label);
            let degree = ped_degree.min(*veh_degree);
            rule_mb_degrees.entry(action).or_default().push(degree);
        }
    }
    /*return*/rule_mb_degrees
}
// result would be like: {FuzzyAction::NoExtension: [0.8, 0.0], FuzzyAction::MediumExtension: [0.2], ...}

// Find final degree by selecting the maximum degree of each action
fn get_operation_membership_degree(     // 3.3.3
    rule_membership_degrees: HashMap<FuzzyAction, Vec<f64>>,
) -> HashMap<FuzzyAction, f64> {    // <FuzzyAction, final degree>
    // just like we do in 3.3.2, init <FuzzyAction, f64> but with 0.0 as default value
    let mut operation_mb_degrees: HashMap<FuzzyAction, f64> = FuzzyAction::all().iter().map(|&a| (a, 0.0)).collect();
    for (action, degrees) in rule_membership_degrees {
        let max_degree = degrees.into_iter().fold(0.0, f64::max);
        operation_mb_degrees.insert(action, max_degree);
    }
    /*return*/operation_mb_degrees
}
// result would be like: {FuzzyAction::NoExtension: 0.8, FuzzyAction::MediumExtension: 0.2, ...}

// Centroid defuzzification, aka 3.4
// t range: [0.0, 60.0], step: 0.1
fn defuzzify(operation_membership_degrees: HashMap<FuzzyAction, f64>) -> f64 {
    let mut numerator :f64 = 0.0;  // Σ μ(t) * t
    let mut denominator :f64 = 0.0;  // Σ μ(t)

    // T_MIN, T_MAX, STEP are defined in config.rs
    let mut t = T_MIN;
    while t <= T_MAX {
        // For each t, take the max of all clipped membership values (union of all actions)
        let mu_t = operation_membership_degrees
            .iter()
            .map(|(&action, &alpha)| action.ext_fn()(t, alpha))  // cut with alpha
            .fold(0.0_f64, f64::max);                            // union = max

        numerator   += mu_t * t;
        denominator += mu_t;
        t += STEP;
    }

    if denominator == 0.0 { 0.0 } else { numerator / denominator }
}
// result: final green extension time in f64

/// Public entry point: i32 ped count + i32 veh count -> f64 extend time
pub fn run_fuzzy_inference(ped_count: i32, veh_count: i32) -> f64 {
    use crate::membership_fns::{ped_low, ped_mid, ped_high, veh_low, veh_mid, veh_high, knowledge_base};

    let ped_degrees = get_element_membership_degree(
        ped_count,
        vec![
            (FuzzyLabels::Low,    ped_low  as fn(i32) -> f64),
            (FuzzyLabels::Medium, ped_mid  as fn(i32) -> f64),
            (FuzzyLabels::High,   ped_high as fn(i32) -> f64),
        ],
    );
    let veh_degrees = get_element_membership_degree(
        veh_count,
        vec![
            (FuzzyLabels::Low,    veh_low  as fn(i32) -> f64),
            (FuzzyLabels::Medium, veh_mid  as fn(i32) -> f64),
            (FuzzyLabels::High,   veh_high as fn(i32) -> f64),
        ],
    );
    let rule_degrees = get_rule_membership_degree(knowledge_base, ped_degrees, veh_degrees);
    let op_degrees   = get_operation_membership_degree(rule_degrees);
    defuzzify(op_degrees)
}
