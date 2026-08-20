# Update Flow Lifecycle

## Context

Manual update checks and downloads are asynchronous and can outlive the UI action that started them. Request identifiers, cancellation handles, and phase transitions were previously stored and changed directly by the application shell.

## Decision

update.rs owns UpdatePhase and UpdateState. The state exposes lifecycle operations for starting checks and downloads, accepting the current request, applying progress, cancelling a download, completing a check or download, and entering restart. The shell remains responsible for GPUI task scheduling and localized status messages, while platform HTTP and process adapters own Windows I/O.

## Consequences

A late check or download result cannot replace a newer request, and the settings page renders a single update projection. Update lifecycle rules can be tested without GPUI or Windows network calls.
