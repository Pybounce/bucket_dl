Theory Crafting

- [ ] Instead of using content-size to derive buckets, it may be better to use speed and content size.
  - Since the bottleneck is the server throttling you, not the size
  - Could download the first x bytes and calculate a speed, then use that and the content size to derive bucket count for the remainder
- [ ] Will need to write to the file as it downloads
  - Right now it downloads the entire file to RAM, before then moving it to a file
  - This sucks because RAM is comparatively small to what you might be downloading, also if it crashes you lose everything but whatever
  - Writing to file mid-download would mean I could allocate set-sized buffers for data
  - Though some of the time, 2 buckets will be trying to write to a file at once and block each other. (Could write to multiple files and combine them..?)
  - Could also give each bucket 2 buffers, then once one is full, it switches to the other and sends the original on to another thread to write to the file when it eventually isn't locked.
  - UPDATE: Turns out file systems are smart enough to do this on their own unless you explicitely flush
- [ ] Saving progress mid crash
  - Not something I'll likely do for a while if ever
  - Will need another file that keeps track of progress and gets deleted after
