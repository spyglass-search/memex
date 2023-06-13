use hnsw_rs::{
    hnswio::{load_description, load_hnsw},
    prelude::*,
};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use super::{VectorStore, VectorStoreError};

const PREFIX: &str = "vectors";
const GRAPH_FILE: &str = "vectors.hnsw.graph";
const DATA_FILE: &str = "vectors.hnsw.data";
const META_FILE: &str = "vectors.meta.json";

pub struct HnswStore {
    pub storage_path: PathBuf,
    pub hnsw: Arc<Hnsw<f32, DistCosine>>,
    pub _id_map: HashMap<usize, String>,
}

impl VectorStore for HnswStore {
    fn insert(&mut self, doc_id: &str, vec: &[f32]) -> Result<(), VectorStoreError> {
        let next_id = self._id_map.len() + 1;
        self._id_map.insert(next_id, doc_id.to_string());
        self.hnsw.insert((&vec.to_vec(), next_id));
        // Naively save after each insert
        let _ = self.save(self.storage_path.clone());
        Ok(())
    }

    fn search(&self, vec: &[f32], limit: usize) -> Result<Vec<(String, f32)>, VectorStoreError> {
        let neighbors = self.hnsw.search(vec, limit, 16 * 2);

        let mut results = Vec::new();
        for x in neighbors.iter() {
            let doc_id = self
                ._id_map
                .get(&x.d_id)
                .expect("Internal inconsistency. Id from vector store not mapped.");
            // Calculate similarity score where 1.0 is exact and 0.0 is completely
            // orthoganal.
            let similarity = 1.0 - (1.0 / (1.0 / x.distance));
            results.push((doc_id.to_string(), similarity));
        }

        Ok(results)
    }
}

impl HnswStore {
    pub fn new(storage_path: &Path) -> Self {
        log::info!(
            "Initializing vector storage @ \"{}\"",
            storage_path.display()
        );

        let store = Hnsw::new(16, 100, 16, 200, DistCosine);

        Self {
            storage_path: storage_path.to_path_buf(),
            hnsw: Arc::new(store),
            _id_map: HashMap::new(),
        }
    }

    pub fn has_store(store_path: &Path) -> bool {
        let meta_path = store_path.join(META_FILE);
        meta_path.exists()
    }

    pub fn load(store_path: &Path) -> Result<Self, VectorStoreError> {
        log::info!("Loading vector storage @ \"{}\"", store_path.display());

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
            storage_path: store_path.to_path_buf(),
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
}

#[cfg(test)]
mod test {
    use super::{HnswStore, VectorStore};
    use std::path::Path;

    #[test]
    fn test_hnsw() {
        let path = Path::new("/tmp");
        let mut store = HnswStore::new(&path);
        store.insert("test-one", &vec![0.0, 0.1, 0.2]).unwrap();
        store.insert("test-two", &vec![0.1, 0.1, 0.1]).unwrap();
        store.insert("test-three", &vec![0.3, 0.2, 0.1]).unwrap();

        let results = store.search(&vec![0.1, 0.1, 0.1], 3).unwrap();
        assert_eq!(results.len(), 3);

        // First result should be "test-two"
        let (doc_id, _) = results.get(0).unwrap();
        assert_eq!(doc_id, "test-two");
    }

    #[test]
    fn test_save_load() {
        let path = Path::new("/tmp");
        let mut store = HnswStore::new(&path);
        store.insert("test-one", &vec![0.0, 0.1, 0.2]).unwrap();
        store.insert("test-two", &vec![0.1, 0.1, 0.1]).unwrap();
        store.insert("test-three", &vec![0.3, 0.2, 0.1]).unwrap();

        assert!(store.save("/tmp".into()).is_ok());

        let loaded = HnswStore::load(&path).unwrap();
        assert_eq!(loaded._id_map.len(), store._id_map.len());
    }
}
