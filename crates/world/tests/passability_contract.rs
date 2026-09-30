//! Wire-format contracts for the six named faces in `Passability`.
//!
//! Each face is exercised independently so a field-to-face mix-up cannot hide
//! behind a successful round trip of a value that uses the same mapping twice.

use rstest::rstest;
use thysalion_world::scene::document::{Face, Passability};

const FACE_NAMES: [&str; 6] = ["pos_x", "neg_x", "pos_y", "neg_y", "pos_z", "neg_z"];

/// Builds a value with just the selected face passable for wire-contract cases.
fn passability_with_only_face_open(face: Face) -> Passability {
    match face {
        Face::PosX => Passability {
            pos_x: true.into(),
            ..Passability::closed()
        },
        Face::NegX => Passability {
            neg_x: true.into(),
            ..Passability::closed()
        },
        Face::PosY => Passability {
            pos_y: true.into(),
            ..Passability::closed()
        },
        Face::NegY => Passability {
            neg_y: true.into(),
            ..Passability::closed()
        },
        Face::PosZ => Passability {
            pos_z: true.into(),
            ..Passability::closed()
        },
        Face::NegZ => Passability {
            neg_z: true.into(),
            ..Passability::closed()
        },
    }
}

/// Builds the expected MessagePack map in the published face-field order.
fn expected_named_map(values: [bool; 6]) -> rmpv::Value {
    let keys = ["pos_x", "neg_x", "pos_y", "neg_y", "pos_z", "neg_z"];
    let entries: Vec<(rmpv::Value, rmpv::Value)> = keys
        .into_iter()
        .zip(values)
        .map(|(key, value)| (rmpv::Value::from(key), rmpv::Value::from(value)))
        .collect();

    rmpv::Value::Map(entries)
}

/// Every face remains one independent named boolean in both wire formats.
#[rstest]
#[case::pos_x(
    Face::PosX,
    "pos_x",
    r#"{"pos_x":true,"neg_x":false,"pos_y":false,"neg_y":false,"pos_z":false,"neg_z":false}"#,
    [true, false, false, false, false, false],
)]
#[case::neg_x(
    Face::NegX,
    "neg_x",
    r#"{"pos_x":false,"neg_x":true,"pos_y":false,"neg_y":false,"pos_z":false,"neg_z":false}"#,
    [false, true, false, false, false, false],
)]
#[case::pos_y(
    Face::PosY,
    "pos_y",
    r#"{"pos_x":false,"neg_x":false,"pos_y":true,"neg_y":false,"pos_z":false,"neg_z":false}"#,
    [false, false, true, false, false, false],
)]
#[case::neg_y(
    Face::NegY,
    "neg_y",
    r#"{"pos_x":false,"neg_x":false,"pos_y":false,"neg_y":true,"pos_z":false,"neg_z":false}"#,
    [false, false, false, true, false, false],
)]
#[case::pos_z(
    Face::PosZ,
    "pos_z",
    r#"{"pos_x":false,"neg_x":false,"pos_y":false,"neg_y":false,"pos_z":true,"neg_z":false}"#,
    [false, false, false, false, true, false],
)]
#[case::neg_z(
    Face::NegZ,
    "neg_z",
    r#"{"pos_x":false,"neg_x":false,"pos_y":false,"neg_y":false,"pos_z":false,"neg_z":true}"#,
    [false, false, false, false, false, true],
)]
fn each_face_keeps_its_flat_wire_mapping(
    #[case] face: Face,
    #[case] face_key: &str,
    #[case] expected_json: &str,
    #[case] expected_values: [bool; 6],
) {
    let passability = passability_with_only_face_open(face);

    let json = serde_json::to_vec(&passability).expect("Passability should serialize as JSON");
    assert_eq!(
        json.as_slice(),
        expected_json.as_bytes(),
        "JSON bytes should set only the {face_key} face",
    );
    let from_json: Passability =
        serde_json::from_slice(&json).expect("Passability should deserialize from JSON");
    assert_eq!(
        from_json, passability,
        "JSON round-trip should preserve the {face_key} face",
    );

    let messagepack = rmp_serde::to_vec_named(&passability)
        .expect("Passability should serialize as a named MessagePack map");
    let messagepack_map: rmpv::Value =
        rmp_serde::from_slice(&messagepack).expect("MessagePack should decode to a value map");
    assert!(
        messagepack_map.is_map(),
        "MessagePack for {face_key} should be a named map, got {messagepack_map:?}",
    );
    assert_eq!(
        messagepack_map,
        expected_named_map(expected_values),
        "MessagePack should contain the six flat faces, with only {face_key} true",
    );
    let from_messagepack: Passability = rmp_serde::from_slice(&messagepack)
        .expect("Passability should deserialize from named MessagePack");
    assert_eq!(
        from_messagepack, passability,
        "MessagePack round-trip should preserve the {face_key} face",
    );
}

