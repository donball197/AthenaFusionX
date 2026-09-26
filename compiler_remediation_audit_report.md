```json
[
  {
    "type": "read_file",
    "path": "crates/runtime_core/src/json_parser.rs",
    "reasoning": "The JSON parser handles the deserialization of the compiler output. We need to read this file to verify that it correctly handles bare JSON arrays and objects, and that it unwraps nested write_file envelopes."
  },
  {
    "type": "read_file",
    "path": "crates/runtime_core/src/arbiter_agent.rs",
    "reasoning": "The arbiter agent is the security boundary for the system. We need to read this file to verify that it remains independent and is not compromised by the remediation logic."
  }
]
```