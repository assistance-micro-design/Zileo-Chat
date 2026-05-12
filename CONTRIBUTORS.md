# Contributors

Thank you to all who contribute to Zileo-Chat-3!

## Organizations

### Assistance Micro Design

- **GitHub**: https://github.com/assistance-micro-design
- **Role**: Project Owner and Primary Maintainer

## Individual Contributors

| Name    | GitHub                                 | Contributions                                                                                                                                |
| ------- | -------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| ScioNos | [@ScioNos](https://github.com/ScioNos) | Frontend, workflow, streaming, MCP, Tauri integration, dependency maintenance and audit-driven hardening — [RouterLab](https://routerlab.ch) |

### ScioNos / RouterLab contributions

ScioNos contributions include several audit-driven fixes and frontend maintenance pull requests submitted from the ScioNos fork and merged or reviewed upstream:

- [#121](https://github.com/assistance-micro-design/Zileo-Chat/pull/121) — First complex workflow/MCP stability pull request. Closed after rebase conflicts and replaced by the cleaner follow-up PR #125.
- [#125](https://github.com/assistance-micro-design/Zileo-Chat/pull/125) — Stabilized streaming and MCP lifecycle handling. Merged after CI-driven Rust formatting and backend test corrections.
- [#127](https://github.com/assistance-micro-design/Zileo-Chat/pull/127) — Secured frontend runtime access, workflow race guards, streaming cancellation and validation flows. Merged upstream.
- [#130](https://github.com/assistance-micro-design/Zileo-Chat/pull/130) — Centralized frontend Tauri access through the `$lib/tauri` adapter layer and updated frontend tests/mocks. Merged upstream.
- [#133](https://github.com/assistance-micro-design/Zileo-Chat/pull/133) — Vitest frontend configuration PR opened on upstream by mistake, then closed cleanly without merge.
- [#140](https://github.com/assistance-micro-design/Zileo-Chat/pull/140) — Large frontend component refactor with helper extraction and targeted tests. Merged upstream.

## How to Contribute

We welcome contributions! Please see:

- [README.md](README.md) for project overview
- [docs/](docs/) for development documentation

### Adding Yourself as a Contributor

1. Fork the repository
2. Add your name to this file
3. Submit a Pull Request

## License

All contributions are made under the [Apache License 2.0](LICENSE).
