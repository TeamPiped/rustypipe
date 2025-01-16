## Development

**Requirements:**

- Current version of stable Rust
- [`just`](https://github.com/casey/just) task runner
- [`nextest`](https://nexte.st) test runner
- [`pre-commit`](https://pre-commit.com/)
- yq (YAML processor)

### Tasks

**Testing**

- `just test` Run unit+integration tests
- `just unittest` Run unit tests
- `just testyt` Run YouTube integration tests
- `just testintl` Run YouTube integration tests for all supported languages (this takes
  a long time and is therefore not run in CI)
- `YT_LANG=de just testyt` Run YouTube integration tests for a specific language

**Tools**

- `just testfiles` Download missing testfiles for unit tests
- `just report2yaml` Convert RustyPipe reports into a more readable yaml format
  (requires `yq`)
