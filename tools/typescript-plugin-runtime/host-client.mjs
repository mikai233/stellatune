import { setTimeout as delay } from "node:timers/promises";

/** Local player client. IDs are decimal strings and never converted to JS numbers. */
export function createHostClient(baseUrl) {
  async function request(path, body, signal) {
    const response = await fetch(new URL(path, baseUrl), {
      method: body === undefined ? "GET" : "POST",
      headers: { "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body), signal,
    });
    const value = await response.json();
    if (!response.ok) throw Object.assign(new Error(value.message ?? response.statusText), { code: value.code });
    return value;
  }
  const getState = (signal) => request("/player/state", undefined, signal);
  const getQueue = (signal) => request("/player/queue", undefined, signal);
  function subscribe(onEvent, onError = () => {}) {
    const controller = new AbortController();
    const { signal } = controller;
    void (async () => {
      while (!signal.aborted) {
        try {
          const response = await fetch(new URL("/player/events", baseUrl), { signal });
          if (!response.ok || !response.body) throw new Error(`events: HTTP ${response.status}`);
          const reader = response.body.getReader();
          const decoder = new TextDecoder();
          let buffer = "";
          try {
            while (!signal.aborted) {
              const { value, done } = await reader.read();
              if (done) break;
              buffer = (buffer + decoder.decode(value, { stream: true })).replace(/\r\n/g, "\n");
              let end;
              while ((end = buffer.indexOf("\n\n")) >= 0) {
                const frame = buffer.slice(0, end); buffer = buffer.slice(end + 2);
                const data = frame.split("\n").filter(line => line.startsWith("data:")).map(line => line.slice(5).trimStart()).join("\n");
                if (!data) continue;
                const event = JSON.parse(data);
                if (event.type === "resync") {
                  const [state, queue] = await Promise.all([getState(signal), getQueue(signal)]);
                  onEvent({ type: "snapshot", state, queue });
                } else onEvent(event);
              }
            }
          } finally { await reader.cancel().catch(() => {}); }
        } catch (error) { if (!signal.aborted) onError(error); }
        if (!signal.aborted) await delay(1000, undefined, { signal }).catch(() => {});
      }
    })();
    return () => controller.abort();
  }
  const command = body => request("/player/commands", body);
  return { command, getState, getQueue, subscribe, play: () => command({ command: "play" }),
    pause: () => command({ command: "pause" }), stop: () => command({ command: "stop" }),
    seek: positionMs => command({ command: "seek", positionMs }) };
}
