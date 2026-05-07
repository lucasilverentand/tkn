# Changelog

## [0.6.3](https://github.com/lucasilverentand/tkn/compare/v0.6.2...v0.6.3) (2026-05-07)


### Features

* Add 7 new plugins, fix ls transform bug, and improve 3 existing plugins ([e6e2dcc](https://github.com/lucasilverentand/tkn/commit/e6e2dcc8e753f89db1b959dc3aefca0b319d27c8))
* Add biome, swift, wrangler, deno plugins and improve routing ([4e60e18](https://github.com/lucasilverentand/tkn/commit/4e60e189bcb90c2fbf297855ec3f7287e5dfd52e))
* add Codex hook support to the CLI ([6b37046](https://github.com/lucasilverentand/tkn/commit/6b37046a101404c5dfea2cbfa3ec8c885e254f35))
* add Codex PreToolUse hook support ([806812a](https://github.com/lucasilverentand/tkn/commit/806812a019a4d6e4e2b0194739a7f336b81203c0))
* add Codex support to hook setup commands ([ba58f8d](https://github.com/lucasilverentand/tkn/commit/ba58f8d4c87cdd4723e66b69f6afe033bfd245c5))
* add diagnose skill for analyzing full-log read patterns ([6d624ef](https://github.com/lucasilverentand/tkn/commit/6d624ef9ad60332b28ace71771c6d388514cf2f2))
* Add global path shortening and improve 16 plugins ([0ed604b](https://github.com/lucasilverentand/tkn/commit/0ed604bae2c0ea3ed7833e5ee6ac3ff67419e15d))
* Add JSON compaction, duplicate line collapsing, and 7 new plugins ([b1cab2e](https://github.com/lucasilverentand/tkn/commit/b1cab2e52e26c91427e0709be05695e5c369ba58))
* Add max_lines to 41 plugins missing output caps ([459274a](https://github.com/lucasilverentand/tkn/commit/459274a37d64af5dc5735b98485cc0b6316c99e7))
* Add README, install script, and release-please CI ([feb3917](https://github.com/lucasilverentand/tkn/commit/feb3917efbf4a929d04be089c7f54ab64f5d286f))
* Add REPL/editor/interactive routing and propagate exit codes ([3490f78](https://github.com/lucasilverentand/tkn/commit/3490f78b584df22df04be4a75ffd2e3ab5d43e5d))
* add setup and doctor flows for Claude and Codex ([4eacdf7](https://github.com/lucasilverentand/tkn/commit/4eacdf7782f03de41ba93f2415b72d22185b991e))
* Deep optimization pass across all plugins and fix flag-value splitting ([d2c0af2](https://github.com/lucasilverentand/tkn/commit/d2c0af2d6fe5cf3f7e8422978ab44f4ae1936f95))
* Enhance analyze command with analytics, reliability, and performance data ([30bd283](https://github.com/lucasilverentand/tkn/commit/30bd283f22d61c6d5df1a6b9739ffb16a06b47a8))
* Expand plugin system to 146 plugins across 62 tool bundles ([637d8ce](https://github.com/lucasilverentand/tkn/commit/637d8ce1bf532cee654c43a63ea7e43791896251))
* Expand routing with docker, k8s, JVM, just, and prefix commands ([1b91fd3](https://github.com/lucasilverentand/tkn/commit/1b91fd3a8c9142215ec98995a22dc35154dafa7d))


### Bug Fixes

* Add || test coverage and remove duplicate assertion ([7845aa3](https://github.com/lucasilverentand/tkn/commit/7845aa3022c22b53c22c45c3b42e1e874241bbdf))
* **deps:** update rust crate toml to 0.9 ([85f1c89](https://github.com/lucasilverentand/tkn/commit/85f1c89041449ddb3484625626cd1343eb6cd967))
* **deps:** update rust crate toml to 0.9 ([18c61c7](https://github.com/lucasilverentand/tkn/commit/18c61c777b35ab6d13002ef3b818554c31704950))
* **deps:** update rust crate toml to v1 ([39d965a](https://github.com/lucasilverentand/tkn/commit/39d965a615d4d484f76f2b27f441b886e001bc66))
* **deps:** update rust crate toml to v1 ([83022dc](https://github.com/lucasilverentand/tkn/commit/83022dc0165a1ed3070c5d83a8fca824c1566334))
* draft releases until assets upload ([03c1e9c](https://github.com/lucasilverentand/tkn/commit/03c1e9c81fdd24498405bcacbd5e8177ea680313))
* draft releases until assets upload ([cab87b5](https://github.com/lucasilverentand/tkn/commit/cab87b53d086d484ac75e0f8b200ffe6b13c5156))
* Fix PATH env-prefix normalization and apply final plugin micro-optimizations ([5f70df8](https://github.com/lucasilverentand/tkn/commit/5f70df898a8ccca7eaffbd0e4b31970a982b7295))
* Harden hook install, cache ANSI regex, and guard NaN in analyze ([05e1447](https://github.com/lucasilverentand/tkn/commit/05e1447d7367615a577b26945f5bc3b30bf7bd64))
* install all hooks by default ([aa0164b](https://github.com/lucasilverentand/tkn/commit/aa0164bad205b142d9ef5b02222b1461fc70bf05))
* preserve GitHub URLs in gh pr and set nl to raw mode ([b0769db](https://github.com/lucasilverentand/tkn/commit/b0769db099138029624fff383b9e52719459ae82))
* Remove --no-color from git push (Apple Git doesn't support it) ([51c06af](https://github.com/lucasilverentand/tkn/commit/51c06af662c3165ebcbf931a39258e161170f8ce))
* satisfy current Rust CI checks ([099a11c](https://github.com/lucasilverentand/tkn/commit/099a11c00e376c70221af16859438a5eba00a001))

## [0.6.2](https://github.com/lucasilverentand/tkn/compare/v0.6.1...v0.6.2) (2026-05-08)


### Bug Fixes

* install all hooks by default ([aa0164b](https://github.com/lucasilverentand/tkn/commit/aa0164bad205b142d9ef5b02222b1461fc70bf05))

## [0.6.1](https://github.com/lucasilverentand/tkn/compare/tkn-v0.6.0...tkn-v0.6.1) (2026-05-07)


### Features

* Add 7 new plugins, fix ls transform bug, and improve 3 existing plugins ([e6e2dcc](https://github.com/lucasilverentand/tkn/commit/e6e2dcc8e753f89db1b959dc3aefca0b319d27c8))
* Add biome, swift, wrangler, deno plugins and improve routing ([4e60e18](https://github.com/lucasilverentand/tkn/commit/4e60e189bcb90c2fbf297855ec3f7287e5dfd52e))
* add Codex hook support to the CLI ([6b37046](https://github.com/lucasilverentand/tkn/commit/6b37046a101404c5dfea2cbfa3ec8c885e254f35))
* add Codex PreToolUse hook support ([806812a](https://github.com/lucasilverentand/tkn/commit/806812a019a4d6e4e2b0194739a7f336b81203c0))
* add Codex support to hook setup commands ([ba58f8d](https://github.com/lucasilverentand/tkn/commit/ba58f8d4c87cdd4723e66b69f6afe033bfd245c5))
* add diagnose skill for analyzing full-log read patterns ([6d624ef](https://github.com/lucasilverentand/tkn/commit/6d624ef9ad60332b28ace71771c6d388514cf2f2))
* Add global path shortening and improve 16 plugins ([0ed604b](https://github.com/lucasilverentand/tkn/commit/0ed604bae2c0ea3ed7833e5ee6ac3ff67419e15d))
* Add JSON compaction, duplicate line collapsing, and 7 new plugins ([b1cab2e](https://github.com/lucasilverentand/tkn/commit/b1cab2e52e26c91427e0709be05695e5c369ba58))
* Add max_lines to 41 plugins missing output caps ([459274a](https://github.com/lucasilverentand/tkn/commit/459274a37d64af5dc5735b98485cc0b6316c99e7))
* Add README, install script, and release-please CI ([feb3917](https://github.com/lucasilverentand/tkn/commit/feb3917efbf4a929d04be089c7f54ab64f5d286f))
* Add REPL/editor/interactive routing and propagate exit codes ([3490f78](https://github.com/lucasilverentand/tkn/commit/3490f78b584df22df04be4a75ffd2e3ab5d43e5d))
* add setup and doctor flows for Claude and Codex ([4eacdf7](https://github.com/lucasilverentand/tkn/commit/4eacdf7782f03de41ba93f2415b72d22185b991e))
* Deep optimization pass across all plugins and fix flag-value splitting ([d2c0af2](https://github.com/lucasilverentand/tkn/commit/d2c0af2d6fe5cf3f7e8422978ab44f4ae1936f95))
* Enhance analyze command with analytics, reliability, and performance data ([30bd283](https://github.com/lucasilverentand/tkn/commit/30bd283f22d61c6d5df1a6b9739ffb16a06b47a8))
* Expand plugin system to 146 plugins across 62 tool bundles ([637d8ce](https://github.com/lucasilverentand/tkn/commit/637d8ce1bf532cee654c43a63ea7e43791896251))
* Expand routing with docker, k8s, JVM, just, and prefix commands ([1b91fd3](https://github.com/lucasilverentand/tkn/commit/1b91fd3a8c9142215ec98995a22dc35154dafa7d))


### Bug Fixes

* Add || test coverage and remove duplicate assertion ([7845aa3](https://github.com/lucasilverentand/tkn/commit/7845aa3022c22b53c22c45c3b42e1e874241bbdf))
* **deps:** update rust crate toml to 0.9 ([85f1c89](https://github.com/lucasilverentand/tkn/commit/85f1c89041449ddb3484625626cd1343eb6cd967))
* **deps:** update rust crate toml to 0.9 ([18c61c7](https://github.com/lucasilverentand/tkn/commit/18c61c777b35ab6d13002ef3b818554c31704950))
* **deps:** update rust crate toml to v1 ([39d965a](https://github.com/lucasilverentand/tkn/commit/39d965a615d4d484f76f2b27f441b886e001bc66))
* **deps:** update rust crate toml to v1 ([83022dc](https://github.com/lucasilverentand/tkn/commit/83022dc0165a1ed3070c5d83a8fca824c1566334))
* draft releases until assets upload ([03c1e9c](https://github.com/lucasilverentand/tkn/commit/03c1e9c81fdd24498405bcacbd5e8177ea680313))
* draft releases until assets upload ([cab87b5](https://github.com/lucasilverentand/tkn/commit/cab87b53d086d484ac75e0f8b200ffe6b13c5156))
* Fix PATH env-prefix normalization and apply final plugin micro-optimizations ([5f70df8](https://github.com/lucasilverentand/tkn/commit/5f70df898a8ccca7eaffbd0e4b31970a982b7295))
* Harden hook install, cache ANSI regex, and guard NaN in analyze ([05e1447](https://github.com/lucasilverentand/tkn/commit/05e1447d7367615a577b26945f5bc3b30bf7bd64))
* preserve GitHub URLs in gh pr and set nl to raw mode ([b0769db](https://github.com/lucasilverentand/tkn/commit/b0769db099138029624fff383b9e52719459ae82))
* Remove --no-color from git push (Apple Git doesn't support it) ([51c06af](https://github.com/lucasilverentand/tkn/commit/51c06af662c3165ebcbf931a39258e161170f8ce))
* satisfy current Rust CI checks ([099a11c](https://github.com/lucasilverentand/tkn/commit/099a11c00e376c70221af16859438a5eba00a001))

## [0.6.0](https://github.com/lucasilverentand/tkn/compare/v0.5.0...v0.6.0) (2026-05-07)


### Features

* add Codex hook support to the CLI ([6b37046](https://github.com/lucasilverentand/tkn/commit/6b37046a101404c5dfea2cbfa3ec8c885e254f35))
* add Codex support to hook setup commands ([ba58f8d](https://github.com/lucasilverentand/tkn/commit/ba58f8d4c87cdd4723e66b69f6afe033bfd245c5))

## [0.5.0](https://github.com/lucasilverentand/tkn/compare/v0.4.0...v0.5.0) (2026-05-07)


### Features

* add Codex PreToolUse hook support ([806812a](https://github.com/lucasilverentand/tkn/commit/806812a019a4d6e4e2b0194739a7f336b81203c0))


### Bug Fixes

* **deps:** update rust crate toml to 0.9 ([85f1c89](https://github.com/lucasilverentand/tkn/commit/85f1c89041449ddb3484625626cd1343eb6cd967))
* **deps:** update rust crate toml to 0.9 ([18c61c7](https://github.com/lucasilverentand/tkn/commit/18c61c777b35ab6d13002ef3b818554c31704950))
* **deps:** update rust crate toml to v1 ([39d965a](https://github.com/lucasilverentand/tkn/commit/39d965a615d4d484f76f2b27f441b886e001bc66))
* **deps:** update rust crate toml to v1 ([83022dc](https://github.com/lucasilverentand/tkn/commit/83022dc0165a1ed3070c5d83a8fca824c1566334))
* satisfy current Rust CI checks ([099a11c](https://github.com/lucasilverentand/tkn/commit/099a11c00e376c70221af16859438a5eba00a001))

## [0.4.0](https://github.com/lucasilverentand/tkn/compare/v0.3.0...v0.4.0) (2026-04-08)


### Features

* add diagnose skill for analyzing full-log read patterns ([6d624ef](https://github.com/lucasilverentand/tkn/commit/6d624ef9ad60332b28ace71771c6d388514cf2f2))
* add setup and doctor flows for Claude and Codex ([4eacdf7](https://github.com/lucasilverentand/tkn/commit/4eacdf7782f03de41ba93f2415b72d22185b991e))


### Bug Fixes

* preserve GitHub URLs in gh pr and set nl to raw mode ([b0769db](https://github.com/lucasilverentand/tkn/commit/b0769db099138029624fff383b9e52719459ae82))

## [0.3.0](https://github.com/lucasilverentand/tkn/compare/v0.2.0...v0.3.0) (2026-03-26)


### Features

* Add 7 new plugins, fix ls transform bug, and improve 3 existing plugins ([e6e2dcc](https://github.com/lucasilverentand/tkn/commit/e6e2dcc8e753f89db1b959dc3aefca0b319d27c8))
* Add biome, swift, wrangler, deno plugins and improve routing ([4e60e18](https://github.com/lucasilverentand/tkn/commit/4e60e189bcb90c2fbf297855ec3f7287e5dfd52e))
* Add global path shortening and improve 16 plugins ([0ed604b](https://github.com/lucasilverentand/tkn/commit/0ed604bae2c0ea3ed7833e5ee6ac3ff67419e15d))
* Add JSON compaction, duplicate line collapsing, and 7 new plugins ([b1cab2e](https://github.com/lucasilverentand/tkn/commit/b1cab2e52e26c91427e0709be05695e5c369ba58))
* Add max_lines to 41 plugins missing output caps ([459274a](https://github.com/lucasilverentand/tkn/commit/459274a37d64af5dc5735b98485cc0b6316c99e7))
* Add README, install script, and release-please CI ([feb3917](https://github.com/lucasilverentand/tkn/commit/feb3917efbf4a929d04be089c7f54ab64f5d286f))
* Add REPL/editor/interactive routing and propagate exit codes ([3490f78](https://github.com/lucasilverentand/tkn/commit/3490f78b584df22df04be4a75ffd2e3ab5d43e5d))
* Deep optimization pass across all plugins and fix flag-value splitting ([d2c0af2](https://github.com/lucasilverentand/tkn/commit/d2c0af2d6fe5cf3f7e8422978ab44f4ae1936f95))
* Enhance analyze command with analytics, reliability, and performance data ([30bd283](https://github.com/lucasilverentand/tkn/commit/30bd283f22d61c6d5df1a6b9739ffb16a06b47a8))
* Expand plugin system to 146 plugins across 62 tool bundles ([637d8ce](https://github.com/lucasilverentand/tkn/commit/637d8ce1bf532cee654c43a63ea7e43791896251))
* Expand routing with docker, k8s, JVM, just, and prefix commands ([1b91fd3](https://github.com/lucasilverentand/tkn/commit/1b91fd3a8c9142215ec98995a22dc35154dafa7d))


### Bug Fixes

* Add || test coverage and remove duplicate assertion ([7845aa3](https://github.com/lucasilverentand/tkn/commit/7845aa3022c22b53c22c45c3b42e1e874241bbdf))
* Fix PATH env-prefix normalization and apply final plugin micro-optimizations ([5f70df8](https://github.com/lucasilverentand/tkn/commit/5f70df898a8ccca7eaffbd0e4b31970a982b7295))
* Harden hook install, cache ANSI regex, and guard NaN in analyze ([05e1447](https://github.com/lucasilverentand/tkn/commit/05e1447d7367615a577b26945f5bc3b30bf7bd64))
* Remove --no-color from git push (Apple Git doesn't support it) ([51c06af](https://github.com/lucasilverentand/tkn/commit/51c06af662c3165ebcbf931a39258e161170f8ce))

## [0.2.0](https://github.com/lucasilverentand/tkn/compare/v0.1.0...v0.2.0) (2026-03-25)


### Features

* Expand plugin system to 146 plugins across 62 tool bundles ([637d8ce](https://github.com/lucasilverentand/tkn/commit/637d8ce1bf532cee654c43a63ea7e43791896251))

## 0.1.0 (2026-03-24)


### Features

* Add 7 new plugins, fix ls transform bug, and improve 3 existing plugins ([e6e2dcc](https://github.com/lucasilverentand/tkn/commit/e6e2dcc8e753f89db1b959dc3aefca0b319d27c8))
* Add biome, swift, wrangler, deno plugins and improve routing ([4e60e18](https://github.com/lucasilverentand/tkn/commit/4e60e189bcb90c2fbf297855ec3f7287e5dfd52e))
* Add global path shortening and improve 16 plugins ([0ed604b](https://github.com/lucasilverentand/tkn/commit/0ed604bae2c0ea3ed7833e5ee6ac3ff67419e15d))
* Add JSON compaction, duplicate line collapsing, and 7 new plugins ([b1cab2e](https://github.com/lucasilverentand/tkn/commit/b1cab2e52e26c91427e0709be05695e5c369ba58))
* Add max_lines to 41 plugins missing output caps ([459274a](https://github.com/lucasilverentand/tkn/commit/459274a37d64af5dc5735b98485cc0b6316c99e7))
* Add README, install script, and release-please CI ([feb3917](https://github.com/lucasilverentand/tkn/commit/feb3917efbf4a929d04be089c7f54ab64f5d286f))
* Add REPL/editor/interactive routing and propagate exit codes ([3490f78](https://github.com/lucasilverentand/tkn/commit/3490f78b584df22df04be4a75ffd2e3ab5d43e5d))
* Deep optimization pass across all plugins and fix flag-value splitting ([d2c0af2](https://github.com/lucasilverentand/tkn/commit/d2c0af2d6fe5cf3f7e8422978ab44f4ae1936f95))
* Enhance analyze command with analytics, reliability, and performance data ([30bd283](https://github.com/lucasilverentand/tkn/commit/30bd283f22d61c6d5df1a6b9739ffb16a06b47a8))
* Expand routing with docker, k8s, JVM, just, and prefix commands ([1b91fd3](https://github.com/lucasilverentand/tkn/commit/1b91fd3a8c9142215ec98995a22dc35154dafa7d))


### Bug Fixes

* Add || test coverage and remove duplicate assertion ([7845aa3](https://github.com/lucasilverentand/tkn/commit/7845aa3022c22b53c22c45c3b42e1e874241bbdf))
* Fix PATH env-prefix normalization and apply final plugin micro-optimizations ([5f70df8](https://github.com/lucasilverentand/tkn/commit/5f70df898a8ccca7eaffbd0e4b31970a982b7295))
* Harden hook install, cache ANSI regex, and guard NaN in analyze ([05e1447](https://github.com/lucasilverentand/tkn/commit/05e1447d7367615a577b26945f5bc3b30bf7bd64))
* Remove --no-color from git push (Apple Git doesn't support it) ([51c06af](https://github.com/lucasilverentand/tkn/commit/51c06af662c3165ebcbf931a39258e161170f8ce))
