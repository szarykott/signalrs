use std::vec;

fn main() {
    let values = vec![
        serde_json::Value::Bool(true),
        serde_json::Value::Number(serde_json::Number::from_f64(12.6f64).unwrap()),
    ];

    let serialized = rmp_serde::to_vec(&values).unwrap();

    let deserialized = rmp_serde::from_slice::<(bool, f64)>(&serialized).unwrap();

    println!("{:#?}", deserialized)
}
