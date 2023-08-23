# memex

Super simple "memory" for LLM projects, semantic search, etc.

<p align="center">
    <img src="docs/memex-in-action.gif">
</p>

## Running the service

Note that if you're running on Apple silicon (M1/M2/etc.), it's best to run natively (and faster)
since Linux ARM builds are very finicky.

``` bash
# Build and run the docker image. This will build & run memex as well as an opensearch
# node for document storage + search.
> docker-compose up
# OR run natively in you have the rust toolchain installed. This uses the default file
# based vector store instead of opensearch which may yield worse results.
> cp .env.template .env
> cargo run --release -p memex serve
# If everything is running correctly, you should see something like:
2023-06-13T05:04:21.518732Z  INFO memex: starting server with roles: [Api, Worker]
```

## Add a document

NOTE: If the `test` collection does not initially exist, it'll be created.

``` bash
> curl http://localhost:8181/collections/test \
    -H "Content-Type: application/json" \
    --data @example_docs/state_of_the_union_2023.json
{
    "task_id": 1,
    "collection": "test",
    "status": "Queued",
    ...
}
```

Feel free to add as many documents as you want. Each one will be enqueued and processed
as they are added.

Wait a couple seconds per document to be processed. You can check the status
using the `task_id` above like so:

## Check task status

``` bash
> curl http://localhost:8181/tasks/1
{
    "task_id": 1,
    "status": "Processing"
}
```

One the task is shown as "Completed", you can now run a query against the doc(s)
you've just added.

## Run a query

``` bash
> curl http://localhost:8181/collections/test/search \
    -H "Content-Type: application/json" \
    -X GET \
    -d "{\"query\": \"what does Biden say about taxes?\", \"limit\": 3}"
[{
    "_id": <internal_id>, // reference to this particular segment text.
    "task_id": <task_id>, // The original document that this came from.
    "segment": <document section>,
    "content": <content block>,
    "score": <relevancy score>
}, ...]
```

## Env variables

- `HOST` & `PORT`:

## Examples

For any of these examples, make sure you have `memex` running in the background.

### Clippy

#### Ask questions about a single or many document(s)

``` bash
# In a different terminal, run memex
> cargo run --release -p memex serve
# In your main terminal
> cd examples/clippy
# NOTE: there is no duplicate detection so running this twice will add the file twice.
> cargo run -- load-file example_docs/state_of_the_union.txt
# To ask clippy about your files, use "ask"
> cargo run -- ask "what does biden say about taxes?"
# To ask clippy without referring to it's memex and _ONLY_ relying on the knowledge
# inside it's LLM, use "qq" / "quick-question"
> cargo run -- qq "wget command to save a file to a directory"
# To clear clippy's memory
> cargo run -- forget
```