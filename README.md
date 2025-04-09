# The Name Here

A multithreaded download manager with the purpose of faster downloads by splitting the download in to several requests, rebuilding the data client-side.

## Features

- [x] When possible, splits download into 'buckets', each using a new thread and request.
- [x] Streams live download progress, ideal for visualising loading bars.
- [ ] Retries failed buckets by creating a new thread/request, up to x times.
- [ ] Supports pausing and resuming downloads at any time.
- [ ] Automatic pausing in the event of a crash.

## Usage

> [!Note]
> For more detailed usage, look at the examples/ directory.

```rust
  let mut client = DownloadClient::init(&url, &file_path);

  if let Ok(_) = client.begin_download().await {
    let mut stream = client.progress_stream();
    while let Some(bucket_progress) = stream.next().await {...}

    match client.status() {
      DownloadStatus::Finished => {...},
      _ => {...}
    }

  }
```

> [!Warning]
> Always remember to check the status of the download, even after exhausting the progress updates.
