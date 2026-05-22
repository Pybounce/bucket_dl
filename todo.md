## Features

- [x] When possible, splits download into 'buckets', each using a new thread and request.
- [x] Streams live download progress, ideal for visualising loading bars.
- [x] Cancellation of downloads
- [ ] Single thread downloads
- [ ] Retries failed buckets, creating a new thread/request up to x times.
- [ ] Supports pausing and resuming downloads at any time.
- [ ] Automatic pausing in the event of a crash.
- [ ] Actual tests
- [ ] Logging to terminal

## Improvements

- [ ] Have a background task check errors.
  - Right now if an error occurs, nothing is cancelled.
  - It's only when the user calls .status() does it check, and then cancel if it's errored
  - This could apply to updating status etc so that calling .status() is no longer mutable, and instead readonly.

## Next Up

- [ ] Status
  - Follow the same pattern I used in the frontend
  - Save the last status
  - Work out what the current status is
  - Used changed detection to trigger things like finalisation or cancel etc.
    - Maybe cancel shouldn't be triggered here but you get it.
- [ ] Pausing/Unpausing
  - Move channels to a Option<BucketHandler>
  - Make sure a bucket has everything it needs to start/stop a download
  - Not sure about passing in url and out or having it in there

## Issues

- [ ] Cancelling when paused keeps the status as paused, instead of cancelled.
