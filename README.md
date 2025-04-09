# bucket_dl

![Tests](https://github.com/Pybounce/bucket_dl/actions/workflows/cargo_test.yml/badge.svg)

A multithreaded downloader with the purpose of faster downloads by splitting it into several requests, rebuilding the data client-side.

> [!Warning]
> This is still a work in progress. Yes there are issues, everything is okay.

## Features

- [x] When possible, splits download into 'buckets', each using a new thread and request.
- [x] Streams live download progress, ideal for visualising loading bars.
- [ ] Retries failed buckets, creating a new thread/request up to x times.
- [ ] Supports pausing and resuming downloads at any time.
- [ ] Automatic pausing in the event of a crash.
- [ ] Actual tests

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
