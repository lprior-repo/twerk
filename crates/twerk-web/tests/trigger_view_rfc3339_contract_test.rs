use std::collections::HashMap;

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use twerk_web::api::trigger_api::TriggerView;

#[test]
fn trigger_view_serializes_created_at_and_updated_at_as_rfc3339_strings() {
    let timestamp = OffsetDateTime::parse("2026-04-22T13:10:48Z", &Rfc3339).expect("timestamp");
    let view = TriggerView {
        id: "trg_contract".to_string(),
        name: "contract-trigger".to_string(),
        enabled: true,
        event: "order.created".to_string(),
        condition: None,
        action: "notify".to_string(),
        metadata: HashMap::new(),
        version: 1,
        created_at: timestamp,
        updated_at: timestamp,
    };

    let json = serde_json::to_value(&view).expect("serialize trigger view");
    let created_at = json["created_at"].as_str().expect("created_at string");
    let updated_at = json["updated_at"].as_str().expect("updated_at string");

    assert_eq!(
        OffsetDateTime::parse(created_at, &Rfc3339).expect("parse created_at"),
        timestamp
    );
    assert_eq!(
        OffsetDateTime::parse(updated_at, &Rfc3339).expect("parse updated_at"),
        timestamp
    );
}