/// The generated schema keeps all six named face properties flat booleans.
#[test]
fn schema_has_six_flat_boolean_face_properties() {
    let schema: serde_json::Value = serde_json::to_value(schemars::schema_for!(Passability))
        .expect("Passability schema should serialize as JSON");
    let properties = schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .expect("Passability schema should expose flat face properties");

    assert_eq!(
        properties.len(),
        FACE_NAMES.len(),
        "schema should contain exactly the six named faces",
    );
    for face_name in FACE_NAMES {
        let property = properties
            .get(face_name)
            .expect("each named face should be a schema property");
        assert_eq!(
            property.get("type").and_then(serde_json::Value::as_str),
            Some("boolean"),
            "face `{face_name}` should remain a flat boolean property",
        );
        assert!(
            property.get("$ref").is_none(),
            "face `{face_name}` should not be a nested schema reference",
        );
    }
}

/// Every face remains required in the published field order.
#[test]
fn schema_requires_faces_in_declared_order() {
    let schema: serde_json::Value = serde_json::to_value(schemars::schema_for!(Passability))
        .expect("Passability schema should serialize as JSON");
    let required: Vec<&str> = schema
        .get("required")
        .and_then(serde_json::Value::as_array)
        .expect("all six face properties should be required")
        .iter()
        .map(|field| {
            field
                .as_str()
                .expect("required face names should be strings")
        })
        .collect();
    assert_eq!(
        required,
        FACE_NAMES.to_vec(),
        "required faces should preserve the declared field order",
    );
}

/// The generated schema rejects unknown fields and omits nested face schemas.
#[test]
fn schema_rejects_unknown_fields_without_nested_face_definitions() {
    let schema: serde_json::Value = serde_json::to_value(schemars::schema_for!(Passability))
        .expect("Passability schema should serialize as JSON");
    assert_eq!(
        schema.get("additionalProperties"),
        Some(&serde_json::Value::Bool(false)),
        "schema should continue to reject unknown face fields",
    );

    for definition_keyword in ["$defs", "definitions"] {
        if let Some(definitions) = schema
            .get(definition_keyword)
            .and_then(serde_json::Value::as_object)
        {
            assert!(
                !definitions.contains_key("FacePassability"),
                "transparent faces should not add a FacePassability definition",
            );
        }
    }
}

/// Unknown face names remain invalid even when all known fields are present.
#[test]
fn json_rejects_an_unknown_face_field() {
    let json = concat!(
        r#"{"pos_x":false,"neg_x":false,"pos_y":false,"neg_y":false,"#,
        r#""pos_z":false,"neg_z":false,"diagonal":true}"#,
    );
    let error = serde_json::from_slice::<Passability>(json.as_bytes())
        .expect_err("an unknown face field must be rejected");

    assert!(
        error.to_string().contains("unknown field `diagonal`"),
        "expected an unknown-field error for `diagonal`, got: {error}",
    );
}
