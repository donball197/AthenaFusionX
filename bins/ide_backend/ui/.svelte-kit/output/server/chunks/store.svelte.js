import "clsx";
const system = {
  cpu_load: 0,
  ram_gb: 0,
  total_ram_gb: 4,
  temperature: 30,
  resonance: 0,
  online: false
};
const ide$1 = {
  activeTab: null,
  openTabs: []
};
const systemVitals = system;
const ide = ide$1;
const wsState = {
  get ready() {
    return system.online;
  }
};
const vmoTelemetry = { active_vmos: 0 };
export {
  ide as i,
  systemVitals as s,
  vmoTelemetry as v,
  wsState as w
};
