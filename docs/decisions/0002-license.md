# 0002: Fully open source under MIT + Apache-2.0

**Status:** Accepted (October 2026)

## Context

A programming language only succeeds if people and companies are free to use it. Noctis Labs plans to earn money from products built with Quell, not from Quell itself.

Options considered:

- **Copyleft (GPL):** forces derived works to be open source. Many companies avoid it, which would limit adoption.
- **MIT only:** simple, but has no explicit patent protection.
- **Apache-2.0 only:** has patent protection, but is incompatible with GPLv2 projects.
- **MIT + Apache-2.0, user's choice:** the combination used by Rust and most of its ecosystem.

## Decision

Everything in the Quell project (compiler, standard library, tools, docs, registry code) is dual licensed under MIT and Apache-2.0, at the user's option. Programs written in Quell belong entirely to their authors.

The name "Quell" and its logo are trademarks of Noctis Labs, to prevent broken or malicious software from using the name.

Nothing in the language, compiler, standard library or official tools will ever be paywalled.

## Why

- Anyone can use Quell for anything, commercial or not, with no fees or permission.
- Apache-2.0 adds patent protection, which companies check for before adopting a language.
- MIT keeps compatibility with GPLv2 projects and is the most widely understood license.
- Trademark protection keeps users safe without restricting the code.

## Trade-offs

- Others can build closed-source products on Quell without giving anything back. This is intended: wide adoption matters more.
- Funding has to come from products, sponsorships, donations or services, not from the language itself.
