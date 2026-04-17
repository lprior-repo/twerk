use std::collections::HashMap;
use twerk_core::asl::choice::{ChoiceRule, ChoiceState, ChoiceStateError};
use twerk_core::asl::types::{Expression, StateName};

#[kani::proof]
fn choice_state_requires_at_least_one_choice() {
    let result = ChoiceState::new(vec![], None);
    assert!(result.is_err());

    if let Err(ChoiceStateError::EmptyChoices) = result {
        // Expected error
    } else {
        panic!("Expected EmptyChoices error");
    }
}

#[kani::proof]
fn choice_state_single_rule_is_valid() {
    let state_name = StateName::new("NextState").unwrap();
    let condition = Expression::new("true").unwrap();
    let rule = ChoiceRule::new(condition, state_name, None);
    let choices = vec![rule];

    let result = ChoiceState::new(choices, None);
    assert!(result.is_ok(), "Single rule should be valid");
}

#[kani::proof]
fn choice_rule_accessors_work() {
    let state_name = StateName::new("Target").unwrap();
    let condition = Expression::new("$.value > 5").unwrap();
    let mut assigns = HashMap::new();
    assigns.insert(
        twerk_core::asl::types::VariableName::new("var").unwrap(),
        Expression::new("\"assigned\"").unwrap(),
    );

    let rule = ChoiceRule::new(condition.clone(), state_name.clone(), Some(assigns.clone()));

    assert_eq!(rule.condition(), &condition);
    assert_eq!(rule.next(), &state_name);
    assert!(rule.assign().is_some());

    let returned_assigns = rule.assign().unwrap();
    assert_eq!(returned_assigns.len(), 1);
}

#[kani::proof]
fn choice_state_serialize_deserialize_roundtrip() {
    let rule1 = ChoiceRule::new(
        Expression::new("$.value > 5").unwrap(),
        StateName::new("HighPath").unwrap(),
        None,
    );
    let rule2 = ChoiceRule::new(
        Expression::new("$.value <= 5").unwrap(),
        StateName::new("LowPath").unwrap(),
        None,
    );

    let choices = vec![rule1, rule2];
    let default = StateName::new("DefaultState").unwrap();

    let choice_state = ChoiceState::new(choices, Some(default)).unwrap();

    let serialized = serde_json::to_string(&choice_state).unwrap();
    let deserialized: ChoiceState = serde_json::from_str(&serialized).unwrap();

    assert_eq!(choice_state.choices().len(), deserialized.choices().len());
    assert!(choice_state.default().is_some());
    assert!(deserialized.default().is_some());
}

#[kani::proof]
fn choice_state_deserialize_validates_empty_choices() {
    let json = r#"{"choices": [], "default": "DefaultState"}"#;
    let result: Result<ChoiceState, _> = serde_json::from_str(json);
    assert!(result.is_err(), "Empty choices should fail deserialization");
}

#[kani::proof]
fn choice_state_without_default_is_valid() {
    let rule = ChoiceRule::new(
        Expression::new("true").unwrap(),
        StateName::new("NextState").unwrap(),
        None,
    );

    let result = ChoiceState::new(vec![rule], None);
    assert!(
        result.is_ok(),
        "ChoiceState without default should be valid"
    );
    assert!(result.unwrap().default().is_none());
}
