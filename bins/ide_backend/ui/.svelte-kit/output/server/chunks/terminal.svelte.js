import "clsx";
const MAX_LOGS = 500;
const terminal = { logs: [] };
function addLog(message, cls = "text-gray-400") {
  const entry = {
    timestamp: /* @__PURE__ */ (/* @__PURE__ */ new Date()).toLocaleTimeString(),
    message,
    cls
  };
  if (terminal.logs.length >= MAX_LOGS) {
    terminal.logs = [...terminal.logs.slice(-MAX_LOGS + 1), entry];
  } else {
    terminal.logs = [...terminal.logs, entry];
  }
}
export {
  addLog as a,
  terminal as t
};
