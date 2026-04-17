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

#[test]
fn level0_increase() {
    run(include_str!("integration/level0/increase.py"));
}

#[test]
fn level0_digits() {
    run(include_str!("integration/level0/digits.py"));
}

#[test]
fn level0_zero_last_digits() {
    run(include_str!("integration/level0/zero_last_digits.py"));
}

// #[test] string concatenation and open-ended slices not supported
fn _ignore_level0_reformat_date() {
    run(include_str!("integration/level0/reformat_date.py"));
}

#[test]
fn level0_fare_exempt() {
    run(include_str!("integration/level0/fare_exempt.py"));
}

// #[test] negative indexing and char comparison not supported
fn _ignore_level0_no_edge_spaces() {
    run(include_str!("integration/level0/no_edge_spaces.py"));
}

// #[test] negative indexing not supported
fn _ignore_level0_name_check() {
    run(include_str!("integration/level0/name_check.py"));
}

// #[test] string concatenation and open-ended slices not supported
fn _ignore_level0_rotate_right() {
    run(include_str!("integration/level0/rotate_right.py"));
}

// #[test] string multiplication not supported
fn _ignore_level0_censor() {
    run(include_str!("integration/level0/censor.py"));
}

// #[test] .upper() and .lower() not supported
fn _ignore_level0_capitalize_first() {
    run(include_str!("integration/level0/capitalize_first.py"));
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

#[test]
fn level1_sign() {
    run(include_str!("integration/level1/sign.py"));
}

#[test]
fn level1_investment() {
    run(include_str!("integration/level1/investment.py"));
}

#[test]
fn level1_clamp() {
    run(include_str!("integration/level1/clamp.py"));
}

// #[test] open-ended slices and char comparison not supported
fn _ignore_level1_doubled_word() {
    run(include_str!("integration/level1/doubled_word.py"));
}

// #[test] string multiplication not supported
fn _ignore_level1_resize_string() {
    run(include_str!("integration/level1/resize_string.py"));
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

#[test]
fn level2_direction() {
    run(include_str!("integration/level2/direction.py"));
}

// #[test] string < comparison not supported
fn _ignore_level2_soccer() {
    run(include_str!("integration/level2/soccer.py"));
}

// #[test] type mismatch with multiple dataclasses
fn _ignore_level2_window() {
    run(include_str!("integration/level2/window.py"));
}

#[test]
fn level2_rock_paper_scissors() {
    run(include_str!("integration/level2/rock_paper_scissors.py"));
}

// #[test] null reference accessing enum field in struct
fn _ignore_level2_max_steps() {
    run(include_str!("integration/level2/max_steps.py"));
}

#[test]
fn level2_resolution() {
    run(include_str!("integration/level2/resolution.py"));
}

#[test]
fn level2_season() {
    run(include_str!("integration/level2/season.py"));
}

// #[test] named match patterns (variable binding) not supported
fn _ignore_level2_match_binding() {
    run(include_str!("integration/level2/match_binding.py"));
}

// #[test] sequence match patterns ([x], [x, y], [x, *rest]) not supported
fn _ignore_level2_match_list() {
    run(include_str!("integration/level2/match_list.py"));
}

// #[test] class match patterns (Pixel(x=x, y=y, ...)) not supported
fn _ignore_level2_match_struct() {
    run(include_str!("integration/level2/match_struct.py"));
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

// #[test] list == [] uses reference equality, not structural
fn _ignore_level3_remove_zeros() {
    run(include_str!("integration/level3/remove_zeros.py"));
}

#[test]
fn level3_all_false() {
    run(include_str!("integration/level3/all_false.py"));
}

// #[test] list + list not supported
fn _ignore_level3_negatives_first() {
    run(include_str!("integration/level3/negatives_first.py"));
}

#[test]
fn level3_majority() {
    run(include_str!("integration/level3/majority.py"));
}

// #[test] for loop over list[struct] not supported
fn _ignore_level3_championship() {
    run(include_str!("integration/level3/championship.py"));
}

// #[test] for loop over list[enum] not supported
fn _ignore_level3_new_position() {
    run(include_str!("integration/level3/new_position.py"));
}

// #[test] struct in list type mismatch
fn _ignore_level3_bounding_box() {
    run(include_str!("integration/level3/bounding_box.py"));
}

#[test]
fn level3_doubled_list() {
    run(include_str!("integration/level3/doubled_list.py"));
}

// #[test] list == [...] uses reference equality, not structural
fn _ignore_level3_insert_at() {
    run(include_str!("integration/level3/insert_at.py"));
}

// #[test] list == [...] uses reference equality, not structural
fn _ignore_level3_remove_at() {
    run(include_str!("integration/level3/remove_at.py"));
}

// #[test] mutual recursion not supported
fn _ignore_level3_even_odd() {
    run(include_str!("integration/level3/even_odd.py"));
}

#[test]
fn level3_pi_series() {
    run(include_str!("integration/level3/pi_series.py"));
}

#[test]
fn level3_factorial_rec() {
    run(include_str!("integration/level3/factorial_rec.py"));
}

// #[test] list slicing in recursion not supported
fn _ignore_level3_concatenate() {
    run(include_str!("integration/level3/concatenate.py"));
}

// #[test] list slicing in recursion not supported
fn _ignore_level3_recursive_max() {
    run(include_str!("integration/level3/recursive_max.py"));
}
