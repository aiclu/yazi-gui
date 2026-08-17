# Use staged byte-level copy for cancellable transfers

yazi-gui keeps file-copy progress in the GUI and reports byte-level updates from a chunked background copy worker. Each file is written beside its destination as a temporary file and committed only after completion, so cancellation removes only the unfinished temporary file while completed siblings remain. A copy error stops the batch and keeps the file clipboard for retry; an explicit cancellation clears it. This preserves the existing copy-clipboard semantics without adding a yazi protocol or a second operation layer.
