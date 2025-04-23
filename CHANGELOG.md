# Changelog

All notable changes to this project will be documented in this file.


## [v0.11.4](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.11.3..rustypipe/v0.11.4) - 2025-04-23

### 🚀 Features

- Player: handle VPN ban and captcha required error messages - ([be6da5e](https://codeberg.org/ThetaDev/rustypipe/commit/be6da5e7e3558ef39773bf45bcb8afbf006bacec))

### 🐛 Bug Fixes

- Deobfuscator: handle 1-char long global variables, find nsig fn (player 6450230e) - ([d675987](https://codeberg.org/ThetaDev/rustypipe/commit/d675987654972c6aa4cc2b291d25bc49fa60173e))


## [v0.11.3](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.11.2..rustypipe/v0.11.3) - 2025-04-03

### 🐛 Bug Fixes

- Deobfuscator: global variable extraction fixed - ([ac44e95](https://codeberg.org/ThetaDev/rustypipe/commit/ac44e95a88d95f9d2d1ec672f86ca9d31d6991b9))
- Deobfuscator: small simplification - ([189ba81](https://codeberg.org/ThetaDev/rustypipe/commit/189ba81a42e6c09f6af4d2768c449c22b864101e))
- Deobfuscator: handle global functions as well - ([939a7ae](https://codeberg.org/ThetaDev/rustypipe/commit/939a7aea61a3eee4c1e67bfbfc835f0ce3934171))
- Handle music playlist/album not found - ([ea80717](https://codeberg.org/ThetaDev/rustypipe/commit/ea80717f692b2c45b5063c362c9fa8ebca5a3471))
- Switch client if no adaptive stream URLs were returned - ([187bf1c](https://codeberg.org/ThetaDev/rustypipe/commit/187bf1c9a0e846bff205e0d71a19c5a1ce7b1943))
- Handle music artist not found - ([daf3d03](https://codeberg.org/ThetaDev/rustypipe/commit/daf3d035be38b59aef1ae205ac91c2bbdda2fe66))

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rust crate rand to 0.9.0 - ([af415dd](https://codeberg.org/ThetaDev/rustypipe/commit/af415ddf8f94f00edb918f271d8e6336503e9faf))


## [v0.11.2](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.11.1..rustypipe/v0.11.2) - 2025-03-24

### 🐛 Bug Fixes

- A/B test 22: commandExecutorCommand for playlist continuations - ([e8acbfb](https://codeberg.org/ThetaDev/rustypipe/commit/e8acbfbbcf5d31b5ac34410ddf334e5534e3762f))
- Extract deobf data with global strings variable - ([4ce6746](https://codeberg.org/ThetaDev/rustypipe/commit/4ce6746be538564e79f7e3c67d7a91aaa53f48ea))
- Handle player returning no adaptive stream URLs - ([07db7b1](https://codeberg.org/ThetaDev/rustypipe/commit/07db7b1166e912e1554f98f2ae20c2c356fed38f))


## [v0.11.1](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.11.0..rustypipe/v0.11.1) - 2025-03-16

### 🐛 Bug Fixes

- Simplify get_player_from_clients logic - ([c04b606](https://codeberg.org/ThetaDev/rustypipe/commit/c04b60604d2628bf8f0e3de453c243adbb966e57))
- Desktop client: generate PO token from user_syncid when authenticated - ([8342cae](https://codeberg.org/ThetaDev/rustypipe/commit/8342caeb0f566a38060a6ec69f3ca65b9a2afcd6))
- Always skip failed clients - ([63a6f50](https://codeberg.org/ThetaDev/rustypipe/commit/63a6f50a8b5ad6bb984282335c1481ae3cd2fe83))

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rust crate rstest to 0.25.0 - ([9ed1306](https://codeberg.org/ThetaDev/rustypipe/commit/9ed1306f3aaeb993c409997ddfbc47499e4f4d22))


## [v0.11.0](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.10.0..rustypipe/v0.11.0) - 2025-02-26

### 🚀 Features

- Add original album track count, fix fetching albums with more than 200 tracks - ([544782f](https://codeberg.org/ThetaDev/rustypipe/commit/544782f8de728cda0aca9a1cb95837cdfbd001f1))

### 🐛 Bug Fixes

- A/B test 21: music album recommendations - ([6737512](https://codeberg.org/ThetaDev/rustypipe/commit/6737512f5f67c8cd05d4552dd0e0f24381035b35))


## [v0.10.0](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.9.0..rustypipe/v0.10.0) - 2025-02-09

### 🚀 Features

- Add visitor data cache, remove random visitor data - ([b12f4c5](https://codeberg.org/ThetaDev/rustypipe/commit/b12f4c5d821a9189d7ed8410ad860824b6d052ef))
- Add support for rustypipe-botguard to get PO tokens - ([b90a252](https://codeberg.org/ThetaDev/rustypipe/commit/b90a252a5e1bf05a5294168b0ec16a73cbb88f42))
- Add session po token cache - ([b72b501](https://codeberg.org/ThetaDev/rustypipe/commit/b72b501b6dbcf4333b24cd80e7c8c61b0c21ec91))
- Check rustypipe-botguard-api version - ([8385b87](https://codeberg.org/ThetaDev/rustypipe/commit/8385b87c63677f32a240679a78702f53072e517a))
- Rewrite request attempt system, retry with different visitor data - ([dfd03ed](https://codeberg.org/ThetaDev/rustypipe/commit/dfd03edfadff2657e9cfbf04e5d313ba409520ac))
- Log failed player fetch attempts with player_from_clients - ([8e35358](https://codeberg.org/ThetaDev/rustypipe/commit/8e35358c8941301f6ebf7646a11ab22711082569))
- Add timezone query option - ([3a2370b](https://codeberg.org/ThetaDev/rustypipe/commit/3a2370b97ca3d0f40d72d66a23295557317d29fb))
- [**breaking**] Add userdata feature for all personal data queries (playback history, subscriptions) - ([65cb424](https://codeberg.org/ThetaDev/rustypipe/commit/65cb4244c6ab547f53d0cb12af802c4189188c86))
- Add RustyPipe::version_botguard fn, detect rustypipe-botguard in current dir, add botguard version to report - ([1d755b7](https://codeberg.org/ThetaDev/rustypipe/commit/1d755b76bf4569f7d0bb90a65494ac8e7aae499a))

### 🐛 Bug Fixes

- Parsing history dates - ([af7dc10](https://codeberg.org/ThetaDev/rustypipe/commit/af7dc1016322a87dd8fec0b739939c2b12b6f400))
- A/V streams incorrectly recognized as video-only - ([2b891ca](https://codeberg.org/ThetaDev/rustypipe/commit/2b891ca0788f91f16dbb9203191cb3d2092ecc74))
- Update iOS client - ([e915416](https://codeberg.org/ThetaDev/rustypipe/commit/e91541629d6c944c1001f5883e3c1264aeeb3969))
- A/B test 20: music continuation item renderer - ([9c67f8f](https://codeberg.org/ThetaDev/rustypipe/commit/9c67f8f85bef8214848dc9d17bff6cff252e015e))
- Include whole request body in report - ([15245c1](https://codeberg.org/ThetaDev/rustypipe/commit/15245c18b584e42523762b94fcc7284d483660a0))
- Extracting nsig fn when outside variable starts with $ - ([eda16e3](https://codeberg.org/ThetaDev/rustypipe/commit/eda16e378730a3b57c4982a626df1622a93c574a))
- Retry updating deobf data after a RustyPipe update - ([50ab1f7](https://codeberg.org/ThetaDev/rustypipe/commit/50ab1f7a5d8aeaa3720264b4a4b27805bb0e8121))
- Allow player data to be fetched without botguard - ([29c854b](https://codeberg.org/ThetaDev/rustypipe/commit/29c854b20d7a6677415b1744e7ba7ecd4f594ea5))
- Output full request body in reports, clean up `get_player_po_token` - ([a0d850f](https://codeberg.org/ThetaDev/rustypipe/commit/a0d850f8e01428a73bbd66397d0dbf797b45958f))
- Correct timezone offset for parsed dates, add timezone_local option - ([a5a7be5](https://codeberg.org/ThetaDev/rustypipe/commit/a5a7be5b4e0a0b73d7e1dc802ebd7bd48dafc76d))
- Use localzone crate to get local tz - ([5acbf0e](https://codeberg.org/ThetaDev/rustypipe/commit/5acbf0e456b1f10707e0a56125d993a8129eee3a))
- Only use cached potokens with min. 10min lifetime - ([0c94267](https://codeberg.org/ThetaDev/rustypipe/commit/0c94267d0371b2b26c7b5c9abfa156d5cde2153e))

### 📚 Documentation

- Add Botguard info to README - ([9957add](https://codeberg.org/ThetaDev/rustypipe/commit/9957add2b5d6391b2c1869d2019fd7dd91b8cd41))

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rust crate rquickjs to 0.9.0 (#33) - ([2c8ac41](https://codeberg.org/ThetaDev/rustypipe/commit/2c8ac410aa535d83f8bcc7181f81914b13bceb77))


## [v0.9.0](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.8.0..rustypipe/v0.9.0) - 2025-01-16

### 🚀 Features

- Add functions to fetch a user's history and subscriptions - ([14e3995](https://codeberg.org/ThetaDev/rustypipe/commit/14e399594f97a1228a8c2991a14dd8745af1beb7))
- Add history item dates, extend timeago parser - ([320a8c2](https://codeberg.org/ThetaDev/rustypipe/commit/320a8c2c24217ad5697f0424c4f994bbbe31f3aa))
- Add session headers when using cookie auth - ([3c95b52](https://codeberg.org/ThetaDev/rustypipe/commit/3c95b52ceaf0df2d67ee0d2f2ac658f666f29836))
- Add cookies.txt parser, add cookie auth + history cmds to CLI - ([cf498e4](https://codeberg.org/ThetaDev/rustypipe/commit/cf498e4a8f9318b0197bc3f0cbaf7043c53adb9d))
- Add method to get saved_playlists - ([27f64fc](https://codeberg.org/ThetaDev/rustypipe/commit/27f64fc412e833d5bd19ad72913aae19358e98b9))
- Extract player DRM data - ([2af4001](https://codeberg.org/ThetaDev/rustypipe/commit/2af4001c75f2ff4f7c891aa59ac22c2c6b7902a2))
- Add Dolby audio codecs (ac-3, ec-3) - ([a7f8c78](https://codeberg.org/ThetaDev/rustypipe/commit/a7f8c789b1a34710274c4630e027ef868397aea2))
- Add DRM and audio channel number filtering to StreamFilter - ([d5abee2](https://codeberg.org/ThetaDev/rustypipe/commit/d5abee275300ab1bc10fc8d6c35a4e3813fd2bd4))
- Set cache file permissions to 600 - ([dee8a99](https://codeberg.org/ThetaDev/rustypipe/commit/dee8a99e7a8d071c987709a01f02ee8fecf2d776))

### 🐛 Bug Fixes

- Dont leak authorization and cookie header in reports - ([75fce91](https://codeberg.org/ThetaDev/rustypipe/commit/75fce91353c02cd498f27d21b08261c23ea03d70))
- Require new time crate version which added Month::length - ([ec7a195](https://codeberg.org/ThetaDev/rustypipe/commit/ec7a195c98f39346c4c8db875212c3843580450e))
- Parsing numbers (it), dates (kn) - ([63f86b6](https://codeberg.org/ThetaDev/rustypipe/commit/63f86b6e186aa1d2dcaf7e9169ccebb2265e5905))
- Accept user-specific playlist ids (LL, WL) - ([97c3f30](https://codeberg.org/ThetaDev/rustypipe/commit/97c3f30d180d3e62b7e19f22d191d7fd7614daca))
- Only use auth-enabled clients for fetching player with auth option enabled - ([2b2b4af](https://codeberg.org/ThetaDev/rustypipe/commit/2b2b4af0b26cdd0d4bf2218d3f527abd88658abf))
- A/B test 19: Music artist album groups reordered - ([5daad1b](https://codeberg.org/ThetaDev/rustypipe/commit/5daad1b700e8dcf1f3e803db1685f08f27794898))
- Switch to rquickjs crate for deobfuscator - ([75c3746](https://codeberg.org/ThetaDev/rustypipe/commit/75c3746890f3428f3314b7b10c9ec816ad275836))
- Player_from_clients method not send/sync - ([9c512c3](https://codeberg.org/ThetaDev/rustypipe/commit/9c512c3c4dbec0fc3b973536733d61ba61125a92))

### 📚 Documentation

- Update README - ([0432477](https://codeberg.org/ThetaDev/rustypipe/commit/0432477451ecd5f64145d65239c721f4e44826c0))
- Fix README - ([11442df](https://codeberg.org/ThetaDev/rustypipe/commit/11442dfd369599396357f5b7a7a4268a7b537f57))

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rust crate rstest to 0.24.0 (#20) - ([ab19034](https://codeberg.org/ThetaDev/rustypipe/commit/ab19034ab19baf090e83eada056559676ffdadce))
- *(deps)* Update rust crate dirs to v6 (#24) - ([6a60425](https://codeberg.org/ThetaDev/rustypipe/commit/6a604252b1af7a9388db5dc170f737069cc31051))
- Update pre-commit hooks - ([7cd9246](https://codeberg.org/ThetaDev/rustypipe/commit/7cd9246260493d7839018cb39a2dfb4dded8b343))


## [v0.8.0](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.7.2..rustypipe/v0.8.0) - 2024-12-20

### 🚀 Features

- Log warning when generating report - ([258f18a](https://codeberg.org/ThetaDev/rustypipe/commit/258f18a99d848ae7e6808beddad054037a3b3799))
- Add auto-dubbed audio tracks, improved StreamFilter - ([1d1ae17](https://codeberg.org/ThetaDev/rustypipe/commit/1d1ae17ffc16724667d43142aa57abda2e6468e4))

### 🐛 Bug Fixes

- Replace deprecated call to `time::util::days_in_year_month` - ([69ef6ae](https://codeberg.org/ThetaDev/rustypipe/commit/69ef6ae51e9b09a9b9c06057e717bf6f054c9803))
- Nsig fn extra variable extraction - ([8014741](https://codeberg.org/ThetaDev/rustypipe/commit/80147413ee3190bb530f8f6b02738bcc787a6444))
- Deobf function extraction, allow $ in variable names - ([8cadbc1](https://codeberg.org/ThetaDev/rustypipe/commit/8cadbc1a4c865d085e30249dba0f353472456a32))
- Remove leading zero-width-space from comments, ensure space after links - ([162959c](https://codeberg.org/ThetaDev/rustypipe/commit/162959ca4513a03496776fae905b4bf20c79899c))
- Update client versions, enable Opus audio with iOS client - ([1b60c97](https://codeberg.org/ThetaDev/rustypipe/commit/1b60c97a183b9d74b92df14b5b113c61aba1be7f))
- Extract transcript from comment voice replies - ([30f60c3](https://codeberg.org/ThetaDev/rustypipe/commit/30f60c30f9d87d39585db93c1c9e274f48d688ba))
- Error 400 when fetching player with login - ([5ce84c4](https://codeberg.org/ThetaDev/rustypipe/commit/5ce84c44a6844f692258066c83e04df875e0aa91))

### ⚙️ Miscellaneous Tasks

- Update user agent - ([53e5846](https://codeberg.org/ThetaDev/rustypipe/commit/53e5846286e8db920622152c2a0a57ddc7c41d25))


## [v0.7.2](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.7.1..rustypipe/v0.7.2) - 2024-12-13

### 🐛 Bug Fixes

- Replace futures dependency with futures-util - ([5c39bf4](https://codeberg.org/ThetaDev/rustypipe/commit/5c39bf4842b13d37a4277ea5506e15c179892ce5))
- Lifetime-related lints - ([c4feff3](https://codeberg.org/ThetaDev/rustypipe/commit/c4feff37a5989097b575c43d89c26427d92d77b9))
- Limit retry attempts to fetch client versions and deobf data - ([44ae456](https://codeberg.org/ThetaDev/rustypipe/commit/44ae456d2c654679837da8ec44932c44b1b01195))
- Deobfuscation function extraction - ([f5437aa](https://codeberg.org/ThetaDev/rustypipe/commit/f5437aa127b2b7c5a08839643e30ea1ec989d30b))


## [v0.7.1](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.7.0..rustypipe/v0.7.1) - 2024-11-25

### 🐛 Bug Fixes

- Disable Android client - ([a846b72](https://codeberg.org/ThetaDev/rustypipe/commit/a846b729e3519e3d5e62bdf028d9b48a7f8ea2ce))
- A/B test 18: music playlist facepile avatar model - ([6c8108c](https://codeberg.org/ThetaDev/rustypipe/commit/6c8108c94acf9ca2336381bdca7c97b24a809521))

### ⚙️ Miscellaneous Tasks

- Add docs badge to README - ([706e881](https://codeberg.org/ThetaDev/rustypipe/commit/706e88134c0e94ce7d880735e9d31b3ff531a4f9))


## [v0.7.0](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.6.0..rustypipe/v0.7.0) - 2024-11-10

### 🚀 Features

- Allow searching for YTM users - ([50010b7](https://codeberg.org/ThetaDev/rustypipe/commit/50010b7b0856d3ce05fe7a9d5989e526089bc2ef))
- [**breaking**] Replace `TrackItem::is_video` attr with TrackType enum; serde lowercase AlbumType enum for consistency - ([044094a](https://codeberg.org/ThetaDev/rustypipe/commit/044094a4b70f05c46a459fa1597e23f4224b7b0b))

### 🐛 Bug Fixes

- Fetch unlocalized player data to interpret errors correctly; regression introduced with v0.6.0 - ([0919cbd](https://codeberg.org/ThetaDev/rustypipe/commit/0919cbd0dfe28ea00610c67a694e5f319e80635f))
- A/B test 17: channel playlists lockupViewModel - ([342119d](https://codeberg.org/ThetaDev/rustypipe/commit/342119dba6f3dc2152eef1fc9841264a9e56b9f0))
- [**breaking**] Serde: lowercase Verification enum - ([badb3ae](https://codeberg.org/ThetaDev/rustypipe/commit/badb3aef8249315909160b8ff73df3019f07cf97))
- Parsing videos using LockupViewModel (Music video recommendations) - ([870ff79](https://codeberg.org/ThetaDev/rustypipe/commit/870ff79ee07dfab1f4f2be3a401cd5320ed587da))
- Parsing lockup playlists with "MIX" instead of view count - ([ac8fbc3](https://codeberg.org/ThetaDev/rustypipe/commit/ac8fbc3e679819189e2791c323975acaf1b43035))

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rust crate thiserror to v2 (#16) - ([e1e1687](https://codeberg.org/ThetaDev/rustypipe/commit/e1e1687605603686ac5fd5deeb6aa8fecaf92494))


## [v0.6.0](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.5.0..rustypipe/v0.6.0) - 2024-10-28

### 🚀 Features

- [**breaking**] Remove TvHtml5Embed client as it got disabled - ([9e835c8](https://codeberg.org/ThetaDev/rustypipe/commit/9e835c8f38a3dd28c65561b2f9bb7a0f530c24f1))
- [**breaking**] Generate random visitorData, remove `RustyPipeQuery::get_context` and `YTContext<'a>` from public API - ([7c4f44d](https://codeberg.org/ThetaDev/rustypipe/commit/7c4f44d09c4d813efff9e7d1059ddacd226b9e9d))
- Add OAuth user login to access age-restricted videos - ([1cc3f9a](https://codeberg.org/ThetaDev/rustypipe/commit/1cc3f9ad74908d33e247ba6243103bfc22540164))
- Add user_auth_logout method - ([9e2fe61](https://codeberg.org/ThetaDev/rustypipe/commit/9e2fe61267846ce216e0c498d8fa9ee672e03cbf))
- Revoke OAuth token when logging out - ([62f8a92](https://codeberg.org/ThetaDev/rustypipe/commit/62f8a9210c23e1f02c711a2294af8766ca6b70e2))

### 🐛 Bug Fixes

- Skip serializing empty cache entries - ([be18d89](https://codeberg.org/ThetaDev/rustypipe/commit/be18d89ea65e35ddcf0f31bea3360e5db209fb9f))
- Fetch artist albums continuation - ([b589061](https://codeberg.org/ThetaDev/rustypipe/commit/b589061a40245637b4fe619a26892291d87d25e6))
- Update channel order tokens - ([79a6281](https://codeberg.org/ThetaDev/rustypipe/commit/79a62816ff62d94e5c706f45b1ce5971e5e58a81))
- Handle auth errors - ([512223f](https://codeberg.org/ThetaDev/rustypipe/commit/512223fd83fb1ba2ba7ad96ed050a70bb7ec294d))
- Use same visitor data for fetching artist album continuations - ([7b0499f](https://codeberg.org/ThetaDev/rustypipe/commit/7b0499f6b7cbf6ac4b83695adadfebb3f30349c7))

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rust crate fancy-regex to 0.14.0 (#14) - ([94194e0](https://codeberg.org/ThetaDev/rustypipe/commit/94194e019c46ca49c343086e80e8eb75c52f4bc6))
- *(deps)* Update rust crate quick-xml to 0.37.0 (#15) - ([0662b5c](https://codeberg.org/ThetaDev/rustypipe/commit/0662b5ccfccc922b28629f11ea52c3eb35f9efd2))


## [v0.5.0](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.4.0..rustypipe/v0.5.0) - 2024-10-13

### 🚀 Features

- Add mobile client - ([71d3ec6](https://codeberg.org/ThetaDev/rustypipe/commit/71d3ec65ddafa966ef6b41cf4eb71687ba4b594c))

### 🐛 Bug Fixes

- Prioritize visitor_data argument before opts - ([ace0fae](https://codeberg.org/ThetaDev/rustypipe/commit/ace0fae1005217cd396000176e7c01682eae026f))
- Ignore live tracks in YTM searches - ([f3f2e1d](https://codeberg.org/ThetaDev/rustypipe/commit/f3f2e1d3ca1e9c838c682356bb5a7ded6951c8e5))
- A/B test 16 (pageHeaderRenderer on playlist pages) - ([e65f145](https://codeberg.org/ThetaDev/rustypipe/commit/e65f14556f3003fa59fee3f9f1410fb5ddf63219))

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rust crate rstest to 0.23.0 (#12) - ([96776e9](https://codeberg.org/ThetaDev/rustypipe/commit/96776e98d76fa1d31d5f84dbceafbe8f9dfd9085))


## [v0.4.0](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.3.0..rustypipe/v0.4.0) - 2024-09-10

### 🚀 Features

- Add RustyPipe version constant - ([7a019f5](https://codeberg.org/ThetaDev/rustypipe/commit/7a019f5706e19f7fe9f2e16e3b94d7b98cc8aca9))

### 🐛 Bug Fixes

- Show docs.rs feature flags - ([67a231d](https://codeberg.org/ThetaDev/rustypipe/commit/67a231d6d1b6427f500667729a59032f2b28cc65))
- A/B test 15 (parsing channel shortsLockupViewModel) - ([7972df0](https://codeberg.org/ThetaDev/rustypipe/commit/7972df0df498edd7801e25037b9b2456367f9204))

### 📚 Documentation

- Fix license badge URL - ([4a253e1](https://codeberg.org/ThetaDev/rustypipe/commit/4a253e1a47317e9999e6ad31ac5c411956a0986a))

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rust crate tokio to 1.20.4 [security] (#10) - ([a445e51](https://codeberg.org/ThetaDev/rustypipe/commit/a445e51b54a9afc44cd9657260a0b3d2abddbfa6))


## [v0.3.0](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.2.1..rustypipe/v0.3.0) - 2024-08-18

### 🚀 Features

- Add client_type to VideoPlayer, simplify MapResponse trait - ([90540c6](https://codeberg.org/ThetaDev/rustypipe/commit/90540c6aaad658d4ce24ed41450d8509bac711bd))
- Add http_client method to RustyPipe and user_agent method to RustyPipeQuery - ([3d6de53](https://codeberg.org/ThetaDev/rustypipe/commit/3d6de5354599ea691351e0ca161154e53f2e0b41))
- Add channel_id and channel_name getters to YtEntity trait - ([bbbe9b4](https://codeberg.org/ThetaDev/rustypipe/commit/bbbe9b4b322c6b5b30764772e282c6823aeea524))
- [**breaking**] Make StreamFilter use Vec internally, remove lifetime - ([821984b](https://codeberg.org/ThetaDev/rustypipe/commit/821984bbd51d65cf96b1d14087417ef968eaf9b2))
- Overhauled downloader - ([11a0038](https://codeberg.org/ThetaDev/rustypipe/commit/11a00383502917cd98245c3da349107289ba3aa9))
- Add player_from_clients function to specify client order - ([72b5dfe](https://codeberg.org/ThetaDev/rustypipe/commit/72b5dfec69ec25445b94cb0976662416a5df56ef))
- [**breaking**] Add TV client - ([e608811](https://codeberg.org/ThetaDev/rustypipe/commit/e608811e5f5615416241e67561671330097092cb))
- Downloader: add audio tagging - ([1e1315a](https://codeberg.org/ThetaDev/rustypipe/commit/1e1315a8378bd0ad25b5f1614e83dabc4a0b40d5))
- Add audiotag+indicatif features to downloader - ([97fb057](https://codeberg.org/ThetaDev/rustypipe/commit/97fb0578b5c4954a596d8dee0c4b6e1d773a9300))
- Add YtEntity trait to YouTubeItem and MusicItem - ([114a86a](https://codeberg.org/ThetaDev/rustypipe/commit/114a86a3823a175875aa2aeb31a61a6799ef13bc))
- Change default player client order - ([97904d7](https://codeberg.org/ThetaDev/rustypipe/commit/97904d77374c2c937a49dc7905759c2d8e8ef9ae))
- [**breaking**] Update channel model, addd handle + video_count, remove tv/mobile banner - ([e671570](https://codeberg.org/ThetaDev/rustypipe/commit/e6715700d950912031d5fbc1263f8770b6ffc49c))
- [**breaking**] Add handle to ChannelItem, remove video_count - ([1cffb27](https://codeberg.org/ThetaDev/rustypipe/commit/1cffb27cc0b64929f9627f5839df2d73b81988a4))
- [**breaking**] Remove startpage - ([3599aca](https://codeberg.org/ThetaDev/rustypipe/commit/3599acafef1a21fa6f8dea97902eb4a3fb048c14))

### 🐛 Bug Fixes

- [**breaking**] Extracting nsig function, remove field `throttled` from Video/Audio stream model - ([dd0565b](https://codeberg.org/ThetaDev/rustypipe/commit/dd0565ba98acb3289ed220fd2a3aaf86bb8b0788))
- Make nsig_fn regex more generic - ([fb7af3b](https://codeberg.org/ThetaDev/rustypipe/commit/fb7af3b96698b452b6b24d1e094ba13a245cb83c))
- Improve deobfuscator (support multiple nsig name matches, error if mapping all streams fails) - ([8152ce6](https://codeberg.org/ThetaDev/rustypipe/commit/8152ce6b088b57be9b8419b754aca93805e5f34d))
- Nsig fn extraction - ([3c83e11](https://codeberg.org/ThetaDev/rustypipe/commit/3c83e11e753f8eb6efea5d453a7c819c487b3464))
- Add var to deobf fn assignment - ([c6bd03f](https://codeberg.org/ThetaDev/rustypipe/commit/c6bd03fb70871ae1b764be18f88e86e71818fc56))
- Make Verification enum exhaustive - ([d053ac3](https://codeberg.org/ThetaDev/rustypipe/commit/d053ac3eba810a7241df91f2f50bcbe1fd968c86))
- Extraction error message - ([d36ba59](https://codeberg.org/ThetaDev/rustypipe/commit/d36ba595dab0bbaef1012ebfa8930fc0e6bf8167))
- Set tracing instrumentation level to Error - ([9da3b25](https://codeberg.org/ThetaDev/rustypipe/commit/9da3b25be2b2577f7bd0282c09d10d368ac8b73f))
- Detect ip-ban error message - ([da39c64](https://codeberg.org/ThetaDev/rustypipe/commit/da39c64f302bc2edc4214bbe25a0a9eb54063b09))
- Player_from_clients: fall back to TvHtml5Embed client - ([d0ae796](https://codeberg.org/ThetaDev/rustypipe/commit/d0ae7961ba91d56c8b9a8d1c545875e869b818f5))
- Parsing channels without banner - ([5a6b2c3](https://codeberg.org/ThetaDev/rustypipe/commit/5a6b2c3a621f6b20c1324ea8b9c03426e3d8018b))
- Get TV client version - ([ee3ae40](https://codeberg.org/ThetaDev/rustypipe/commit/ee3ae40395263c5989784c7e00038ff13bc1151a))

### ⚙️ Miscellaneous Tasks

- Renovate: disable approveMajorUpdates - ([4743f9d](https://codeberg.org/ThetaDev/rustypipe/commit/4743f9d8e101b58ad6a43548495da9f4f381b9f4))
- Renovate: disable scheduleDaily - ([015bd6f](https://codeberg.org/ThetaDev/rustypipe/commit/015bd6fcbf04163565fcb190b163ecfdb5664e11))
- Renovate: enable automerge - ([882abc5](https://codeberg.org/ThetaDev/rustypipe/commit/882abc53ca894229ee78ec0edaa723d9ea61bbcb))
- *(deps)* Update rust crate quick-xml to 0.36.0 (#8) - ([b6bc05c](https://codeberg.org/ThetaDev/rustypipe/commit/b6bc05c1f39da9a846b2e3d1d24bcbccb031203b))
- *(deps)* Update rust crate rstest to 0.22.0 (#9) - ([abb7832](https://codeberg.org/ThetaDev/rustypipe/commit/abb783219aba4b492c1dff03c2148acf1f51a55d))
- Change repo URL to Codeberg - ([1793331](https://codeberg.org/ThetaDev/rustypipe/commit/17933315d947f76d5fe1aa52abf7ea24c3ce6381))
- Adjust dependency versions - ([70c6f8c](https://codeberg.org/ThetaDev/rustypipe/commit/70c6f8c3b97baefd316fff90cc727524516657af))

### Todo

- Update metadata - ([8692ca8](https://codeberg.org/ThetaDev/rustypipe/commit/8692ca81d972d0d2acf6fb4da79b9e0f5ebf4daf))


## [v0.2.1](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.2.0..rustypipe/v0.2.1) - 2024-07-01

### 🐛 Bug Fixes

- *(deps)* Update quick-xml to v0.35.0 - ([298e4de](https://codeberg.org/ThetaDev/rustypipe/commit/298e4def93d1595fba91be103f014aa645a08937))


## [v0.2.0](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.1.3..rustypipe/v0.2.0) - 2024-06-27

### 🚀 Features

- Add text formatting (bold/italic/strikethrough) - ([b8825f9](https://codeberg.org/ThetaDev/rustypipe/commit/b8825f9199365c873a4f0edd98a435e986b8daa2))
- Prefix chip-style web links (social media) with the service name - ([6c41ef2](https://codeberg.org/ThetaDev/rustypipe/commit/6c41ef2fb2531e10a12c271e2d48504510a3b0bf))
- Make get_visitor_data() public - ([da1d1bd](https://codeberg.org/ThetaDev/rustypipe/commit/da1d1bd2a0b214da10436ae221c90a0f88697b9a))
- Add UnavailabilityReason: IpBan - ([401d4e8](https://codeberg.org/ThetaDev/rustypipe/commit/401d4e8255b1e86444319fed6d114dfbd0f80bbd))
- Add YtEntity trait - ([792e3b3](https://codeberg.org/ThetaDev/rustypipe/commit/792e3b31e0101087a167935baad39a2e3b4296d0))

### 🐛 Bug Fixes

- Remove Innertube API keys, update android player params - ([a8fb337](https://codeberg.org/ThetaDev/rustypipe/commit/a8fb337fae9cb0112e0152f9a0a19ebae49c2a4d))
- Parsing error when no `music_related` content available - ([8fbd6b9](https://codeberg.org/ThetaDev/rustypipe/commit/8fbd6b95b6f01108b46f53fe60a56b0c561e40c1))
- Parsing audiobook type in European Portuguese - ([041ce2d](https://codeberg.org/ThetaDev/rustypipe/commit/041ce2d08f6021c88e8890034f551f7e01b2f012))
- Renovate ci token - ([e0759eb](https://codeberg.org/ThetaDev/rustypipe/commit/e0759ebce32a5520245bb2c0cb920734b04ee7dc))

### 🚜 Refactor

- [**breaking**] Rename VideoItem/VideoPlayerDetails.length to duration for consistency - ([94e8d24](https://codeberg.org/ThetaDev/rustypipe/commit/94e8d24c6848b8bfca70dd03a7d89547ba9d6051))

### 📚 Documentation

- Add logo - ([6646078](https://codeberg.org/ThetaDev/rustypipe/commit/66460789449be0d5984cbdb6ec372e69323b7a88))

### ⚙️ Miscellaneous Tasks

- Changelog: fix incorrect version URLs - ([97b6f07](https://codeberg.org/ThetaDev/rustypipe/commit/97b6f07399e80e00a6c015d013e744568be125dd))
- Update rstest to v0.19.0 - ([50fd1f0](https://codeberg.org/ThetaDev/rustypipe/commit/50fd1f08caf39c1298654e06059cc393543e925b))
- Introduce MSRV - ([5dbb288](https://codeberg.org/ThetaDev/rustypipe/commit/5dbb288a496d53a299effa2026f5258af7b1f176))
- Fix clippy lints - ([45b9f2a](https://codeberg.org/ThetaDev/rustypipe/commit/45b9f2a627b4e7075ba0b1c5f16efcc19aef7922))
- Vscode: enable rss feature by default - ([e75ffbb](https://codeberg.org/ThetaDev/rustypipe/commit/e75ffbb5da6198086385ea96383ab9d0791592a5))
- Configure Renovate (#3) - ([44c2deb](https://codeberg.org/ThetaDev/rustypipe/commit/44c2debea61f70c24ad6d827987e85e2132ed3d1))
- *(deps)* Update rust crate tokio to 1.20.4 [security] (#4) - ([ce3ec34](https://codeberg.org/ThetaDev/rustypipe/commit/ce3ec34337b8acac41410ea39264aab7423d5801))
- *(deps)* Update rust crate quick-xml to 0.34.0 (#5) - ([1e8a1af](https://codeberg.org/ThetaDev/rustypipe/commit/1e8a1af08c873cee7feadf63c2eff62753a78f64))
- *(deps)* Update rust crate rstest to 0.21.0 (#7) - ([c3af918](https://codeberg.org/ThetaDev/rustypipe/commit/c3af918ba53c6230c0e4aef822a0cb2cf120bf3f))

## [v0.1.3](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.1.2..rustypipe/v0.1.3) - 2024-04-01

### 🐛 Bug Fixes

- Parse new comment model (A/B#14 frameworkUpdates) - ([b0331f7](https://codeberg.org/ThetaDev/rustypipe/commit/b0331f7250f5d7d61a45209150739d2cb08b4280))

### ◀️ Revert

- "fix: improve VecLogErr messages" (leads to infinite loop) - ([348c852](https://codeberg.org/ThetaDev/rustypipe/commit/348c8523fe847f2f6ce98317375a7ab65e778ed2))


## [v0.1.2](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.1.1..rustypipe/v0.1.2) - 2024-03-26

### 🐛 Bug Fixes

- Correctly parse subscriber count with new channel header - ([180dd98](https://codeberg.org/ThetaDev/rustypipe/commit/180dd9891a14b4da9f130a73d73aecc3822fce2f))


## [v0.1.1](https://codeberg.org/ThetaDev/rustypipe/compare/rustypipe/v0.1.0..rustypipe/v0.1.1) - 2024-03-26

### 🐛 Bug Fixes

- Specify internal dependency versions - ([6598a23](https://codeberg.org/ThetaDev/rustypipe/commit/6598a23d0699e6fe298275a67e0146a19c422c88))
- Move package attributes to workspace - ([e4b204e](https://codeberg.org/ThetaDev/rustypipe/commit/e4b204eae65f450471be0890b0198d2f30714b3b))
- Parsing music details with video description tab - ([a81c3e8](https://codeberg.org/ThetaDev/rustypipe/commit/a81c3e83366fdf72d01dd3ee00fb2e831f7aaa26))

### ⚙️ Miscellaneous Tasks

- Changes to release command - ([0bcced1](https://codeberg.org/ThetaDev/rustypipe/commit/0bcced1db377198a54c9c7d03b8d038125a2bfe4))
- Update user agent (FF 115.0) - ([be314d5](https://codeberg.org/ThetaDev/rustypipe/commit/be314d57ea1d99bfdc80649351ee3e7845541238))
- Fix release script (unquoted include paths) - ([78ba9cb](https://codeberg.org/ThetaDev/rustypipe/commit/78ba9cb34c6bba3aba177583b242d3f76ea9847d))


## [v0.1.0](https://codeberg.org/ThetaDev/rustypipe/commits/tag/rustypipe/v0.1.0) - 2024-03-22

Initial release

<!-- generated by git-cliff -->
