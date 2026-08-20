# Workspace State and Yazi Session Seam

## Context

The GUI owns multiple independent views while one Yazi process publishes directory snapshots. Keeping tab state and asynchronous request checks in the application shell made event handling depend on shell fields and allowed protocol details to leak into unrelated code.

## Decision

WorkspaceState owns GUI tabs, the active tab, the tab viewport, drive roots, the Folder Tree, and per-tab refresh and search request tokens. YaziSession is the single backend process; its parser emits only the directory change and complete file-list data consumed by the workspace, and its handle exposes only directory navigation.

## Consequences

Stale refresh and search results are rejected at the workspace seam, and the shell coordinates UI lifetime and background execution without owning tab-domain transitions. Backend tab identifiers and unused hover metadata are no longer part of the application event model.
