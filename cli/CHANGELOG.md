# Changelog

All notable changes to this project will be documented in this file.


## [v0.2.0](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe-cli/v0.1.1..rustypipe-cli/v0.2.0) - 2024-08-18

### 🚀 Features

- Overhauled downloader - ([11a0038](https://codeberg.org/ThetaDev/rustypipe/commit/11a00383502917cd98245c3da349107289ba3aa9))
- [**breaking**] Add TV client - ([e608811](https://codeberg.org/ThetaDev/rustypipe/commit/e608811e5f5615416241e67561671330097092cb))
- Downloader: add audio tagging - ([1e1315a](https://codeberg.org/ThetaDev/rustypipe/commit/1e1315a8378bd0ad25b5f1614e83dabc4a0b40d5))
- Downloader: add download_track fn, improve path templates - ([e1e4fb2](https://codeberg.org/ThetaDev/rustypipe/commit/e1e4fb29c190fec07f17c59ec88bef4f1c2a76a1))
- Add audiotag+indicatif features to downloader - ([97fb057](https://codeberg.org/ThetaDev/rustypipe/commit/97fb0578b5c4954a596d8dee0c4b6e1d773a9300))
- Add plaintext output to CLI - ([91b020e](https://codeberg.org/ThetaDev/rustypipe/commit/91b020efd498eff6e0f354a1de39439e252a79dd))
- Add potoken option to downloader - ([904f821](https://codeberg.org/ThetaDev/rustypipe/commit/904f8215d84c810b04e4d2134718e786a4803ad2))
- Print error message - ([8f16e5b](https://codeberg.org/ThetaDev/rustypipe/commit/8f16e5ba6eec3fd6aba1bb6a19571c65fb69ce0e))
- Add list of clients to downloader - ([5e646af](https://codeberg.org/ThetaDev/rustypipe/commit/5e646afd1edc6c0101501311527ea56d3bad5fd2))
- Retry with different client after 403 error - ([d875b54](https://codeberg.org/ThetaDev/rustypipe/commit/d875b5442de9822ba7ddc6f05789f56a8962808c))
- Add option to fetch RSS feed - ([03c4d3c](https://codeberg.org/ThetaDev/rustypipe/commit/03c4d3c392386e06f2673f0e0783e22d10087989))
- [**breaking**] Update channel model, addd handle + video_count, remove tv/mobile banner - ([e671570](https://codeberg.org/ThetaDev/rustypipe/commit/e6715700d950912031d5fbc1263f8770b6ffc49c))

### 🐛 Bug Fixes

- *(deps)* Update quick-xml to v0.35.0 - ([298e4de](https://codeberg.org/ThetaDev/rustypipe/commit/298e4def93d1595fba91be103f014aa645a08937))
- Improve deobfuscator (support multiple nsig name matches, error if mapping all streams fails) - ([8152ce6](https://codeberg.org/ThetaDev/rustypipe/commit/8152ce6b088b57be9b8419b754aca93805e5f34d))
- Cli: print video ID when logging errors - ([2c7a3fb](https://codeberg.org/ThetaDev/rustypipe/commit/2c7a3fb5cc153ff0b8b5e79234ae497d916e471c))
- Use anstream + owo-color for colorful CLI output - ([e8324cf](https://codeberg.org/ThetaDev/rustypipe/commit/e8324cf3b065cb977adbc9529b1ef5ee18c3dd47))
- Use native tls by default for CLI - ([f37432a](https://codeberg.org/ThetaDev/rustypipe/commit/f37432a48c1f93cab5f7942f791daf7b27cb1565))
- Detect ip-ban error message - ([da39c64](https://codeberg.org/ThetaDev/rustypipe/commit/da39c64f302bc2edc4214bbe25a0a9eb54063b09))
- Dont store cache in current dir with --report option - ([6009de7](https://codeberg.org/ThetaDev/rustypipe/commit/6009de7bddc6031f2af17005c473c17934327c02))
- Show docs.rs feature flags - ([67a231d](https://codeberg.org/ThetaDev/rustypipe/commit/67a231d6d1b6427f500667729a59032f2b28cc65))

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rust crate quick-xml to 0.36.0 (#8) - ([b6bc05c](https://codeberg.org/ThetaDev/rustypipe/commit/b6bc05c1f39da9a846b2e3d1d24bcbccb031203b))
- *(deps)* Update rust crate rstest to 0.22.0 (#9) - ([abb7832](https://codeberg.org/ThetaDev/rustypipe/commit/abb783219aba4b492c1dff03c2148acf1f51a55d))
- Change repo URL to Codeberg - ([1793331](https://codeberg.org/ThetaDev/rustypipe/commit/17933315d947f76d5fe1aa52abf7ea24c3ce6381))
- Adjust dependency versions - ([70c6f8c](https://codeberg.org/ThetaDev/rustypipe/commit/70c6f8c3b97baefd316fff90cc727524516657af))

### Todo

- Update metadata - ([8692ca8](https://codeberg.org/ThetaDev/rustypipe/commit/8692ca81d972d0d2acf6fb4da79b9e0f5ebf4daf))


## [v0.1.1](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe-cli/v0.1.0..rustypipe-cli/v0.1.1) - 2024-06-27

### 🚀 Features

- CLI: setting player type - ([16e0e28](https://codeberg.org/ThetaDev/rustypipe/commit/16e0e28c4866bb69d8e4c06eef94176f329a1c27))

### 🐛 Bug Fixes

- Clippy warning - ([8420c2f](https://codeberg.org/ThetaDev/rustypipe/commit/8420c2f8dbd2791b524ceca2e19fb68e5b918bfa))

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

## [v0.1.0](https://codeberg.org/ThetaDev/rustypipe/commits/tag/rustypipe-cli/v0.1.0) - 2024-03-22

Initial release

<!-- generated by git-cliff -->
