export default {
  descriptor: {
    id: "dev.stellatune.fixture.http-source",
    apiVersion: 2,
    capabilities: ["fixture-source", "fixture-search"],
  },
  async invoke(request) {
    if (request.capabilityId === "fixture-source" && request.operation === "resolve") {
      return {
        source: {
          kind: "http",
          url: request.input.url,
          headers: { "x-stellatune-fixture": "1" },
        },
        media: { mimeType: "audio/flac", codecHint: "flac" },
        capabilities: { seekable: true },
      };
    }
    if (request.operation === "echo") return request.input;
    const error = new Error(`unsupported fixture operation ${request.operation}`);
    error.code = "unsupported_operation";
    error.retryable = false;
    throw error;
  },
};
