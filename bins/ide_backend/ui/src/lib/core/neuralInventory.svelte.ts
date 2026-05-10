// ui/src/lib/core/neuralInventory
// Nexus Singularity — Quota-Aware Model Inventory

export const neuralInventory = $state({
    models: [
        {
            id: "gemm-26b-moe",
            name: "gemm 26b Moe",
            tier: "STD",
            context: 1048576,
            speed: "fast",
            note: "Use sparingly — tight daily RPD"
        },
        {
            id: "gemma-4-31b-it",
            name: "Gemma 4 31B",
            tier: "ELITE",
            context: 262144,
            speed: "deep",
            note: "High headroom"
        },
        {
            id: "gemini-3.1-flash-lite",
            name: "3.1 flash lite",
            tier: "APEX",
            context: 1048576,
            speed: "frontier",
            note: "Reserved for singularity tasks"
        }
    ]
});
