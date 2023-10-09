In `snark-verifier` root directory:

Addition:

1. Run

```
cargo run --example rlc-evm-verifier --release
```

to test rlc between plonk protocol and plain kzg commitment without the need to perform KZG chip. We basically just defer the pairing check of kzg commitment by doing RLC with kzg commitment of halo2 backend in order to verify just once on-chain. This is called Aggregation.

Note: According to Yi. Thanks!

```
In any aggregation procedure, you have 2 options for what to do with the pairing:
1. Verify it in ZK using the KZG chip in halo2, which will cost slightly more in ZK (but not a huge amount)
2. RLC it into the final on-chain check, which will cost slightly more on-chain and requires you to be careful the RLC is done correctly.  But this will save cost in ZK.
```

Original:

1. Create `./configs/verify_circuit.config`

2. Run

```
cargo run --example evm-verifier-with-accumulator --release
```
