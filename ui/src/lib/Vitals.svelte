<script>
    export let ram = "0MB";
    export let status = "OFFLINE";
    $: ramPercent = (parseInt(ram) / 2000) * 100; // 2GB Wall scaling
</script>

<div class="vitals-card">
    <div class="header">SYSTEM_STATUS: <span class={status}>{status}</span></div>
    <div class="meter">
        <div class="label">RAM_USAGE: {ram} / 2GB</div>
        <div class="bar"><div class="fill" style="width: {ramPercent}%"></div></div>
    </div>
</div>

<style>
    .vitals-card { background: #050505; border: 1px solid #1a1a1a; padding: 1rem; }
    .status { font-weight: bold; }
    .OPERATIONAL { color: #0f0; }
    .bar { background: #111; height: 4px; width: 100%; margin-top: 5px; }
    .fill { background: #0f0; height: 100%; transition: width 0.3s; }
</style>
