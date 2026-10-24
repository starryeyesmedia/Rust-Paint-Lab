# Supplemental upstream notices

These crate archives omit a top-level license file. Notices were retrieved over
verified HTTPS from the exact upstream revisions in each archive's
`.cargo_vcs_info.json`, without modifying dependency source. The packaging script
uses these only for the exact locked versions; missing notices for new versions
fail packaging so they can be investigated.

| Crate | Version | Upstream revision / notice |
| --- | --- | --- |
| gl_generator | 0.14.0 | [gl-rs ea503e8/LICENSE](https://github.com/brendanzab/gl-rs/blob/ea503e8d5fb6d73c6030e6191ce738cd3bf3433e/LICENSE) |
| khronos_api | 3.1.0 | [gl-rs f150967/LICENSE](https://github.com/brendanzab/gl-rs/blob/f150967b1c44ae888e6676f93f639ebc82771bdc/LICENSE) |
| profiling | 1.0.18 | [profiling 8271551](https://github.com/aclysma/profiling/tree/8271551172eb6fa4cba47369aedd93790c623df9), LICENSE-MIT and LICENSE-APACHE |
| spirv | 0.4.0+sdk-1.4.341.0 | [rspirv 8afc3d0/LICENSE](https://github.com/gfx-rs/rspirv/blob/8afc3d0ac8e158128cd1410bb2e4b4c26ab11bb4/LICENSE) |


Overdrive additions, exact `.cargo_vcs_info.json` revisions:

| Crate | Revision and notice source |
| --- | --- |
| egui, emath, ecolor, epaint, egui-wgpu, egui-winit 0.33.3 | emilk/egui `44cdd653e2317d300fb8a6c9c36b03f23991e803`, root LICENSE-MIT and LICENSE-APACHE |
| gpu-alloc 0.6.2 | zakarumych/gpu-alloc `61020bdcb4eb04fca6bbf748aeb9570e859236e0`, license/MIT and license/APACHE |
| gpu-alloc-types 0.3.1 | same repository `7ddd1fe576fcfc503347eb97e476a47581479f22`, byte-identical license texts verified |
| gpu-descriptor 0.3.2 | zakarumych/gpu-descriptor `bb28b4e55b742e0ea3e7a0968e7ee6287ba0e06a`, license/MIT and license/APACHE |
| gpu-descriptor-types 0.2.0 | same repository `9f2fb9546591322e7b77ee1e79385e01effd85e4`, byte-identical license texts verified |
| spirv 0.3.0+sdk-1.3.268.0 | gfx-rs/rspirv `e91a5debc10c395b36db7721dfff6326d697a1e2`, LICENSE |
| clipboard-win 5.4.3 | DoumanAsh/clipboard-win `ed27008ae5d1f10cfbb266de039f0fb35fd05a56`, LICENSE |
| hexf-parse 0.2.1 | Manifest declares CC0-1.0; full canonical text from spdx/license-list-data v3.26.0 text/CC0-1.0.txt |

Bundled fonts are collected recursively from their shipped font license files.

block 0.1.6 archive and upstream tag 0.1.6 (47178790cfc9d4a8b092051d8b413b78bd31254a,
SSheldon/rust-block) contain no separate license/copyright text. The manifest
explicitly declares MIT and author Steven Sheldon. The supplement retains that
attribution and the canonical SPDX MIT license text from license-list-data
v3.26.0, without inventing missing copyright fields or claiming upstream supplied it.

block2 0.5.1: madsmtm/objc2 revision 4fc083f1c6d6784577e38b0ee8dbd344481e2fd2, root LICENSE.txt (archive VCS metadata).

dispatch 0.2.0: same manifest-MIT/canonical-text approach as block. Archive VCS revision
82d6c7a5b75dc0c71c3f46f87bb6c16a476f7748, SSheldon/rust-dispatch; no notice file upstream.

dispatch2 0.3.1: objc2 8852b424193ca41602281b3d7540d7c8ed51e49a root LICENSE.md.

Exact Objective-C workspace revision notice supplements:

- objc-sys 0.3.5: madsmtm/objc2 4fc083f1c6d6784577e38b0ee8dbd344481e2fd2, root LICENSE.txt.
- objc2 0.5.3: madsmtm/objc2 5237cbf081d04ef75a1f6a207b4018e7c1ae7438, root LICENSE.txt.
- objc2 0.6.5: madsmtm/objc2 d7d2fa23ceaa5e6096c923b081040e5d81b3b9df, root LICENSE.md.
- objc2-app-kit 0.2.2: madsmtm/objc2 e282618be4c3a3b9542957e0c8540e9588472ce8, root LICENSE.txt.
- objc2-app-kit 0.3.2: madsmtm/objc2 7b1abfd750a2cacaea71d6a56ecfb83cb7de560b, root LICENSE.md.
- objc2-cloud-kit 0.2.2: madsmtm/objc2 e282618be4c3a3b9542957e0c8540e9588472ce8, root LICENSE.md.
- objc2-contacts 0.2.2: madsmtm/objc2 e282618be4c3a3b9542957e0c8540e9588472ce8, root LICENSE.md.
- objc2-core-data 0.2.2: madsmtm/objc2 e282618be4c3a3b9542957e0c8540e9588472ce8, root LICENSE.md.
- objc2-core-foundation 0.3.2: madsmtm/objc2 7b1abfd750a2cacaea71d6a56ecfb83cb7de560b, root LICENSE.md.
- objc2-core-graphics 0.3.2: madsmtm/objc2 7b1abfd750a2cacaea71d6a56ecfb83cb7de560b, root LICENSE.md.
- objc2-core-image 0.2.2: madsmtm/objc2 e282618be4c3a3b9542957e0c8540e9588472ce8, root LICENSE.md.
- objc2-core-location 0.2.2: madsmtm/objc2 e282618be4c3a3b9542957e0c8540e9588472ce8, root LICENSE.md.
- objc2-encode 4.1.0: madsmtm/objc2 8d214f5477365ffcbcbb7de058c86ed9a518efb7, root LICENSE.md.
- objc2-foundation 0.2.2: madsmtm/objc2 e282618be4c3a3b9542957e0c8540e9588472ce8, root LICENSE.md.
- objc2-foundation 0.3.2: madsmtm/objc2 7b1abfd750a2cacaea71d6a56ecfb83cb7de560b, root LICENSE.md.
- objc2-io-surface 0.3.2: madsmtm/objc2 7b1abfd750a2cacaea71d6a56ecfb83cb7de560b, root LICENSE.md.
- objc2-link-presentation 0.2.2: madsmtm/objc2 e282618be4c3a3b9542957e0c8540e9588472ce8, root LICENSE.md.
- objc2-metal 0.2.2: madsmtm/objc2 e282618be4c3a3b9542957e0c8540e9588472ce8, root LICENSE.md.
- objc2-metal 0.3.2: madsmtm/objc2 7b1abfd750a2cacaea71d6a56ecfb83cb7de560b, root LICENSE.md.
- objc2-quartz-core 0.2.2: madsmtm/objc2 e282618be4c3a3b9542957e0c8540e9588472ce8, root LICENSE.md.
- objc2-quartz-core 0.3.2: madsmtm/objc2 7b1abfd750a2cacaea71d6a56ecfb83cb7de560b, root LICENSE.md.
- objc2-symbols 0.2.2: madsmtm/objc2 e282618be4c3a3b9542957e0c8540e9588472ce8, root LICENSE.md.
- objc2-ui-kit 0.2.2: madsmtm/objc2 e282618be4c3a3b9542957e0c8540e9588472ce8, root LICENSE.md.
- objc2-uniform-type-identifiers 0.2.2: madsmtm/objc2 e282618be4c3a3b9542957e0c8540e9588472ce8, root LICENSE.md.
- objc2-user-notifications 0.2.2: madsmtm/objc2 e282618be4c3a3b9542957e0c8540e9588472ce8, root LICENSE.md.

malloc_buf 0.0.6 declares MIT/Steven Sheldon; published archive omits license text.
Canonical SPDX MIT supplement follows the documented block approach.
