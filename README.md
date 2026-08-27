<p>
  <img src="static/easyreg-icon.svg" alt="EasyREG" width="96" align="left">
</p>

# EasyREG<br><sup><sup><em>"From real log lines to tested parsers."</em></sup></sup>

EasyREG is a local, example-driven regex inference tool. It learns the structure
of real log lines, rejects near misses, assigns contextual capture names, and
exports validated regex and parser code.

## Highlights

- Generates strict, balanced, and flexible patterns from positive examples
- Uses negative examples to avoid matching similar but unrelated input
- Recognizes common log values such as IP addresses, UUIDs, timestamps, paths,
  URLs, email addresses, numbers, and hexadecimal values
- Assigns portable capture names from nearby keys such as `client_ip=`,
  `status=`, and `duration=`
- Exports JavaScript, Python, and PCRE2 regex plus JavaScript and Python parsers
- Validates every candidate against every supplied example before recommending it
- Runs locally without API keys, external services, or data uploads

## Quick start

The repository pins its Rust toolchain in `rust-toolchain.toml`.

```bash
cargo run

# Open http://127.0.0.1:3000
```

Paste log lines into the workspace or drop a UTF-8 `.log`, `.txt`, `.json`,
`.jsonl`, or `.ndjson` file onto it. Large files are read as a stream in the
browser; EasyREG keeps a bounded, varied sample instead of loading the complete
file into memory.

## How it works

```text
Examples → structure inference → PatternSpec → dialect rendering → validation
```

EasyREG first builds a regex-engine-independent `PatternSpec`. The same inferred
structure can then be rendered for multiple regex dialects and checked against
all positive and negative examples. The web server adds local, context-aware
field names before returning the result.

## Workspace

| Package | Responsibility |
| --- | --- |
| `easyreg-core` | Request, analysis, and `PatternSpec` models |
| `easyreg-detectors` | Semantic value detection |
| `easyreg-inference` | Structure inference from examples |
| `easyreg-semantics` | Context-aware capture naming |
| `easyreg-dialects` | JavaScript, Python, and PCRE2 rendering |
| `easyreg-validation` | Match and capture validation |
| `easyreg-engine` | Candidate generation, scoring, and recommendation |
| `easyreg` | HTTP API and embedded web workspace |

## License

MIT License. See [LICENSE](LICENSE).
