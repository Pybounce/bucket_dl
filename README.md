Theory Crafting

- [ ] github readme
- [ ] Stop unwrapping
  - It will cause a panic
- [ ] Saving progress mid crash
  - Not something I'll likely do for a while if ever
  - Will need another file that keeps track of progress and gets deleted after
- [ ] Handling threaded errors and retries
  - Right now, once the download has started, there is no communication between the threads downloading the chunks, nor is there a manager for them
  - So if one fails, none of the others will know, and will just continue downloading
  - Also they don't retry if something fails - which they COULD handle on their own, however, if something goes so bad that the thread cannot restart itself, there may be need for a manager/orchestrator of the download to handle this.
- look into publishing

  - what are dev deps
  - optional deps for examples
  - examples
  - is tokio a dev dep?
  - read up on publishing and the crate being found etc
  - probably remove main.rs but make an example with indicatif

- ERROR HANDLING
