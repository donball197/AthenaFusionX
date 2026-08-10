//! Minimal generation harness: prompt -> tokens -> prefill -> greedy decode
//! loop -> generated text. This is an integration test bed for the
//! tokenizer + forward pass + LM head wired together -- not a chat
//! interface. No conversation state, no persona, no message history.

use nexus_edge_minion::inference_engine::InferenceEngine;
use nexus_edge_minion::tokenizer::NexusTokenizer;
use std::env;
use std::io::Write;
use std::path::PathBuf;

fn find_file(names: &[&str]) -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let bases = [manifest_dir.clone(), manifest_dir.join(".."), manifest_dir.join("../..")];
    for base in &bases {
        for name in names {
            let candidate = base.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let raw_prompt = args.get(1).cloned().unwrap_or_else(|| "The capital of France is".to_string());
    let max_new_tokens: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(8);

    let weights_path = find_file(&["gemma-4-E2B-it.safetensors"])
        .expect("Could not find gemma-4-E2B-it.safetensors in crate dir, parent, or workspace root.");
    let tokenizer_path = find_file(&["tokenizer.json"])
        .expect("Could not find tokenizer.json in crate dir, parent, or workspace root.");

    println!(">> Loading tokenizer from {}", tokenizer_path.display());
    let tok = NexusTokenizer::from_file(tokenizer_path.to_str().unwrap())
        .expect("Failed to load tokenizer.json");

    println!(">> Loading model weights from {}", weights_path.display());
    let mut engine = InferenceEngine::new(&weights_path).expect("Failed to init InferenceEngine");

    // --- STRUCTURAL FIX: Apply Chat Template & Enforce BOS (2) ---
    let formatted_prompt = format!("<start_of_turn>user\n{}<end_of_turn>\n<start_of_turn>model\n", raw_prompt);
    println!(">> Encoding prompt:\n{}", formatted_prompt);
    
    let mut prompt_ids = tok.encode(&formatted_prompt, true).expect("encode failed");
    
    // Gemma expects token 2 at index 0. Force it if the tokenizer omits it.
    if prompt_ids.is_empty() || prompt_ids[0] != 2 {
        prompt_ids.insert(0, 2);
    }
    // -------------------------------------------------------------

    println!(">> Prompt token IDs: {:?}", prompt_ids);

    const EOS_IDS: [u32; 2] = [1, 106];

    let mut hidden_state = vec![0.0f32; 1536];
    let mut position = 0usize;

    println!(">> Prefilling {} prompt tokens...", prompt_ids.len());
    for &tok_id in &prompt_ids {
        engine.step(tok_id, position, &mut hidden_state).expect("step failed during prefill");
        position += 1;
    }

    let mut generated_ids: Vec<u32> = Vec::new();

    println!(">> Generating up to {} tokens (greedy argmax)...", max_new_tokens);
    print!(">> Output: ");
    std::io::stdout().flush().ok();

    for _ in 0..max_new_tokens {
        let logits = engine.compute_logits(&hidden_state).expect("compute_logits failed");
        let (next_id, _next_val) = logits
            .iter()
            .enumerate()
            .fold((0usize, f32::NEG_INFINITY), |acc, (i, &v)| if v > acc.1 { (i, v) } else { acc });
        let next_id = next_id as u32;

        if EOS_IDS.contains(&next_id) {
            println!("\n>> EOS token {} reached, stopping.", next_id);
            break;
        }

        generated_ids.push(next_id);
        let piece = tok.decode(&[next_id], false).unwrap_or_else(|_| format!("<tok:{}>", next_id));
        print!("{}", piece);
        std::io::stdout().flush().ok();

        engine.step(next_id, position, &mut hidden_state).expect("step failed during decode");
        position += 1;
    }
    println!();

    println!(">> Generated token IDs: {:?}", generated_ids);
    let full_text = tok.decode(&generated_ids, true).unwrap_or_default();
    println!(">> Prompt + generated: {}{}", raw_prompt, full_text);
}
