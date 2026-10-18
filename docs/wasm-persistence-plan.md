# Browser firmware and flash persistence

## Intended behavior

Remember the exact firmware package and all flash contents changed by firmware saves.
Returning to a firmware restores its settings, patches, sequences and samples, to the
extent that firmware writes them to flash. Starting audio still requires a click.
RAM, held keys, playback position and unsaved patch edits are outside this feature.
Support both the FM-1 and SMK-37 pages, including locally opened packages.

## Storage and identity

- Identify firmware by SHA-256 of the entire `.fwsc`, never just its catalog ID or URL.
- Keep a firmware's exact package and sparse 4096-byte flash sectors in IndexedDB.
- Compare against the deterministic, pristine flash after unpacking and factory defaults,
  before DX7 imports or saved sectors. Store whole changed sectors, including erased ones.
- Keep catalog/local selection metadata separately so local packages can be reopened.
- Upgrade the existing database without losing stock packages stored by earlier versions.
- Commit package, sector state and selection metadata atomically. Report success only
  after transaction completion; show failures and retain a downloadable in-memory copy.
- Use a record revision to reject stale writes from another tab rather than overwrite it.
- Cache community packages for repeat visits; provide an explicit check for updates.
  New package bytes get a separate save. Never automatically migrate flash between versions.

## WASM interface

Expose pristine and current flash pointers, flash length, a change revision and whether
an SPI operation is in progress. Increment the revision after erase/program operations.
Expose a validated, boot-only restore operation accepting offset + 4096-byte records.
Reject unaligned, duplicate, out-of-range or truncated sectors before mutating memory.

Restore before the first render. Copy every saved sector whole into the overlapping XIP
window, just like a live SPI erase/program. This includes bytes equal to installed raw
flash: installed raw flash and decrypted XIP differ. Keep untouched sectors decrypted.

Apply a newly imported DX7 bank after restored state, preserving other saved flash.
Use the pristine flash as the comparison baseline so imported voices are saved even
before the firmware explicitly rewrites them. Consume each import once, not every reboot.

## AudioWorklet protocol

Include a boot sequence on every persistence message. Send the baseline once at boot.
Copy current flash only when it changes, at most once a second after a short quiet period.
Perform sector comparisons, serialization, hashing and database writes on the page thread.
Transfer copied ArrayBuffers; never transfer the WASM memory itself.

Provide an explicit snapshot request with a response ID. Capture before replacing a
machine, switching clock/firmware, rebuilding the audio context, exporting or resetting.
Wait for an active SPI operation to finish. Preserve snapshots under the old machine's
identity even if a new selection has already been made. Ignore stale boot responses.

Periodic saving is the primary durability mechanism. Request a best-effort snapshot on
page hide; browser termination cannot guarantee a final asynchronous save.

## Page and controls

Add a separate save status that performance reports cannot overwrite. Provide Save now,
Export save, Import save, Reset saved flash, and Check for firmware updates controls.
Explain that users still use their firmware's own save operation for edits in RAM.
Reset only the selected firmware's flash after confirmation; keep its package.

Export a versioned JSON backup containing the package, hash, display metadata and sectors.
For bring-your-own firmware, omit the package (`package: null`); import resolves an already
opened/cached package and requires the exact hash. The recipient supplies their own .fwsc.
Validate format version, bounded sizes, package hash, sector alignment, length and
duplicates before accepting imports. Verify the package boots before saving it locally.
The export is an emulator save, not a firmware updater or a standard DX7 patch bank.

## Implementation sequence

1. Add flash revision/busy tracking and WASM read/restore/DX7 interfaces.
2. Implement pure sector/backup serialization and IndexedDB storage helpers.
3. Extend worklet boot and snapshot messaging.
4. Integrate cached packages, saved state, local selection and ordered saves into main.js.
5. Add controls, update both pages and build copying, and document behavior.
6. Add focused automated coverage and verify actual built WASM with local packages.

## Verification

- Round trip unchanged, changed, erased and reverted sectors; reject malformed restores.
- Restore a changed sector overlapping XIP and verify its bytes through the CPU bus.
- Boot an actual local package, mutate flash through emulated SPI, export differences,
  boot a fresh instance and restore byte-for-byte. Retain the normal audio/screen smoke test.
