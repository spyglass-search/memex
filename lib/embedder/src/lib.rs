use rust_bert::{
    pipelines::sentence_embeddings::{SentenceEmbeddingsBuilder, SentenceEmbeddingsModelType},
    RustBertError,
};
use std::{sync::mpsc, thread::JoinHandle};
use tokio::{sync::oneshot, task};

type Message = (Vec<String>, oneshot::Sender<Vec<Vec<f32>>>);

#[derive(Debug, Clone)]
pub struct SentenceEmbedder {
    sender: mpsc::SyncSender<Message>,
}

impl SentenceEmbedder {
    /// Spawn a classifier on a separate thread and return a classifier instance
    /// to interact with it
    pub fn spawn() -> (JoinHandle<Result<(), RustBertError>>, SentenceEmbedder) {
        let (sender, receiver) = mpsc::sync_channel(100);
        let handle = std::thread::spawn(move || Self::runner(receiver));
        (handle, SentenceEmbedder { sender })
    }

    /// The classification runner itself
    fn runner(receiver: mpsc::Receiver<Message>) -> Result<(), RustBertError> {
        // Needs to be in sync runtime, async doesn't work
        let model = SentenceEmbeddingsBuilder::remote(SentenceEmbeddingsModelType::AllMiniLmL12V2)
            .create_model()?;

        while let Ok((texts, sender)) = receiver.recv() {
            let results = model.encode(&texts)?;
            sender.send(results).expect("sending results");
        }

        Ok(())
    }

    /// Make the runner predict a sample and return the result
    pub async fn encode(&self, texts: Vec<String>) -> anyhow::Result<Vec<Vec<f32>>> {
        let (sender, receiver) = oneshot::channel();
        task::block_in_place(|| self.sender.send((texts, sender)))?;
        Ok(receiver.await?)
    }
}
