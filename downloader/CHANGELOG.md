# Changelog

All notable changes to this project will be documented in this file.


## [v0.3.0](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe-downloader/v0.2.7..rustypipe-downloader/v0.3.0) - 2025-02-09

### 🚀 Features

- [**breaking**] Remove manual PO token options from downloader in favor of rustypipe-botguard - ([cddb32f](https://codeberg.org/ThetaDev/rustypipe/commit/cddb32f190276265258c6ab45b3d43a8891c4b39))

### 🐛 Bug Fixes

- Ensure downloader futures are send - ([812ff4c](https://codeberg.org/ThetaDev/rustypipe/commit/812ff4c5bafffc5708a6d5066f1ebadb6d9fc958))
- Download audio with dolby codec - ([9234005](https://codeberg.org/ThetaDev/rustypipe/commit/92340056f868007beccb64e9e26eb39abc40f7aa))

### 🚜 Refactor

- [**breaking**] Add client_type field to DownloadError, rename cli option po-token-cache to pot-cache - ([594e675](https://codeberg.org/ThetaDev/rustypipe/commit/594e675b39efc5fbcdbd5e920a4d2cdee64f718e))

### 📚 Documentation

- Add Botguard info to README - ([9957add](https://codeberg.org/ThetaDev/rustypipe/commit/9957add2b5d6391b2c1869d2019fd7dd91b8cd41))

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rustypipe to 0.10.0


## [v0.2.7](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe-downloader/v0.2.6..rustypipe-downloader/v0.2.7) - 2025-01-16

### 🚀 Features

- Extract player DRM data - ([2af4001](https://codeberg.org/ThetaDev/rustypipe/commit/2af4001c75f2ff4f7c891aa59ac22c2c6b7902a2))
- Prefer maxresdefault.jpg thumbnail if available - ([a8e97f4](https://codeberg.org/ThetaDev/rustypipe/commit/a8e97f411a1e769e52d8cbde11f0a4ca1535f7ef))
- Add DRM and audio channel number filtering to StreamFilter - ([d5abee2](https://codeberg.org/ThetaDev/rustypipe/commit/d5abee275300ab1bc10fc8d6c35a4e3813fd2bd4))

### 🐛 Bug Fixes

- Remove Unix file metadata usage (Windows compatibility) - ([5c6d992](https://codeberg.org/ThetaDev/rustypipe/commit/5c6d992939f55a203ac1784f1e9175ac1d498ce8))

### 📚 Documentation

- Update README - ([0432477](https://codeberg.org/ThetaDev/rustypipe/commit/0432477451ecd5f64145d65239c721f4e44826c0))
- Fix README - ([11442df](https://codeberg.org/ThetaDev/rustypipe/commit/11442dfd369599396357f5b7a7a4268a7b537f57))

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rustypipe to 0.9.0
- *(deps)* Update rust crate rstest to 0.24.0 (#20) - ([ab19034](https://codeberg.org/ThetaDev/rustypipe/commit/ab19034ab19baf090e83eada056559676ffdadce))
- *(deps)* Update rust crate lofty to 0.22.0 - ([addeb82](https://codeberg.org/ThetaDev/rustypipe/commit/addeb821101aa968b95455604bc13bd24f50328f))
- *(deps)* Update rust crate dirs to v6 (#24) - ([6a60425](https://codeberg.org/ThetaDev/rustypipe/commit/6a604252b1af7a9388db5dc170f737069cc31051))


## [v0.2.6](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe-downloader/v0.2.5..rustypipe-downloader/v0.2.6) - 2024-12-20

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rustypipe to 0.8.0


## [v0.2.5](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe-downloader/v0.2.4..rustypipe-downloader/v0.2.5) - 2024-12-13

### 🐛 Bug Fixes

- Replace futures dependency with futures-util - ([5c39bf4](https://codeberg.org/ThetaDev/rustypipe/commit/5c39bf4842b13d37a4277ea5506e15c179892ce5))
- Remove empty tempfile after unsuccessful download - ([5262bec](https://codeberg.org/ThetaDev/rustypipe/commit/5262becca1e9e3e8262833764ef18c23bc401172))

### ⚙️ Miscellaneous Tasks

- Add docs badge to README - ([706e881](https://codeberg.org/ThetaDev/rustypipe/commit/706e88134c0e94ce7d880735e9d31b3ff531a4f9))


## [v0.2.4](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe-downloader/v0.2.3..rustypipe-downloader/v0.2.4) - 2024-11-10

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rust crate thiserror to v2 (#16) - ([e1e1687](https://codeberg.org/ThetaDev/rustypipe/commit/e1e1687605603686ac5fd5deeb6aa8fecaf92494))
- *(deps)* Update rustypipe to 0.7.0


## [v0.2.3](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe-downloader/v0.2.2..rustypipe-downloader/v0.2.3) - 2024-10-28

### 🐛 Bug Fixes

- Remove unnecessary image.rs dependencies - ([1b08166](https://codeberg.org/ThetaDev/rustypipe/commit/1b08166399cccb8394d2fdd82d54162c1a9e01be))

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rustypipe to 0.6.0


## [v0.2.2](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe-downloader/v0.2.1..rustypipe-downloader/v0.2.2) - 2024-10-13

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rust crate rstest to 0.23.0 (#12) - ([96776e9](https://codeberg.org/ThetaDev/rustypipe/commit/96776e98d76fa1d31d5f84dbceafbe8f9dfd9085))
- *(deps)* Update rustypipe to 0.5.0


## [v0.2.1](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe-downloader/v0.2.0..rustypipe-downloader/v0.2.1) - 2024-09-10

### 📚 Documentation

- Fix license badge URL - ([4a253e1](https://codeberg.org/ThetaDev/rustypipe/commit/4a253e1a47317e9999e6ad31ac5c411956a0986a))

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rust crate tokio to 1.20.4 [security] (#10) - ([a445e51](https://codeberg.org/ThetaDev/rustypipe/commit/a445e51b54a9afc44cd9657260a0b3d2abddbfa6))


## [v0.2.0](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe-downloader/v0.1.1..rustypipe-downloader/v0.2.0) - 2024-08-18

### 🚀 Features

- Overhauled downloader - ([11a0038](https://codeberg.org/ThetaDev/rustypipe/commit/11a00383502917cd98245c3da349107289ba3aa9))
- [**breaking**] Add TV client - ([e608811](https://codeberg.org/ThetaDev/rustypipe/commit/e608811e5f5615416241e67561671330097092cb))
- Downloader: add audio tagging - ([1e1315a](https://codeberg.org/ThetaDev/rustypipe/commit/1e1315a8378bd0ad25b5f1614e83dabc4a0b40d5))
- Downloader: add download_track fn, improve path templates - ([e1e4fb2](https://codeberg.org/ThetaDev/rustypipe/commit/e1e4fb29c190fec07f17c59ec88bef4f1c2a76a1))
- Add audiotag+indicatif features to downloader - ([97fb057](https://codeberg.org/ThetaDev/rustypipe/commit/97fb0578b5c4954a596d8dee0c4b6e1d773a9300))
- Add plaintext output to CLI - ([91b020e](https://codeberg.org/ThetaDev/rustypipe/commit/91b020efd498eff6e0f354a1de39439e252a79dd))
- Add potoken option to downloader - ([904f821](https://codeberg.org/ThetaDev/rustypipe/commit/904f8215d84c810b04e4d2134718e786a4803ad2))
- Add list of clients to downloader - ([5e646af](https://codeberg.org/ThetaDev/rustypipe/commit/5e646afd1edc6c0101501311527ea56d3bad5fd2))
- Retry with different client after 403 error - ([d875b54](https://codeberg.org/ThetaDev/rustypipe/commit/d875b5442de9822ba7ddc6f05789f56a8962808c))
- [**breaking**] Update channel model, addd handle + video_count, remove tv/mobile banner - ([e671570](https://codeberg.org/ThetaDev/rustypipe/commit/e6715700d950912031d5fbc1263f8770b6ffc49c))

### 🐛 Bug Fixes

- *(deps)* Update quick-xml to v0.35.0 - ([298e4de](https://codeberg.org/ThetaDev/rustypipe/commit/298e4def93d1595fba91be103f014aa645a08937))
- Improve deobfuscator (support multiple nsig name matches, error if mapping all streams fails) - ([8152ce6](https://codeberg.org/ThetaDev/rustypipe/commit/8152ce6b088b57be9b8419b754aca93805e5f34d))
- Set tracing instrumentation level to Error - ([9da3b25](https://codeberg.org/ThetaDev/rustypipe/commit/9da3b25be2b2577f7bd0282c09d10d368ac8b73f))
- Add docs.rs feature attributes - ([ec13cbb](https://codeberg.org/ThetaDev/rustypipe/commit/ec13cbb1f35081118dda0f7f35e3ef90f7ca79a8))
- Show docs.rs feature flags - ([67a231d](https://codeberg.org/ThetaDev/rustypipe/commit/67a231d6d1b6427f500667729a59032f2b28cc65))

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rust crate quick-xml to 0.36.0 (#8) - ([b6bc05c](https://codeberg.org/ThetaDev/rustypipe/commit/b6bc05c1f39da9a846b2e3d1d24bcbccb031203b))
- *(deps)* Update rust crate rstest to 0.22.0 (#9) - ([abb7832](https://codeberg.org/ThetaDev/rustypipe/commit/abb783219aba4b492c1dff03c2148acf1f51a55d))
- Change repo URL to Codeberg - ([1793331](https://codeberg.org/ThetaDev/rustypipe/commit/17933315d947f76d5fe1aa52abf7ea24c3ce6381))
- Adjust dependency versions - ([70c6f8c](https://codeberg.org/ThetaDev/rustypipe/commit/70c6f8c3b97baefd316fff90cc727524516657af))

### Todo

- Update metadata - ([8692ca8](https://codeberg.org/ThetaDev/rustypipe/commit/8692ca81d972d0d2acf6fb4da79b9e0f5ebf4daf))


## [v0.1.1](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe-downloader/v0.1.0..rustypipe-downloader/v0.1.1) - 2024-06-27

### 📚 Documentation

- Add logo - ([6646078](https://codeberg.org/ThetaDev/rustypipe/commit/66460789449be0d5984cbdb6ec372e69323b7a88))

### ⚙️ Miscellaneous Tasks

- Changelog: fix incorrect version URLs - ([97b6f07](https://codeberg.org/ThetaDev/rustypipe/commit/97b6f07399e80e00a6c015d013e744568be125dd))
- Update rstest to v0.19.0 - ([50fd1f0](https://codeberg.org/ThetaDev/rustypipe/commit/50fd1f08caf39c1298654e06059cc393543e925b))
- Introduce MSRV - ([5dbb288](https://codeberg.org/ThetaDev/rustypipe/commit/5dbb288a496d53a299effa2026f5258af7b1f176))
- Fix clippy lints - ([45b9f2a](https://codeberg.org/ThetaDev/rustypipe/commit/45b9f2a627b4e7075ba0b1c5f16efcc19aef7922))
- *(deps)* Update rust crate tokio to 1.20.4 [security] (#4) - ([ce3ec34](https://codeberg.org/ThetaDev/rustypipe/commit/ce3ec34337b8acac41410ea39264aab7423d5801))
- *(deps)* Update rust crate quick-xml to 0.34.0 (#5) - ([1e8a1af](https://codeberg.org/ThetaDev/rustypipe/commit/1e8a1af08c873cee7feadf63c2eff62753a78f64))
- *(deps)* Update rust crate rstest to 0.21.0 (#7) - ([c3af918](https://codeberg.org/ThetaDev/rustypipe/commit/c3af918ba53c6230c0e4aef822a0cb2cf120bf3f))
- Update rustypipe to 0.2.0

## [v0.1.0](https://codeberg.org/ThetaDev/rustypipe/commits/tag/rustypipe-downloader/v0.1.0) - 2024-03-22

Initial release

<!-- generated by git-cliff -->
