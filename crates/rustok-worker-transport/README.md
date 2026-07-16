# rustok-worker-transport

This infrastructure crate owns shared mutually authenticated tonic listener and
client configuration for isolated RusToK workers. It loads mounted server/client
identity and trust material, applies bounded listener limits, and never exposes
a plaintext listener.

Workers supply a stable uppercase environment prefix. Worker-specific policy,
credentials, tool execution, database access, and request/result contracts do
not belong here.
