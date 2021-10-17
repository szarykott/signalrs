use serde::{Deserialize, Serialize};

// #[derive(Debug, Serialize, Deserialize)]
// /// Indicates individual items of streamed response data from a previous `StreamInvocation` message.
// pub struct Item1 {
//     r#type: u8,
//     item: String,
//     invocation_id: String,
// }

// #[derive(Debug, Serialize, Deserialize)]
// /// Indicates individual items of streamed response data from a previous `StreamInvocation` message.
// pub struct Item2 {
//     r#type: u8,
//     item: Object,
//     invocation_id: String,
// }

// #[test]
// fn test() {
//     let i1 = Item1 {
//         r#type: 1,
//         item: "Dupa".to_string(),
//         invocation_id: "1".to_string(),
//     };

//     let bytes = rmp_serde::to_vec(&i1).unwrap();

//     let i2: Item2 = rmp_serde::from_read_ref(&bytes).unwrap();

//     assert_eq!(i2.invocation_id, "1".to_string());
//     assert_eq!(i2.r#type, 1);
//     assert!(std::matches!(i2.item, Object::String(_)));
// }
