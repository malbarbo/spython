mod testlib;

use testlib::run;

// =====================================================================
// Level 0 — Functions
// =====================================================================

#[test]
fn level0_double() {
    run(include_str!("integration/level0/double.py"));
}

// #[test]
fn _ignore_level0_hypotenuse() {
    run(include_str!("integration/level0/hypotenuse.py"));
}

// #[test]
fn _ignore_level0_new_century() {
    run(include_str!("integration/level0/new_century.py"));
}

#[test]
fn level0_trip_cost() {
    run(include_str!("integration/level0/trip_cost.py"));
}

// #[test]
fn _ignore_level0_iron_pipe_mass() {
    run(include_str!("integration/level0/iron_pipe_mass.py"));
}

// #[test]
fn _ignore_level0_tile_count() {
    run(include_str!("integration/level0/tile_count.py"));
}

// =====================================================================
// Level 1 — Selection (if/elif/else)
// =====================================================================

// #[test]
fn _ignore_level1_adjust_phone() {
    run(include_str!("integration/level1/adjust_phone.py"));
}

// #[test]
fn _ignore_level1_center() {
    run(include_str!("integration/level1/center.py"));
}

#[test]
fn level1_fuel_choice() {
    run(include_str!("integration/level1/fuel_choice.py"));
}

#[test]
fn level1_maximum() {
    run(include_str!("integration/level1/maximum.py"));
}

// #[test]
fn _ignore_level1_add_period() {
    run(include_str!("integration/level1/add_period.py"));
}

// =====================================================================
// Level 2 — User types (Enum, dataclass, match)
// =====================================================================

#[test]
fn level2_fuel() {
    run(include_str!("integration/level2/fuel.py"));
}

// #[test]
fn _ignore_level2_lottery() {
    run(include_str!("integration/level2/lottery.py"));
}

#[test]
fn level2_traffic_light() {
    run(include_str!("integration/level2/traffic_light.py"));
}

// #[test]
fn _ignore_level2_duration() {
    run(include_str!("integration/level2/duration.py"));
}

// =====================================================================
// Level 3 — Repetition and lists
// =====================================================================

// #[test]
fn _ignore_level3_passing_students() {
    run(include_str!("integration/level3/passing_students.py"));
}

// #[test]
fn _ignore_level3_find_starts_a() {
    run(include_str!("integration/level3/find_starts_a.py"));
}

// #[test]
fn _ignore_level3_lottery_list() {
    run(include_str!("integration/level3/lottery_list.py"));
}

#[test]
fn level3_list_maximum() {
    run(include_str!("integration/level3/list_maximum.py"));
}

#[test]
fn level3_avg_length() {
    run(include_str!("integration/level3/avg_length.py"));
}

#[test]
fn level3_list_sum() {
    run(include_str!("integration/level3/list_sum.py"));
}

// #[test]
fn _ignore_level3_factorial() {
    run(include_str!("integration/level3/factorial.py"));
}

// #[test]
fn _ignore_level3_max_index() {
    run(include_str!("integration/level3/max_index.py"));
}

// #[test]
fn _ignore_level3_count_zeros() {
    run(include_str!("integration/level3/count_zeros.py"));
}

// #[test]
fn _ignore_level3_zero_matrix() {
    run(include_str!("integration/level3/zero_matrix.py"));
}

#[test]
fn level3_regular_matrix() {
    run(include_str!("integration/level3/regular_matrix.py"));
}

// #[test]
fn _ignore_level3_transpose() {
    run(include_str!("integration/level3/transpose.py"));
}

// #[test]
fn _ignore_level3_non_decreasing() {
    run(include_str!("integration/level3/non_decreasing.py"));
}

#[test]
fn level3_palindrome() {
    run(include_str!("integration/level3/palindrome.py"));
}

#[test]
fn level3_prime() {
    run(include_str!("integration/level3/prime.py"));
}

// #[test]
fn _ignore_level3_insert_sorted() {
    run(include_str!("integration/level3/insert_sorted.py"));
}

// #[test]
fn _ignore_level3_reverse() {
    run(include_str!("integration/level3/reverse.py"));
}

// #[test]
fn _ignore_level3_in_order() {
    run(include_str!("integration/level3/in_order.py"));
}

// #[test]
fn _ignore_level3_frequency() {
    run(include_str!("integration/level3/frequency.py"));
}

#[test]
fn level3_power() {
    run(include_str!("integration/level3/power.py"));
}

// #[test]
fn _ignore_level3_sum_recursive() {
    run(include_str!("integration/level3/sum_recursive.py"));
}

#[test]
fn level3_sum_naturals() {
    run(include_str!("integration/level3/sum_naturals.py"));
}
