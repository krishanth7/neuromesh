# Contributing

Start with an engineering issue describing context, the problem, acceptance
criteria, tests, and documentation impact. Keep changes small enough for real
review and use conventional commit titles where helpful.

Create a feature branch. Run the commands in the README, plus integration or
failure tests relevant to your change. Include actual results in the PR body.
Review correctness, concurrency, errors, limits, authentication, performance,
and compatibility. Resolve genuine findings, then squash-merge only after CI
passes. Never invent review discussions, test runs, benchmark results, or users.

Update public behavior documentation with the code. Document limitations and
avoid claims of production readiness without evidence. New dependencies need a
clear purpose and compatible licenses. No unsafe Rust is allowed in our crates.

Contributions are provided under Apache-2.0. Do not submit code or data you lack
permission to contribute. Follow the [Code of Conduct](CODE_OF_CONDUCT.md).
