#!/bin/bash
sed -i -e '/hidden_state\[i\] = residual\[i\] + attn_out\[i\];/ s/attn_out\[i\]/attn_out[i] * weights.layer_scalar/' \
       -e '/hidden_state\[i\] = residual\[i\] + mlp_out\[i\];/ s/mlp_out\[i\]/mlp_out[i] * weights.layer_scalar/' \
       -e '/hidden_state\[i\] += ple_out\[i\];/ s/ple_out\[i\]/ple_out[i] * weights.layer_scalar/' \
       -e '/\/\/ ---- layer_scalar ----/,+3d' crates/nexus_edge_minion/src/decoder_layer.rs
