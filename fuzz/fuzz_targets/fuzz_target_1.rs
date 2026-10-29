#![no_main]

use libfuzzer_sys::fuzz_target;
use nexus_ast_tool::mutation::{all_strategies, MutationError};

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 || data.len() > 4096 {
        return;
    }

    let strategy_idx = data[0] as usize;
    let symbol_len = (data[1] as usize) % 64;
    let payload_len = (data[2] as usize) % 128;

    let rest = &data[3..];
    if rest.len() < symbol_len + payload_len {
        return;
    }

    let symbol = match std::str::from_utf8(&rest[..symbol_len]) {
        Ok(s) if !s.is_empty() => s,
        _ => return,
    };

    let payload = match std::str::from_utf8(&rest[symbol_len..symbol_len + payload_len]) {
        Ok(s) => s,
        _ => return,
    };

    let source = match std::str::from_utf8(&rest[symbol_len + payload_len..]) {
        Ok(s) if s.len() <= 2048 => s,
        _ => return,
    };

    let strategies = all_strategies();
    if strategies.is_empty() {
        return;
    }

    let strategy = &strategies[strategy_idx % strategies.len()];

    // Accept all expected error variants – we only care about panics / OOM
    match strategy.apply(source, symbol, payload) {
        Ok(_) => {}
        Err(MutationError::ParseError(_)) => {}
        Err(MutationError::SymbolNotFound { .. }) => {}
        Err(MutationError::InvalidPayload(_)) => {}
        Err(MutationError::InvalidResult(_)) => {}
        Err(MutationError::TreeSitterError(_)) => {}
        Err(MutationError::Other(_)) => {}
    }
});
