# Contributing to Supernova

We welcome contributions from the community.

## How to Contribute

### Reporting Issues

- Check existing issues before creating a new one
- Provide clear description and steps to reproduce
- Include relevant system information and error messages

### Submitting Pull Requests

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes following our coding standards
4. Add tests for new functionality
5. Run tests and ensure they pass: `cargo test --workspace`
6. Run formatting: `cargo fmt --all`
7. Run clippy: `cargo clippy --all-targets`
8. Commit with clear, descriptive messages
9. Push to your fork and submit a pull request

## Development Guidelines

### Code Standards

- Follow Rust idioms and best practices
- Use `Result<T, E>` for error handling, avoid `unwrap()` in production code
- Write clear, self-documenting code with appropriate comments
- Ensure all public APIs are documented
- Maintain test coverage above 90%

### Testing

- Write unit tests for all new functionality
- Include integration tests for complex features
- Test edge cases and error conditions
- Run the full test suite before submitting PRs

### Security

- Never commit sensitive information (keys, passwords, tokens)
- Report security vulnerabilities privately to security@supernovanetwork.xyz
- Follow secure coding practices for blockchain systems

## Questions?

Feel free to open an issue for discussion or reach out to the maintainers.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
