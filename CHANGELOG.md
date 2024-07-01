# Changelog

All notable changes to this project will be documented in this file.


## [v0.2.1](https://code.thetadev.de/ThetaDev/rustypipe/compare/rustypipe/v0.2.0..rustypipe/v0.2.1) - 2024-07-01

### 🐛 Bug Fixes

- *(deps)* Update quick-xml to v0.35.0 - ([298e4de](https://code.thetadev.de/ThetaDev/rustypipe/commit/298e4def93d1595fba91be103f014aa645a08937))


## [v0.2.0](https://code.thetadev.de/ThetaDev/rustypipe/compare/rustypipe/v0.1.3..rustypipe/v0.2.0) - 2024-06-27

### 🚀 Features

- Add text formatting (bold/italic/strikethrough) - ([b8825f9](https://code.thetadev.de/ThetaDev/rustypipe/commit/b8825f9199365c873a4f0edd98a435e986b8daa2))
- Prefix chip-style web links (social media) with the service name - ([6c41ef2](https://code.thetadev.de/ThetaDev/rustypipe/commit/6c41ef2fb2531e10a12c271e2d48504510a3b0bf))
- Make get_visitor_data() public - ([da1d1bd](https://code.thetadev.de/ThetaDev/rustypipe/commit/da1d1bd2a0b214da10436ae221c90a0f88697b9a))
- Add UnavailabilityReason: IpBan - ([401d4e8](https://code.thetadev.de/ThetaDev/rustypipe/commit/401d4e8255b1e86444319fed6d114dfbd0f80bbd))
- Add YtEntity trait - ([792e3b3](https://code.thetadev.de/ThetaDev/rustypipe/commit/792e3b31e0101087a167935baad39a2e3b4296d0))

### 🐛 Bug Fixes

- Remove Innertube API keys, update android player params - ([a8fb337](https://code.thetadev.de/ThetaDev/rustypipe/commit/a8fb337fae9cb0112e0152f9a0a19ebae49c2a4d))
- Parsing error when no `music_related` content available - ([8fbd6b9](https://code.thetadev.de/ThetaDev/rustypipe/commit/8fbd6b95b6f01108b46f53fe60a56b0c561e40c1))
- Parsing audiobook type in European Portuguese - ([041ce2d](https://code.thetadev.de/ThetaDev/rustypipe/commit/041ce2d08f6021c88e8890034f551f7e01b2f012))
- Renovate ci token - ([e0759eb](https://code.thetadev.de/ThetaDev/rustypipe/commit/e0759ebce32a5520245bb2c0cb920734b04ee7dc))

### 🚜 Refactor

- [**breaking**] Rename VideoItem/VideoPlayerDetails.length to duration for consistency - ([94e8d24](https://code.thetadev.de/ThetaDev/rustypipe/commit/94e8d24c6848b8bfca70dd03a7d89547ba9d6051))

### 📚 Documentation

- Add logo - ([6646078](https://code.thetadev.de/ThetaDev/rustypipe/commit/66460789449be0d5984cbdb6ec372e69323b7a88))

### ⚙️ Miscellaneous Tasks

- Changelog: fix incorrect version URLs - ([97b6f07](https://code.thetadev.de/ThetaDev/rustypipe/commit/97b6f07399e80e00a6c015d013e744568be125dd))
- Update rstest to v0.19.0 - ([50fd1f0](https://code.thetadev.de/ThetaDev/rustypipe/commit/50fd1f08caf39c1298654e06059cc393543e925b))
- Introduce MSRV - ([5dbb288](https://code.thetadev.de/ThetaDev/rustypipe/commit/5dbb288a496d53a299effa2026f5258af7b1f176))
- Fix clippy lints - ([45b9f2a](https://code.thetadev.de/ThetaDev/rustypipe/commit/45b9f2a627b4e7075ba0b1c5f16efcc19aef7922))
- Vscode: enable rss feature by default - ([e75ffbb](https://code.thetadev.de/ThetaDev/rustypipe/commit/e75ffbb5da6198086385ea96383ab9d0791592a5))
- Configure Renovate (#3) - ([44c2deb](https://code.thetadev.de/ThetaDev/rustypipe/commit/44c2debea61f70c24ad6d827987e85e2132ed3d1))
- *(deps)* Update rust crate tokio to 1.20.4 [security] (#4) - ([ce3ec34](https://code.thetadev.de/ThetaDev/rustypipe/commit/ce3ec34337b8acac41410ea39264aab7423d5801))
- *(deps)* Update rust crate quick-xml to 0.34.0 (#5) - ([1e8a1af](https://code.thetadev.de/ThetaDev/rustypipe/commit/1e8a1af08c873cee7feadf63c2eff62753a78f64))
- *(deps)* Update rust crate rstest to 0.21.0 (#7) - ([c3af918](https://code.thetadev.de/ThetaDev/rustypipe/commit/c3af918ba53c6230c0e4aef822a0cb2cf120bf3f))

## [v0.1.3](https://code.thetadev.de/ThetaDev/rustypipe/compare/rustypipe/v0.1.2..rustypipe/v0.1.3) - 2024-04-01

### 🐛 Bug Fixes

- Parse new comment model (A/B#14 frameworkUpdates) - ([b0331f7](https://code.thetadev.de/ThetaDev/rustypipe/commit/b0331f7250f5d7d61a45209150739d2cb08b4280))

### ◀️ Revert

- "fix: improve VecLogErr messages" (leads to infinite loop) - ([348c852](https://code.thetadev.de/ThetaDev/rustypipe/commit/348c8523fe847f2f6ce98317375a7ab65e778ed2))


## [v0.1.2](https://code.thetadev.de/ThetaDev/rustypipe/compare/rustypipe/v0.1.1..rustypipe/v0.1.2) - 2024-03-26

### 🐛 Bug Fixes

- Correctly parse subscriber count with new channel header - ([180dd98](https://code.thetadev.de/ThetaDev/rustypipe/commit/180dd9891a14b4da9f130a73d73aecc3822fce2f))


## [v0.1.1](https://code.thetadev.de/ThetaDev/rustypipe/compare/rustypipe/v0.1.0..rustypipe/v0.1.1) - 2024-03-26

### 🐛 Bug Fixes

- Specify internal dependency versions - ([6598a23](https://code.thetadev.de/ThetaDev/rustypipe/commit/6598a23d0699e6fe298275a67e0146a19c422c88))
- Move package attributes to workspace - ([e4b204e](https://code.thetadev.de/ThetaDev/rustypipe/commit/e4b204eae65f450471be0890b0198d2f30714b3b))
- Parsing music details with video description tab - ([a81c3e8](https://code.thetadev.de/ThetaDev/rustypipe/commit/a81c3e83366fdf72d01dd3ee00fb2e831f7aaa26))

### ⚙️ Miscellaneous Tasks

- Changes to release command - ([0bcced1](https://code.thetadev.de/ThetaDev/rustypipe/commit/0bcced1db377198a54c9c7d03b8d038125a2bfe4))
- Update user agent (FF 115.0) - ([be314d5](https://code.thetadev.de/ThetaDev/rustypipe/commit/be314d57ea1d99bfdc80649351ee3e7845541238))
- Fix release script (unquoted include paths) - ([78ba9cb](https://code.thetadev.de/ThetaDev/rustypipe/commit/78ba9cb34c6bba3aba177583b242d3f76ea9847d))


## [v0.1.0](https://code.thetadev.de/ThetaDev/rustypipe/commits/tag/rustypipe/v0.1.0) - 2024-03-22

Initial release

<!-- generated by git-cliff -->