- Exercise worklet snapshot sequence IDs, stale boots, boot failures and SPI-busy requests.
- Check IndexedDB migration, transaction failures, conflict detection and backup validation.
- Verify save before firmware/clock/context changes and separate state for different hashes.
- Verify DX7 imports survive subsequent reboots and do not repeatedly overwrite patch edits.
- Build both pages and include the JavaScript tests in CI.
- Inspect browser controls and reload behavior where a browser is available; record any
  validation that could not be run in the available environment.

## Completion criteria

After a firmware saves a patch or setting, the browser indicates it is saved, and a reload
or switch away/back restores it. A backup reproduces the same firmware and flash in a
fresh session. Storage failures are visible, and unrelated firmware saves remain isolated.

## Implemented and verified

The implementation follows the plan above. `web/persistence.js` owns hashing, sparse
sector encoding, versioned `.fm1save` backups, IndexedDB migration and transaction conflict
checks. `web/main.js` serializes reboots and storage commits. The worklet copies a full
1 MiB image only on change or explicit request; the page reduces it to changed sectors.
Replacement snapshots wait for a quiet interval between flash writes, and timed-out
requests are cancelled so they cannot unexpectedly pause a later session.

Validation performed:

- Rust ISA/core/SoC/web tests, including restore validation, SPI revision tracking, XIP
  consistency and bank imports over restored flash.
- `node --test web/*.test.mjs`: page lifecycle, backup validation, storage commit/failure
  behavior, worklet snapshots, reset isolation and preservation after failed replacement boots.
- `bash web/build.sh`: both browser pages and their shared persistence module.
- `node web/persistence.wasm.mjs web/dist reference/firmwares/felucca.fwsc`: actual
  built WASM, rendered firmware, sparse backup encode/decode and byte-for-byte restore.
  The altered sectors in this test are synthetic; it does not claim to exercise Felucca's
  patch-save user interface. Emulated SPI erase/program is covered by the SoC tests.
- `node web/smoke.mjs web/dist reference/firmwares/felucca.fwsc`: audio, screen and MIDI.
- Chrome: all seven checks in `web/persistence.browser.html`, including migration from
  database v1, binary data restoration, rejected stale writes and reset.
- Chrome: imported a generated backup containing a known flash sector, booted, waited
  for the IndexedDB save, reloaded, booted the remembered local firmware, and exported
  again. The exported package hash and all 4096 bytes of the sector matched the fixture.

CI now runs the JavaScript regression tests, web-crate Rust tests and WASM save round trip.
The standalone browser test page is a development fixture and is not copied into the site.

Browser termination can still interrupt the most recent asynchronous save. The saved
indicator represents a completed IndexedDB transaction, and exported backups provide a
separate copy. Physical-device patch-bank export, RAM snapshots, automatic migration across
firmware versions and a library UI for managing old versions remain outside this feature.

## Review corrections

- Restore full sectors in XIP; compare actual SPI erase/program with restore in Rust and
  inspect XIP in the built-WASM round trip. Bank imports touch only changed bank sectors.
- A missing/crashed machine acknowledges snapshots. Render errors clear the running page
  session; a boot timeout cancels the pending worklet boot and clears its machine.
- Reopening an unchanged save only updates its selection entry, leaving its revision intact.
  Package bytes are inserted once; subsequent transactions update sparse state and metadata.
  Compare raw snapshots in 32-bit words on the main thread, rather than byte by byte.
- Every explicit snapshot waits for SPI idle and 250 ms quiet. After 1.5 s, switching may
  proceed using the last stable save; Save/Export reports an error instead of writing an
  unfinished snapshot. Quiescence is a heuristic, not a firmware transaction protocol.
- Visibility changes and pagehide request best-effort snapshots. Browser termination can
  still lose changes since the last committed quiet autosave.
- An unsupported DX7 user bank is ignored, preserving the existing boot behavior.
- Browse saved firmwares exposes exact prior hashes, including older catalog versions and
  local files replaced by later imports. Cross-version patch migration is deferred until
  compatible flash layouts can be established; reopening the old version remains supported.
