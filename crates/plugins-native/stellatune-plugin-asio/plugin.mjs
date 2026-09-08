// Native PCM output is opened directly by the Rust sink adapter. No Node
// process, JSON audio, or per-block RPC is needed for device playback.
export default {
  descriptor: {
    id: "dev.stellatune.output.asio",
    apiVersion: 2,
    capabilities: ["asio"],
  },
  invoke() {
    throw new Error("ASIO is a native output sink; select it in Settings > Audio");
  },
};
