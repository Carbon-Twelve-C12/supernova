# Fuzz seed corpora

Each subdirectory holds AFL++ seed inputs for the matching fuzz target:

- `block_validation/`    — bytes interpreted as bincode-encoded `Block`
- `transaction_parsing/` — bytes interpreted as bincode-encoded `Transaction`
- `quantum_crypto/`      — `[level_tag][split_a][split_b][pk || sig || msg]`
- `p2p_messages/`        — `[variant_tag][bincode-encoded body]`
- `consensus/`           — `[target u32 LE][split u8][u64 LE timestamps…][u64 LE heights…]`

## Populating seeds

The simplest starting seed is a single byte:

```bash
printf '\x00' > fuzz/corpus/block_validation/seed-zero
```

High-quality seeds come from recording real-world wire traffic:

```bash
# Capture a real block as a seed (from a synced node)
supernova-cli block get <hash> --raw > fuzz/corpus/block_validation/block-<hash>

# Capture a real transaction
supernova-cli tx get <txid> --raw > fuzz/corpus/transaction_parsing/tx-<txid>
```

AFL++ will preserve, mutate, and cross-pollinate any seeds placed here. Keep
seeds small (<4 KiB) and semantically diverse rather than many near-copies.

Corpus minimization (`afl-cmin`) should be run periodically to trim
redundant seeds — see `fuzz/README.md`.
