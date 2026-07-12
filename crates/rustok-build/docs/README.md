# rustok-build documentation

The build capability owns persistence contracts for queued builds and releases,
including the typed `ReleasePublisherPort` hand-off. Runtime worker and concrete
filesystem, HTTP, or container deployment adapters remain host responsibilities.
