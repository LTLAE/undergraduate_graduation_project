// This file contains fuzzy inference logics and operations
// Exact pedestrian or vehicle count -> list of membership degrees (by hashmap)
use crate::membership_fns::{FuzzyLabels};
fn fuzzy_inference(
    exact_value: i32,
    membership_fn_list: Vec<(FuzzyLabels, fn(i32) -> f64)>
) -> Vec<(FuzzyLabels, f64)> {    // <FuzzyResult, degree>
    let mut results = Vec::new();
    for (fz_label, func) in membership_fn_list {
        results.push((fz_label, func(exact_value)));
    }
    results
}

