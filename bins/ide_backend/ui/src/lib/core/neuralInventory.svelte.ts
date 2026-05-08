// ui/src/lib/core/neuralInventory
// Nexus Singularity — Quota-Aware Model Inventory

export const neuralInventory = $state({
    models: [
        {
            id: "gemini-2.5-flash-lite",
            name: "Gemini 2.5 Flash Lite",
            tier: "STD",
            context: 1048576,
            speed: "fast",
            note: "Use sparingly — tight daily RPD"
        },
        {
            id: "gemma-4-26b-a4b-it",
            name: "Gemma 4 26B A4B",
            tier: "PRO",
            context: 262144,
            speed: "balanced",
            note: "Safe daily driver — generous quota"
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
            id: "gemini-3.1-pro-preview",
            name: "Gemini 3.1 Pro Preview",
            tier: "APEX",
            context: 1048576,
            speed: "frontier",
            note: "Reserved for singularity tasks"
        }
    ]
});
