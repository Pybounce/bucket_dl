## Features

- [x] When possible, splits download into 'buckets', each using a new thread and request.
- [x] Streams live download progress, ideal for visualising loading bars.
- [x] Cancellation of downloads
- [ ] Single thread downloads
- [x] Supports pausing and resuming downloads at any time.
- [ ] Actual tests
- [ ] Logging to terminal

## Improvements

- [ ] Have a background task check errors.
  - Right now if an error occurs, nothing is cancelled.
  - It's only when the user calls .status() does it check, and then cancel if it's errored
  - This could apply to updating status etc so that calling .status() is no longer mutable, and instead readonly.

## Next Up

- [ ] Single bar downloads
- [ ] Dynamic bars

## Issues

- [ ] Can pause single bar downloads
  - Do they even work?!
- [ ] Failed download doesn't delete tempfile
  - If the fail happens between creating it, but before starting the download.
  - Maybe should not return an error from that start_download function
  - Instead just set the download status to Failed.
