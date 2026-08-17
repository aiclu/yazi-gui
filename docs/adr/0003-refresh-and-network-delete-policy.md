# Refresh the active tab and delete network paths permanently

yazi-gui refreshes only the active tab by sending yazi's `cd <current directory>` action from a background task. The command result ends the tab's `Refresh Request`, while the `gui-files` plugin publishes the resulting snapshot when yazi emits it. Computer View scans drive roots on a background executor. Selection is reconciled by filename and a preview is cleared when its target disappears.

Windows network paths are identified from UNC prefixes or `GetDriveTypeW` for mapped drive letters. Local paths continue to use `trash::delete`; network files and directories use direct filesystem removal after one blocking confirmation for the whole batch. Mixed batches keep both behaviors. Deletion continues after an item error and reports the first failure, without retrying through another deletion mode.
