pub mod db;
pub mod embedding;
pub mod llm;
pub mod storage;

// Used to generate UUIDs
pub const NAMESPACE: uuid::Uuid = uuid::uuid!("5fdfe40a-de2c-11ed-bfa7-00155deae876");
