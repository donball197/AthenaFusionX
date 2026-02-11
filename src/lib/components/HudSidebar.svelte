<script>
  export let vitals = { cpu_usage: 0, ram_percent: 0, status: "OFFLINE" };
  export let intelLog = "";
</script>
<div class="sidebar">
    <h3>System_Health</h3>
    <div class="metric">CPU: {vitals.cpu_usage}%</div>
    <div class="metric">RAM: {vitals.ram_percent}%</div>
    <div class="status">STATUS: {vitals.status}</div>
</div>
