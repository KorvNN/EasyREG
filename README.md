<p>
  <img src="apps/easyreg-server/static/easyreg-icon.svg" alt="EasyREG" width="96" align="left">
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
cargo run -p easyreg-server

# Open http://127.0.0.1:3000
```

Paste log lines into the workspace or drop a UTF-8 `.log`, `.txt`, `.json`,
`.jsonl`, or `.ndjson` file onto it. Large files are read as a stream in the
browser; EasyREG keeps a bounded, varied sample instead of loading the complete
file into memory.

## CLI

```bash
cargo run -p easyreg-cli -- infer \
  -p 'INV-2026-00127' \
  -p 'INV-2025-84621' \
  -p 'INV-2026-18342' \
  -n 'ORD-2026-00127' \
  -n 'INV-26-127' \
  -n 'INV-2026-ABCDE'
```

The result is machine-readable JSON containing the inferred candidates, rendered
expressions, validation results, captured values, scores, and recommendation.
Use `--mode search` to find a pattern within the input or `--compact` for
single-line JSON.

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
| `easyreg-cli` | Command-line interface |
| `easyreg-server` | HTTP API and embedded web workspace |

## Development

Run formatting, Clippy, the complete Rust test suite, and the web JavaScript
syntax check with one command:

```bash
./scripts/check.sh
```

The versioned corpus in `tests/corpus/` covers distinct log families, close
negative examples, expected semantic rules, and expected capture values. Its
end-to-end harness requires 100% positive coverage and 100% negative rejection
from the recommended expression.
