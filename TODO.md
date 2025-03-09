# boltdb-rs

# TODO

## 1.4.0

- [ ] Add support for more database operations
- [ ] Write tests

# WIP

## src/bucket.rs
- [ ] bucket.go write trait API and methods
- [ ] bucket.go implement trait API and methods
- [ ] 

## src/cursor.rs
- [ ] Cursor Next and Prev and Seek implementations
- [ ] Cursor Search implementations

## src/db.rs

## src/node.rs
- [ ] rewrite RefCell to auto ref pointer
- [ ] rewrite *const RawBucket to Bucket Reference.

## src/tx.rs

# Go
- [x] internal/common/bucket.go
- [x] internal/common/inode.go
- [x] internal/common/meta.go
- [x] internal/common/page.go
- [ ] internal/common/page_test.go
- [x] internal/common/types.go
- [ ] bucket.go
- [ ] db.go
- [ ] db_test.go
- [x] cursor.go
- [x] errors.go
- [x] errors/errors.go

# NotStarted
- [ ] allocate_test.go
- [ ] bolt_386.go
- [ ] bolt_aix.go
- [ ] bolt_amd64.go
- [ ] bolt_android.go
- [ ] bolt_arm.go
- [ ] bolt_arm64.go
- [ ] bolt_linux.go
- [ ] bolt_loong64.go
- [ ] bolt_mips64x.go
- [ ] bolt_mipsx.go
- [ ] bolt_openbsd.go
- [ ] bolt_ppc.go
- [ ] bolt_ppc64.go
- [ ] bolt_ppc64le.go
- [ ] bolt_riscv64.go
- [ ] bolt_s390x.go
- [ ] bolt_solaris.go
- [ ] bolt_unix.go
- [ ] bolt_windows.go
- [ ] boltsync_unix.go
- [ ] bucket_test.go
- [ ] cmd/bbolt/command_check.go
- [ ] cmd/bbolt/command_check_test.go
- [ ] cmd/bbolt/command_inspect.go
- [ ] cmd/bbolt/command_inspect_test.go
- [ ] cmd/bbolt/command_root.go
- [ ] cmd/bbolt/command_surgery.go
- [ ] cmd/bbolt/command_surgery_freelist.go
- [ ] cmd/bbolt/command_surgery_freelist_test.go
- [ ] cmd/bbolt/command_surgery_meta.go
- [ ] cmd/bbolt/command_surgery_meta_test.go
- [ ] cmd/bbolt/command_surgery_test.go
- [ ] cmd/bbolt/command_version.go
- [ ] cmd/bbolt/main.go
- [ ] cmd/bbolt/main_test.go
- [ ] cmd/bbolt/page_command.go
- [ ] cmd/bbolt/utils.go
- [ ] cmd/bbolt/utils_test.go
- [ ] compact.go
- [ ] concurrent_test.go
- [ ] cursor_test.go
- [ ] db.go
- [ ] db_test.go
- [ ] db_whitebox_test.go
- [x] doc.go
- [ ] internal/btesting/btesting.go
- [ ] internal/common/unsafe.go
- [ ] internal/common/utils.go
- [ ] internal/common/verify.go
- [ ] internal/freelist/array.go
- [ ] internal/freelist/array_test.go
- [ ] internal/freelist/freelist.go
- [ ] internal/freelist/freelist_test.go
- [ ] internal/freelist/hashmap.go
- [ ] internal/freelist/hashmap_test.go
- [ ] internal/freelist/shared.go
- [ ] internal/guts_cli/guts_cli.go
- [ ] internal/surgeon/surgeon.go
- [ ] internal/surgeon/surgeon_test.go
- [ ] internal/surgeon/xray.go
- [ ] internal/surgeon/xray_test.go
- [ ] internal/tests/tx_check_test.go
- [ ] logger.go
- [ ] manydbs_test.go
- [ ] mlock_unix.go
- [ ] mlock_windows.go
- [ ] movebucket_test.go
- [ ] node.go
- [ ] node_test.go
- [ ] quick_test.go
- [ ] simulation_no_freelist_sync_test.go
- [ ] simulation_test.go
- [ ] tests/dmflakey/dmflakey.go
- [ ] tests/dmflakey/dmflakey_test.go
- [ ] tests/dmflakey/dmsetup.go
- [ ] tests/dmflakey/loopback.go
- [ ] tests/failpoint/db_failpoint_test.go
- [ ] tests/robustness/main_test.go
- [ ] tests/robustness/powerfailure_test.go
- [ ] tests/utils/helpers.go
- [ ] tx.go
- [ ] tx_check.go
- [ ] tx_check_test.go
- [ ] tx_stats_test.go
- [ ] tx_test.go
- [ ] unix_test.go
- [ ] utils_test.go
- [x] version/version.go