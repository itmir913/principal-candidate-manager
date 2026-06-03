# Contributing

Thank you for contributing.

## Contribution License

By submitting a contribution, you agree that:

1. You assign to the project owner (itmir913, luminousky.com) all copyright and related rights in your contribution, worldwide and in perpetuity. This assignment takes effect upon submission of your contribution.

2. This assignment allows the project owner to use, modify, distribute, sublicense, and relicense your contribution under any terms, including terms different from the current project license, at their sole discretion.

3. You represent that:
    - you are the sole author of the contribution and have the legal right to assign these rights,
    - it does not violate any third-party rights, and
    - it does not introduce any license terms or dependencies that conflict with the project license.

4. You assign any patent rights necessary to use, modify, distribute, and sublicense your contribution as part of the project.

5. Contributions are provided "as is", without warranty of any kind.

6. The project owner reserves the right to accept, reject, modify, or remove contributions at their sole discretion.

## How to Contribute

- Fork the repository
- Create a feature branch
- Submit a pull request with a clear description

## Technical Rules

This codebase has strict invariants. Please review before submitting a PR.

- **Float-Free**: All scores are stored as integers multiplied by 100,000. Never use `f32`/`f64` for scores, and never divide on the frontend.
- **Fail-Fast**: Return `Err` immediately on any score calculation error. `unwrap_or(0)` and similar silent fallbacks are not allowed.
- **Score calculation is backend-only**: The frontend displays values only. No score logic in Vue components.
- **Transactions**: Any handler that performs multiple writes must use a SQLx transaction.
- **Tests**: Run `npm test` (`cargo test`) before submitting. New validation logic must include passing, boundary, and rejection test cases.
- **Commit signing**: All commits must be GPG-signed. Do not use `--no-verify` or `--no-gpg-sign`.
- **Font size**: Do not use `text-sm`, `text-xs`, or any `font-size` below `text-base` in the frontend.
