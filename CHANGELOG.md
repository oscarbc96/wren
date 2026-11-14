# Changelog

## [0.2.0](https://github.com/oscarbc96/wren/compare/wren-v0.1.0...wren-v0.2.0) (2026-11-14)


### Features

* **adblock_rules:** block ads and trackers ([#30](https://github.com/oscarbc96/wren/issues/30)) ([b22a10f](https://github.com/oscarbc96/wren/commit/b22a10fbfec0bc973eec2845eabc1e6c93243419))
* cdp server behind --remote-debugging-port ([#44](https://github.com/oscarbc96/wren/issues/44)) ([491e650](https://github.com/oscarbc96/wren/commit/491e650e7535da153de9572049886e283512cfc2))
* **cdp:** dom, network and stub domains ([#46](https://github.com/oscarbc96/wren/issues/46)) ([8cea37d](https://github.com/oscarbc96/wren/commit/8cea37df276e33553399f9d09e2c02de253a255d))
* **cdp:** page, runtime, input and console domains ([#45](https://github.com/oscarbc96/wren/issues/45)) ([3578c84](https://github.com/oscarbc96/wren/commit/3578c84dd59d26f03eb3f507a93fce18e3b641a4))
* **cdp:** protocol server, discovery and target sessions ([#41](https://github.com/oscarbc96/wren/issues/41)) ([1a2164e](https://github.com/oscarbc96/wren/commit/1a2164e03ead8657da48660f264404a693793be9))
* **engine:** evaluate with result, screenshots, console and request events ([#42](https://github.com/oscarbc96/wren/issues/42)) ([08dffd8](https://github.com/oscarbc96/wren/commit/08dffd8671dfb831bc7fd90dbc2e4836cd737f11))
* **extensions:** manifest, crx3 verification and registry ([#47](https://github.com/oscarbc96/wren/issues/47)) ([ffe1f19](https://github.com/oscarbc96/wren/commit/ffe1f19427c4c74b60bbdd3089cc895848799add))
* fetch script for the chrome devtools frontend ([#43](https://github.com/oscarbc96/wren/issues/43)) ([205c848](https://github.com/oscarbc96/wren/commit/205c848247ee24b0cc03f5c57b5377ca8516e43d))
* **parity:** top-sites harness and must-work smoke tests ([#37](https://github.com/oscarbc96/wren/issues/37)) ([a836f5d](https://github.com/oscarbc96/wren/commit/a836f5d1ec959c828224538e8f2a6f302f7394e9))
* **tabs:** sleep idle tabs and keep a memory budget ([#29](https://github.com/oscarbc96/wren/issues/29)) ([1ec25da](https://github.com/oscarbc96/wren/commit/1ec25da92d11445a4de2d9b8fdfb19f9709ec744))
* **wren:** devtools via servo remote debugging ([#36](https://github.com/oscarbc96/wren/issues/36)) ([d9df6c4](https://github.com/oscarbc96/wren/commit/d9df6c49f2f1b8e529595a6c01664f8f2b5b84c5))
* **wren:** downloads ([#35](https://github.com/oscarbc96/wren/issues/35)) ([eccc4a5](https://github.com/oscarbc96/wren/commit/eccc4a51294174050efc3bcbb94d4306e9a16bbf))
* **wren:** open in chrome with per-site rules ([#32](https://github.com/oscarbc96/wren/issues/32)) ([626cba5](https://github.com/oscarbc96/wren/commit/626cba5073dfe85d4bdafd7881a8b8b528fb6171))
* **wren:** task manager, end process and hang watchdog ([#33](https://github.com/oscarbc96/wren/issues/33)) ([914d403](https://github.com/oscarbc96/wren/commit/914d403ed3e22a54ef70f0913cb615883e0c8d3c))


### Fixes

* **wren:** pages render, browser chrome polish and a window screenshot tool ([#34](https://github.com/oscarbc96/wren/issues/34)) ([4e15b2e](https://github.com/oscarbc96/wren/commit/4e15b2e5c2fa777f9e2b8a2ab93a58d2f821b536))


### Performance

* memory pressure response and allocator relief ([#39](https://github.com/oscarbc96/wren/issues/39)) ([f4498c5](https://github.com/oscarbc96/wren/commit/f4498c5cb21f7eb50c5dcbdf6001bf52a6eb9189))
* memory report, profile footprint and the js gc profile ([#38](https://github.com/oscarbc96/wren/issues/38)) ([028a85a](https://github.com/oscarbc96/wren/commit/028a85aab16570530dcb5ff4d68afe972f33ae37))


### Docs

* bun spike (mimalloc and bun for extension workers) ([1d38692](https://github.com/oscarbc96/wren/commit/1d3869279e8f4fdd3a5e37744cbe8b673947d40e))
* generative UI is the moat (spec, roadmap, m5 logged-in tools and re-check) ([eb2246c](https://github.com/oscarbc96/wren/commit/eb2246c5366361c6a41b64ec317778249c218140))
* m2 chrome devtools plan ([67ece48](https://github.com/oscarbc96/wren/commit/67ece48894ab031834842d388605527e1dee6947))
* m3 extensions plan ([4062d61](https://github.com/oscarbc96/wren/commit/4062d6132f032cd58a4b8ab6fb58aa6aadae028f))
* m4 ai core plan ([5a8966e](https://github.com/oscarbc96/wren/commit/5a8966e556ee33fc70e44e5c3575ad9b769c8816))
* m5 generated pages plan ([457f90e](https://github.com/oscarbc96/wren/commit/457f90ebb72e2f2431109d274c282ad57fb71018))
* m6, m7 and alpha plans ([ec18631](https://github.com/oscarbc96/wren/commit/ec18631137e41b636fd6a9e1ee3ffb1fd03191de))
* **spec:** servo patch series on the fork, no upstream prs, fork policy ([fecef84](https://github.com/oscarbc96/wren/commit/fecef842b317bc0e41333e84d8cec13b369e31f6))

## 0.1.0 (2026-11-14)


### ⚠ BREAKING CHANGES

* relicense Wren under MPL-2.0 ([#15](https://github.com/oscarbc96/wren/issues/15))

### Features

* **creds:** keychain passwords, chrome and csv import, fill ([#28](https://github.com/oscarbc96/wren/issues/28)) ([9calf9ad](https://github.com/oscarbc96/wren/commit/9caf9adbddeb90a2330df27f3576ae388048dcc2))
* **engine:** engine trait and fake engine ([#11](https://github.com/oscarbc96/wren/issues/11)) ([f3e2c41](https://github.com/oscarbc96/wren/commit/f3e2c4186f87981808a2616c3db76931b6034f4a))
* **engine:** page input through the Engine trait ([#21](https://github.com/oscarbc96/wren/issues/21)) ([12f177a](https://github.com/oscarbc96/wren/commit/12f177a535f83544b8c9f54a496c75b00ed5e1ec))
* **engine:** Servo engine with one content process per tab ([#19](https://github.com/oscarbc96/wren/issues/19)) ([d83dd5d](https://github.com/oscarbc96/wren/commit/d83dd5d1974e1144e4a2a0078875b3b162d145ce))
* **omnibox:** classify, suggest and safe host display ([#14](https://github.com/oscarbc96/wren/issues/14)) ([861b170](https://github.com/oscarbc96/wren/commit/861b170df79be8c15f3136c7a7519030a8211632))
* **parity:** ram benchmark and wpt diff against chrome ([#10](https://github.com/oscarbc96/wren/issues/10)) ([333769f](https://github.com/oscarbc96/wren/commit/333769f04e80f5ed2334028417466a00ce6e11e0))
* **settings:** settings, themes and components on GPUI-CE ([#13](https://github.com/oscarbc96/wren/issues/13)) ([42423c0](https://github.com/oscarbc96/wren/commit/42423c08f7d0141cd1fafc347bbb0b410b92058e))
* **site:** new Wren brand, theme, downloads page and pending release ([#25](https://github.com/oscarbc96/wren/issues/25)) ([b5111aa](https://github.com/oscarbc96/wren/commit/b5111aa0c7c42a22864a2fcb402ccef7cb020909))
* **store:** history, bookmarks and session in sqlite ([#8](https://github.com/oscarbc96/wren/issues/8)) ([10538e6](https://github.com/oscarbc96/wren/commit/10538e6ae211153f161fe80af88e907416b94657))
* **tabs:** tab strip model ([#12](https://github.com/oscarbc96/wren/issues/12)) ([62b4e4c](https://github.com/oscarbc96/wren/commit/62b4e4cfc2f212c743c1c9d999ddbe9cd15c81c1))
* **wren:** browser window model, session, history and bookmarks ([#20](https://github.com/oscarbc96/wren/issues/20)) ([cbb159d](https://github.com/oscarbc96/wren/commit/cbb159ddffcef795a0008c82b26082c609e9c636))
* **wren:** usable browser window with omnibox, tabs, shortcuts and page input ([#22](https://github.com/oscarbc96/wren/issues/22)) ([da5be9f](https://github.com/oscarbc96/wren/commit/da5be9ff0f15213f6474744ce7f8867f3309d29c))


### Docs

* add memory diet task to the m1 plan ([#27](https://github.com/oscarbc96/wren/issues/27)) ([415750b](https://github.com/oscarbc96/wren/commit/415750b44b069f940d4e218f707677bbbe31db84))
* m1 daily driver plan ([#23](https://github.com/oscarbc96/wren/issues/23)) ([fa9ec33](https://github.com/oscarbc96/wren/commit/fa9ec33bd2e85632eee1c82de5adb1559d15a4ec))
* one content process per tab, task manager and hang watchdog ([#17](https://github.com/oscarbc96/wren/issues/17)) ([2abeb97](https://github.com/oscarbc96/wren/commit/2abeb977724637c9a75061f9464a43c3ce8b2508))
* **perf:** m0 ram benchmark, wren at parity with chrome on footprint ([#24](https://github.com/oscarbc96/wren/issues/24)) ([04d7cf0](https://github.com/oscarbc96/wren/commit/04d7cf0e6ce36fdbcb6145256eeb2c1e68952638))
* **site:** wren landing page and docs site ([#1](https://github.com/oscarbc96/wren/issues/1)) ([3f273cd](https://github.com/oscarbc96/wren/commit/3f273cde2dff2f0cbf44b2ff4f17cefb36874912))
* Wren design spec, roadmap and M0 plan ([80ae8f7](https://github.com/oscarbc96/wren/commit/80ae8f7c22f7909bc2f35f3c52ee640fff41239b))


### Chores

* relicense Wren under MPL-2.0 ([#15](https://github.com/oscarbc96/wren/issues/15)) ([3357242](https://github.com/oscarbc96/wren/commit/3357242cb21c9eec36d01d44db740bd3d4e857fb))
