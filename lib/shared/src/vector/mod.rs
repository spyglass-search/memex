use hnsw_rs::{
    hnswio::{load_description, load_hnsw},
    prelude::*,
};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Write},
    path::PathBuf,
    sync::Arc,
};
use thiserror::Error;

const PREFIX: &str = "vectors";
const GRAPH_FILE: &str = "vectors.hnsw.graph";
const DATA_FILE: &str = "vectors.hnsw.data";
const META_FILE: &str = "vectors.meta.json";

#[derive(Debug, Error)]
pub enum VectorStoreError {
    #[error("File IO error: {0}")]
    FileIOError(#[from] std::io::Error),
    #[error("Unable to deserialize: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("Unable to save db file: {0}")]
    SaveError(String),
}

pub struct HnswStore {
    pub hnsw: Arc<Hnsw<f32, DistCosine>>,
    pub _id_map: HashMap<usize, String>,
}

impl HnswStore {
    pub fn new() -> Self {
        let store = Hnsw::new(16, 100, 16, 200, DistCosine);

        Self {
            hnsw: Arc::new(store),
            _id_map: HashMap::new(),
        }
    }

    pub fn load(store_path: PathBuf) -> Result<Self, VectorStoreError> {
        let graph_path = store_path.join(GRAPH_FILE);
        let graph_fhand = File::open(graph_path)?;

        let data_path = store_path.join(DATA_FILE);
        let data_fhand = File::open(data_path)?;

        let mut graph_in = BufReader::new(graph_fhand);
        let mut data_in = BufReader::new(data_fhand);

        let desc = load_description(&mut graph_in).unwrap();
        let hnsw_loaded: Hnsw<f32, DistCosine> =
            load_hnsw(&mut graph_in, &desc, &mut data_in).unwrap();

        let meta_path = store_path.join(META_FILE);
        let meta_fhand = File::open(meta_path)?;
        let meta_reader = BufReader::new(meta_fhand);
        let _id_map: HashMap<usize, String> = serde_json::from_reader(meta_reader)?;

        Ok(Self {
            hnsw: Arc::new(hnsw_loaded),
            _id_map,
        })
    }

    pub fn save(&self, store_path: PathBuf) -> Result<(), VectorStoreError> {
        let filename = store_path.join(PREFIX).display().to_string();

        let _ = self
            .hnsw
            .file_dump(&filename)
            .map_err(VectorStoreError::SaveError)?;

        // Save id map as a json file
        let result = serde_json::to_string(&self._id_map)
            .map_err(|err| VectorStoreError::SaveError(err.to_string()))?;

        let id_map = store_path.join(META_FILE);
        let mut f = File::create(id_map)?;
        let _ = f.write(result.as_bytes())?;
        f.flush()?;

        Ok(())
    }

    pub fn search(&self, vec: &[f32]) -> Result<Vec<(String, f32)>, VectorStoreError> {
        let neighbors = self.hnsw.search(vec, 10, 16 * 2);

        let mut results = Vec::new();
        for x in neighbors.iter() {
            let doc_id = self._id_map.get(&x.d_id)
                .expect("Internal inconsistency. Id from vector store not mapped.");
            // Calculate similarity score where 1.0 is exact and 0.0 is completely
            // orthoganal.
            let similarity = 1.0 - (1.0 / (1.0 / x.distance));
            results.push((doc_id.to_string(), similarity));
        }

        Ok(results)
    }

    pub fn insert(&mut self, doc_id: &str, vec: Vec<f32>) {
        let next_id = self._id_map.len() + 1;
        self._id_map.insert(next_id, doc_id.to_string());
        self.hnsw.insert((&vec, next_id))
    }
}

#[cfg(test)]
mod test {
    use super::HnswStore;

    #[test]
    fn test_hnsw() {
        let mut store = HnswStore::new();
        store.insert("test-one", vec![0.0, 0.1, 0.2]);
        store.insert("test-two", vec![0.1, 0.1, 0.1]);
        store.insert("test-three", vec![0.3, 0.2, 0.1]);

        let results = store.search(&vec![0.1, 0.1, 0.1]).unwrap();
        assert_eq!(results.len(), 3);

        // First result should be "test-two"
        let (doc_id, _) = results.get(0).unwrap();
        assert_eq!(doc_id, "test-two");
    }

    #[test]
    fn test_save_load() {
        let mut store = HnswStore::new();
        store.insert("test-one", vec![0.0, 0.1, 0.2]);
        store.insert("test-two", vec![0.1, 0.1, 0.1]);
        store.insert("test-three", vec![0.3, 0.2, 0.1]);

        assert!(store.save("/tmp".into()).is_ok());

        let loaded = HnswStore::load("/tmp".into()).unwrap();
        assert_eq!(loaded._id_map.len(), store._id_map.len());
    }
}
