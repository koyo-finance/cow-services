contracts:
	cargo run -p contracts --features=bin

orderbook-boba-debug:
	cargo run --bin orderbook -- \
	--node-url https://mainnet.boba.network \
	--base-tokens 0x66a2A913e447d6b4BF33EFbec43aAeF87890FBbc,0xf74195Bb8a5cf652411867c5C2C5b8C2a402be35 \
	--price-estimators Baseline,KoyoSor \
	--enable-presign-orders true \
	--koyo-sor-url https://api.koyo.finance/sor

solver-boba-debug:
	cargo run -p solver --  \
	--orderbook-url https://momiji.koyo.finance/boba/ \
    --base-tokens 0x66a2A913e447d6b4BF33EFbec43aAeF87890FBbc,0xf74195Bb8a5cf652411867c5C2C5b8C2a402be35 \
	--node-url https://mainnet.boba.network \
    --solver-account $(ACCOUNT)
