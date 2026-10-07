# B-Health

A lightweight terminal (ratatui) tracker for blood test results from Brazilian labs.
Everything stays local: SQLite + copies of your PDFs in `~/.local/share/b-health`.

## Usage

```
cargo run --release                     # open the app
cargo run --release -- import file.pdf  # import a report straight into the review screen
cargo run --release -- parse file.pdf   # print what the parser reads (no saving)
cargo run --release -- text file.pdf    # print the extracted PDF text (parser development)
cargo run --release -- export csv|json  # export all data to stdout
```

Keys on the main screen: `i` import · `w` log weight · `t` personal target · `p` profile ·
`L` Português/English · `e` export · `Tab`/`1-3` tabs · `q` quit.

## Supported labs

| Lab | Module |
|-----|--------|
| Laboratório Santa Maria (lacsm.com.br) | `src/parser/santa_maria.rs` |
| Unimed (web report viewer export, incl. partner-lab pages) | `src/parser/unimed.rs` |

To add a lab: implement `LabParser` in a new module under `src/parser/` and register it in
`PARSERS`. Parsers return results plus the raw reference text; sex/age-specific ranges are
resolved centrally (`units::pick_range`). `b-health text <pdf>` shows the text the parser receives. Only PDFs with a text
layer are supported (scanned images are not, yet).

## Development

```
cargo test
cargo test screens -- --ignored --nocapture   # renders every screen using samples/*.pdf
```

`samples/` holds real (personal) reports and is git-ignored.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for
inclusion in this project, as defined in the Apache-2.0 license, shall be dual licensed
as above, without any additional terms or conditions.

*Informational only — not a medical diagnosis.*
